use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use toml_edit::{value, Array, DocumentMut, InlineTable, Item, Table, Value};

use crate::config::{McpTransport, NormalizedMcpServer};

/// Merge normalized MCP servers into Codex's project-scoped `.codex/config.toml`.
/// Existing non-MCP settings, comments, Codex-specific server options, and MCP
/// servers not present in the source config are preserved.
pub fn merge_codex_mcp_toml(existing: &str, servers: &[NormalizedMcpServer]) -> Result<String> {
    let mut document = if existing.trim().is_empty() {
        DocumentMut::new()
    } else {
        existing
            .parse::<DocumentMut>()
            .context("failed to parse Codex config TOML")?
    };

    if !document.as_table().contains_key("mcp_servers") {
        document["mcp_servers"] = Item::Table(Table::new());
    }

    let mcp_servers_inline = document["mcp_servers"].is_inline_table();
    let mcp_servers = document["mcp_servers"]
        .as_table_like_mut()
        .context("Codex config `mcp_servers` must be a table")?;

    for server in servers {
        if !mcp_servers.contains_key(&server.name) {
            let entry = if mcp_servers_inline {
                Item::Value(Value::InlineTable(InlineTable::new()))
            } else {
                Item::Table(Table::new())
            };
            mcp_servers.insert(&server.name, entry);
        }
        let entry_item = mcp_servers
            .get_mut(&server.name)
            .context("newly inserted Codex MCP server is missing")?;
        let entry_inline = entry_item.is_inline_table();
        let entry = entry_item.as_table_like_mut().with_context(|| {
            format!("Codex config `mcp_servers.{}` must be a table", server.name)
        })?;

        // Every normalized source server is enabled. Keeping a pre-existing
        // `enabled = false` here would make sync/check report success while
        // Codex continues to hide the server.
        entry.remove("enabled");

        match &server.transport {
            McpTransport::Stdio { command, args } => {
                // Every key Codex rejects on a stdio server ("X is not
                // supported for stdio"), so a server switched from HTTP to
                // stdio stays loadable.
                for key in [
                    "url",
                    "http_headers",
                    "env_http_headers",
                    "http_headers_helper",
                    "bearer_token_env_var",
                    "bearer_token",
                    "auth",
                    "oauth",
                    "oauth_resource",
                ] {
                    entry.remove(key);
                }
                entry.insert("command", value(command.clone()));

                let mut toml_args = Array::new();
                for arg in args {
                    toml_args.push(arg);
                }
                entry.insert("args", value(toml_args));

                // Codex expands no `${VAR}`: `NAME=${NAME}` is forwarded from
                // Codex's environment through `env_vars`; anything else is a
                // literal value.
                let (forwarded, literal): (Vec<_>, Vec<_>) = server
                    .env
                    .iter()
                    .partition(|(name, env_value)| env_ref_name(env_value) == Some(name.as_str()));
                let literal: BTreeMap<String, String> = literal
                    .into_iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                set_string_map(entry, "env", &literal, entry_inline);
                // Object entries (`source = "remote"`) are Codex-only and kept.
                let mut env_vars: Array = entry
                    .get("env_vars")
                    .and_then(Item::as_array)
                    .map(|existing| existing.iter().filter(|v| !v.is_str()).cloned().collect())
                    .unwrap_or_default();
                for (name, _) in forwarded {
                    env_vars.push(name.as_str());
                }
                if env_vars.is_empty() {
                    entry.remove("env_vars");
                } else {
                    entry.insert("env_vars", value(env_vars));
                }
            }
            McpTransport::Http { url, headers } => {
                // Codex rejects `env` on an HTTP server: it is dropped, as in
                // every JSON shape without `env` on remote servers.
                entry.remove("command");
                entry.remove("args");
                entry.remove("env");
                entry.remove("cwd");
                entry.remove("env_vars");
                entry.insert("url", value(url.clone()));

                // Codex expands no `${VAR}` in headers: `Bearer ${VAR}` on
                // `Authorization` becomes `bearer_token_env_var`, and a header
                // that is exactly `${VAR}` becomes `env_http_headers`.
                let mut bearer = None;
                let mut from_env = BTreeMap::new();
                let mut literal = BTreeMap::new();
                for (name, header_value) in headers {
                    let bearer_ref = header_value
                        .strip_prefix("Bearer ")
                        .and_then(env_ref_name)
                        .filter(|_| name.eq_ignore_ascii_case("Authorization"));
                    if let Some(var) = bearer_ref {
                        bearer = Some(var.to_string());
                    } else if let Some(var) = env_ref_name(header_value) {
                        from_env.insert(name.clone(), var.to_string());
                    } else {
                        literal.insert(name.clone(), header_value.clone());
                    }
                }
                match bearer {
                    Some(var) => {
                        entry.insert("bearer_token_env_var", value(var));
                    }
                    None => {
                        entry.remove("bearer_token_env_var");
                    }
                }
                set_string_map(entry, "env_http_headers", &from_env, entry_inline);
                set_string_map(entry, "http_headers", &literal, entry_inline);
            }
        }
    }

    Ok(document.to_string())
}

/// `VAR` for a value that is exactly `${VAR}`.
fn env_ref_name(text: &str) -> Option<&str> {
    let name = text.strip_prefix("${")?.strip_suffix('}')?;
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .then_some(name)
}

/// Write `map` as the string table `key` of a Codex server entry (inline when
/// the entry is), or remove the key when `map` is empty.
fn set_string_map(
    entry: &mut dyn toml_edit::TableLike,
    key: &str,
    map: &BTreeMap<String, String>,
    inline: bool,
) {
    if map.is_empty() {
        entry.remove(key);
        return;
    }
    let mut table = Table::new();
    for (name, map_value) in map {
        table[name.as_str()] = value(map_value.clone());
    }
    let item = if inline {
        Item::Value(Value::InlineTable(table.into_inline_table()))
    } else {
        Item::Table(table)
    };
    entry.insert(key, item);
}

/// Merge normalized MCP servers into Mistral Vibe's project `.vibe/config.toml`,
/// an array of `[[mcp_servers]]` tables matched by `name`. Every other setting,
/// comment and server the source does not define is kept, and so are the
/// per-server keys conforme does not own (`prompt`, timeouts, `cwd`,
/// `disabled_tools`, an OAuth `auth`). Vibe expands no `${VAR}`: an
/// `Authorization: Bearer ${VAR}` header becomes a static `auth` reading the
/// token from `VAR` (`api_key_env`); any other value is written as it is.
pub fn merge_vibe_mcp_toml(existing: &str, servers: &[NormalizedMcpServer]) -> Result<String> {
    let mut document = if existing.trim().is_empty() {
        DocumentMut::new()
    } else {
        existing
            .parse::<DocumentMut>()
            .context("failed to parse Vibe config TOML")?
    };
    if !document.as_table().contains_key("mcp_servers") {
        document["mcp_servers"] = Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
    }
    let entries = document["mcp_servers"]
        .as_array_of_tables_mut()
        .context("Vibe config `mcp_servers` must be an array of tables ([[mcp_servers]])")?;

    for server in servers {
        let position = entries
            .iter()
            .position(|t| t.get("name").and_then(Item::as_str) == Some(server.name.as_str()));
        let index = match position {
            Some(index) => index,
            None => {
                let mut table = Table::new();
                table["name"] = value(server.name.clone());
                entries.push(table);
                entries.len() - 1
            }
        };
        let entry = entries
            .get_mut(index)
            .context("newly inserted Vibe MCP server is missing")?;
        // A synced server is enabled, as with every other target.
        entry.remove("disabled");

        match &server.transport {
            McpTransport::Stdio { command, args } => {
                for key in [
                    "url",
                    "headers",
                    "api_key_env",
                    "api_key_header",
                    "api_key_format",
                ] {
                    entry.remove(key);
                }
                if entry
                    .get("auth")
                    .and_then(|a| a.get("type"))
                    .and_then(Item::as_str)
                    != Some("oauth")
                {
                    entry.remove("auth");
                }
                entry.insert("transport", value("stdio"));
                entry.insert("command", value(command.clone()));
                let mut toml_args = Array::new();
                for arg in args {
                    toml_args.push(arg);
                }
                entry.insert("args", value(toml_args));
                set_string_map(entry, "env", &server.env, true);
            }
            McpTransport::Http { url, headers } => {
                for key in [
                    "command",
                    "args",
                    "env",
                    "cwd",
                    "headers",
                    "api_key_env",
                    "api_key_header",
                    "api_key_format",
                ] {
                    entry.remove(key);
                }
                entry.insert("transport", value("streamable-http"));
                entry.insert("url", value(url.clone()));

                let mut bearer = None;
                let mut literal = BTreeMap::new();
                for (name, header_value) in headers {
                    let bearer_ref = header_value
                        .strip_prefix("Bearer ")
                        .and_then(env_ref_name)
                        .filter(|_| name.eq_ignore_ascii_case("Authorization"));
                    match bearer_ref {
                        Some(var) => bearer = Some(var.to_string()),
                        None => {
                            literal.insert(name.clone(), header_value.clone());
                        }
                    }
                }
                let oauth = entry
                    .get("auth")
                    .and_then(|a| a.get("type"))
                    .and_then(Item::as_str)
                    == Some("oauth");
                if bearer.is_none() && literal.is_empty() {
                    if !oauth {
                        entry.remove("auth");
                    }
                } else {
                    let mut auth = InlineTable::new();
                    auth.insert("type", "static".into());
                    if let Some(var) = bearer {
                        auth.insert("api_key_env", var.into());
                        auth.insert("api_key_header", "Authorization".into());
                        auth.insert("api_key_format", "Bearer {token}".into());
                    }
                    if !literal.is_empty() {
                        let mut table = InlineTable::new();
                        for (name, header_value) in &literal {
                            table.insert(name, header_value.as_str().into());
                        }
                        auth.insert("headers", Value::InlineTable(table));
                    }
                    entry.insert("auth", value(auth));
                }
            }
        }
    }

    Ok(document.to_string())
}

/// Parse Vibe's `[[mcp_servers]]` tables into normalized servers (the inverse
/// of [`merge_vibe_mcp_toml`]). A disabled server is skipped; a static
/// `auth` reads back as headers (`api_key_env` as `Bearer ${VAR}`).
pub fn parse_vibe_mcp_toml(content: &str) -> Result<Vec<NormalizedMcpServer>> {
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    let root: toml::Value = toml::from_str(content).context("failed to parse Vibe config TOML")?;
    let Some(entries) = root.get("mcp_servers").and_then(toml::Value::as_array) else {
        return Ok(Vec::new());
    };
    let strings = |value: Option<&toml::Value>| -> BTreeMap<String, String> {
        value
            .and_then(toml::Value::as_table)
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut result = Vec::new();
    for entry in entries.iter().filter_map(toml::Value::as_table) {
        let Some(name) = entry.get("name").and_then(toml::Value::as_str) else {
            continue;
        };
        if entry.get("disabled").and_then(toml::Value::as_bool) == Some(true) {
            continue;
        }
        let transport = entry
            .get("transport")
            .and_then(toml::Value::as_str)
            .unwrap_or("stdio");
        let (transport, env) = if transport == "stdio" {
            // `command` is a string, or a list holding the arguments too.
            let mut parts: Vec<String> = match entry.get("command") {
                Some(toml::Value::String(command)) => vec![command.clone()],
                Some(toml::Value::Array(items)) => items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
                _ => continue,
            };
            if parts.is_empty() {
                continue;
            }
            let command = parts.remove(0);
            let args = entry
                .get("args")
                .and_then(toml::Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            parts.extend::<Vec<String>>(args);
            (
                McpTransport::Stdio {
                    command,
                    args: parts,
                },
                strings(entry.get("env")),
            )
        } else {
            let Some(url) = entry.get("url").and_then(toml::Value::as_str) else {
                continue;
            };
            // Legacy top-level `headers` / `api_key_env`, or a static `auth`.
            let auth = entry
                .get("auth")
                .and_then(toml::Value::as_table)
                .filter(|a| a.get("type").and_then(toml::Value::as_str) != Some("oauth"));
            let field = |key: &str| {
                auth.and_then(|a| a.get(key))
                    .or_else(|| entry.get(key))
                    .and_then(toml::Value::as_str)
            };
            let mut headers = strings(auth.and_then(|a| a.get("headers")).or(entry.get("headers")));
            if let Some(var) = field("api_key_env") {
                let header = field("api_key_header").unwrap_or("Authorization");
                let format = field("api_key_format").unwrap_or("Bearer {token}");
                headers.insert(
                    header.to_string(),
                    format.replace("{token}", &format!("${{{var}}}")),
                );
            }
            (
                McpTransport::Http {
                    url: url.to_string(),
                    headers,
                },
                BTreeMap::new(),
            )
        };
        result.push(NormalizedMcpServer {
            name: name.to_string(),
            transport,
            env,
        });
    }
    Ok(result)
}

/// Parse Codex's TOML MCP tables into normalized server definitions.
pub fn parse_codex_mcp_toml(content: &str) -> Result<Vec<NormalizedMcpServer>> {
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    let root: toml::Value = toml::from_str(content).context("failed to parse Codex config TOML")?;
    let Some(servers_value) = root.get("mcp_servers") else {
        return Ok(Vec::new());
    };
    let servers = servers_value
        .as_table()
        .context("Codex config `mcp_servers` must be a table")?;

    let mut result = Vec::new();
    for (name, value) in servers {
        let entry = value
            .as_table()
            .with_context(|| format!("Codex MCP server `{name}` must be a table"))?;

        if let Some(enabled) = entry.get("enabled") {
            let enabled = enabled.as_bool().with_context(|| {
                format!("Codex MCP server `{name}` field `enabled` must be a boolean")
            })?;
            if !enabled {
                continue;
            }
        }

        let url = optional_toml_string(entry, "url", name)?;
        let command = optional_toml_string(entry, "command", name)?;

        let (transport, env) = if let Some(url) = url {
            if command.is_some() {
                bail!("Codex MCP server `{name}` cannot define both `url` and `command`");
            }
            validate_codex_mcp_fields(
                entry,
                &[
                    &[
                        "url",
                        "http_headers",
                        "env_http_headers",
                        "bearer_token_env_var",
                    ],
                    CODEX_TUNING_KEYS,
                ]
                .concat(),
                name,
            )?;
            let mut headers = toml_string_map(entry, "http_headers", name)?;
            for (header, var) in toml_string_map(entry, "env_http_headers", name)? {
                headers.insert(header, format!("${{{var}}}"));
            }
            if let Some(var) = optional_toml_string(entry, "bearer_token_env_var", name)? {
                headers.insert("Authorization".to_string(), format!("Bearer ${{{var}}}"));
            }
            (McpTransport::Http { url, headers }, BTreeMap::new())
        } else if let Some(command) = command {
            validate_codex_mcp_fields(
                entry,
                &[&["command", "args", "env", "env_vars"], CODEX_TUNING_KEYS].concat(),
                name,
            )?;
            let args = toml_string_array(entry, "args", name)?;
            let mut env = toml_string_map(entry, "env", name)?;
            for var in codex_env_vars(entry, name)? {
                env.insert(var.clone(), format!("${{{var}}}"));
            }
            (McpTransport::Stdio { command, args }, env)
        } else {
            bail!("Codex MCP server `{name}` must define either `url` or `command`");
        };

        result.push(NormalizedMcpServer {
            name: name.clone(),
            transport,
            env,
        });
    }

    Ok(result)
}

fn optional_toml_string(
    entry: &toml::value::Table,
    field: &str,
    server_name: &str,
) -> Result<Option<String>> {
    entry
        .get(field)
        .map(|value| {
            let value = value.as_str().with_context(|| {
                format!("Codex MCP server `{server_name}` field `{field}` must be a string")
            })?;
            if value.trim().is_empty() {
                bail!("Codex MCP server `{server_name}` field `{field}` must not be empty");
            }
            Ok(value.to_string())
        })
        .transpose()
}

/// Codex server options that tune how Codex itself runs a server, with no
/// equivalent elsewhere: reading them changes nothing a target can express,
/// and the merge keeps them in `.codex/config.toml`.
const CODEX_TUNING_KEYS: &[&str] = &[
    "enabled",
    "required",
    "startup_timeout_sec",
    "startup_timeout_ms",
    "tool_timeout_sec",
    "enabled_tools",
    "disabled_tools",
    "default_tools_approval_mode",
    "tools",
    "scopes",
    "oauth_resource",
    "startup_readiness",
    "supports_parallel_tool_calls",
    "tool_input_schema_max_bytes",
    "omit_tools_from",
    "name",
    // Only `"local"` is accepted below: another environment is a remote
    // executor no other tool has.
    "environment_id",
];

/// The variables a stdio server forwards from Codex's environment
/// (`env_vars`): plain names, or `{ name, source = "local" }`. A variable read
/// from a remote executor has no equivalent in other tools.
fn codex_env_vars(entry: &toml::value::Table, server_name: &str) -> Result<Vec<String>> {
    let Some(value) = entry.get("env_vars") else {
        return Ok(Vec::new());
    };
    let unsupported = || {
        anyhow::anyhow!(
            "Codex MCP server `{server_name}` uses `env_vars` entries conforme cannot safely migrate without losing their semantics"
        )
    };
    value
        .as_array()
        .ok_or_else(unsupported)?
        .iter()
        .map(|item| match item {
            toml::Value::String(name) => Ok(name.clone()),
            toml::Value::Table(table)
                if table
                    .get("source")
                    .and_then(toml::Value::as_str)
                    .unwrap_or("local")
                    == "local" =>
            {
                table
                    .get("name")
                    .and_then(toml::Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(unsupported)
            }
            _ => Err(unsupported()),
        })
        .collect()
}

fn validate_codex_mcp_fields(
    entry: &toml::value::Table,
    allowed: &[&str],
    server_name: &str,
) -> Result<()> {
    for (field, value) in entry {
        let remote_environment = field == "environment_id" && value.as_str() != Some("local");
        if !allowed.contains(&field.as_str()) || remote_environment {
            bail!(
                "Codex MCP server `{server_name}` uses `{field}`, which conforme cannot safely migrate without losing its semantics"
            );
        }
    }
    Ok(())
}

fn toml_string_array(
    entry: &toml::value::Table,
    field: &str,
    server_name: &str,
) -> Result<Vec<String>> {
    let Some(value) = entry.get(field) else {
        return Ok(Vec::new());
    };
    let values = value.as_array().with_context(|| {
        format!("Codex MCP server `{server_name}` field `{field}` must be an array")
    })?;
    values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).with_context(|| {
                format!(
                    "Codex MCP server `{server_name}` field `{field}` must contain only strings"
                )
            })
        })
        .collect()
}

fn toml_string_map(
    entry: &toml::value::Table,
    field: &str,
    server_name: &str,
) -> Result<BTreeMap<String, String>> {
    let Some(value) = entry.get(field) else {
        return Ok(BTreeMap::new());
    };
    let values = value.as_table().with_context(|| {
        format!("Codex MCP server `{server_name}` field `{field}` must be a table")
    })?;
    values
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.clone(), value.to_string()))
                .with_context(|| {
                    format!(
                        "Codex MCP server `{server_name}` field `{field}.{key}` must be a string"
                    )
                })
        })
        .collect()
}

/// How a tool spells an environment-variable reference inside its MCP config
/// strings (`command`, `args`, `env` values, `url`, header values).
///
/// conforme keeps the Claude Code spelling, `${VAR}` / `${VAR:-default}`, in
/// its normalized config, and translates on the way in and out: a raw `${VAR}`
/// copied into Cursor or VS Code would never resolve there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvRefStyle {
    /// `${VAR}` and `${VAR:-default}` (Claude Code).
    Dollar,
    /// `$VAR`, `${VAR}` and `${VAR:-default}` (Gemini CLI).
    DollarOrBare,
    /// `${VAR}` with no default form (Kiro, Amp); a default is dropped.
    DollarNoDefault,
    /// `${env:VAR}` (Cursor, VS Code / Copilot, Zoo Code, Devin); a default is
    /// dropped, and predefined variables such as `${workspaceFolder}` are kept.
    EnvColon,
    /// `{env:VAR}` (OpenCode); a default is dropped.
    OpenCode,
    /// No interpolation (Zed, Codex): strings are copied verbatim.
    Literal,
}

/// Predefined variables of the `${env:VAR}` tools (Cursor, VS Code, Zoo). A
/// normalized `${workspaceFolder}` came from one of those tools and is not an
/// environment variable, so it is never rewritten to `${env:workspaceFolder}`.
const PREDEFINED_VARIABLES: &[&str] = &[
    "workspaceFolder",
    "workspaceFolderBasename",
    "userHome",
    "pathSeparator",
];

fn is_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Rewrite every `${…}` reference in `s` through `f`, which receives the text
/// between the braces and returns the replacement (or `None` to keep it).
fn rewrite_braced(s: &str, open: &str, f: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find(open) {
        let body_start = start + open.len();
        // The closing brace at the same depth, so a nested default such as
        // `${VAR:-${OTHER}}` is one reference.
        let mut depth = 1usize;
        let Some(len) = rest[body_start..].char_indices().find_map(|(i, c)| {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(i)
        }) else {
            break;
        };
        let body = &rest[body_start..body_start + len];
        out.push_str(&rest[..start]);
        match f(body) {
            Some(replacement) => out.push_str(&replacement),
            None => out.push_str(&rest[start..body_start + len + 1]),
        }
        rest = &rest[body_start + len + 1..];
    }
    out.push_str(rest);
    out
}

/// Translate a normalized string (`${VAR}` references) into `style`.
pub fn env_refs_to_tool(s: &str, style: EnvRefStyle) -> String {
    let split = |body: &str| -> Option<(String, Option<String>)> {
        let (name, default) = match body.split_once(":-") {
            Some((name, default)) => (name, Some(default.to_string())),
            None => (body, None),
        };
        is_var_name(name).then(|| (name.to_string(), default))
    };
    match style {
        EnvRefStyle::Dollar | EnvRefStyle::DollarOrBare | EnvRefStyle::Literal => s.to_string(),
        EnvRefStyle::DollarNoDefault => rewrite_braced(s, "${", |body| {
            split(body).map(|(n, _)| format!("${{{n}}}"))
        }),
        EnvRefStyle::EnvColon => rewrite_braced(s, "${", |body| {
            split(body)
                .filter(|(n, _)| !PREDEFINED_VARIABLES.contains(&n.as_str()))
                .map(|(n, _)| format!("${{env:{n}}}"))
        }),
        EnvRefStyle::OpenCode => rewrite_braced(s, "${", |body| {
            split(body).map(|(n, _)| format!("{{env:{n}}}"))
        }),
    }
}

/// Translate a string read from a tool written in `style` into the normalized
/// `${VAR}` spelling.
pub fn env_refs_from_tool(s: &str, style: EnvRefStyle) -> String {
    match style {
        EnvRefStyle::EnvColon => rewrite_braced(s, "${env:", |name| {
            is_var_name(name).then(|| format!("${{{name}}}"))
        }),
        EnvRefStyle::OpenCode => {
            // `{env:VAR}` never follows a `$`; leave `${env:…}` alone.
            let mut out = String::with_capacity(s.len());
            let mut rest = s;
            while let Some(start) = rest.find("{env:") {
                let Some(len) = rest[start + 5..].find('}') else {
                    break;
                };
                let name = &rest[start + 5..start + 5 + len];
                out.push_str(&rest[..start]);
                if is_var_name(name) && !out.ends_with('$') {
                    out.push_str(&format!("${{{name}}}"));
                } else {
                    out.push_str(&rest[start..start + 5 + len + 1]);
                }
                rest = &rest[start + 5 + len + 1..];
            }
            out.push_str(rest);
            out
        }
        EnvRefStyle::DollarOrBare => {
            // Gemini also expands a bare `$VAR`.
            let mut out = String::with_capacity(s.len());
            let mut i = 0;
            while let Some(c) = s[i..].chars().next() {
                if c == '$' {
                    let tail = &s[i + 1..];
                    let len = tail
                        .bytes()
                        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                        .count();
                    if is_var_name(&tail[..len]) {
                        out.push_str(&format!("${{{}}}", &tail[..len]));
                        i += 1 + len;
                        continue;
                    }
                }
                out.push(c);
                i += c.len_utf8();
            }
            out
        }
        EnvRefStyle::Dollar | EnvRefStyle::DollarNoDefault | EnvRefStyle::Literal => s.to_string(),
    }
}

/// Apply `f` to every string of a server that may hold a variable reference.
fn map_server_strings(
    server: &NormalizedMcpServer,
    f: impl Fn(&str) -> String,
) -> NormalizedMcpServer {
    let map = |m: &BTreeMap<String, String>| m.iter().map(|(k, v)| (k.clone(), f(v))).collect();
    NormalizedMcpServer {
        name: server.name.clone(),
        transport: match &server.transport {
            McpTransport::Stdio { command, args } => McpTransport::Stdio {
                command: f(command),
                args: args.iter().map(|a| f(a)).collect(),
            },
            McpTransport::Http { url, headers } => McpTransport::Http {
                url: f(url),
                headers: map(headers),
            },
        },
        env: map(&server.env),
    }
}

/// Rewrite the variable references of servers read from a tool written in
/// `style` into the normalized `${VAR}` spelling.
pub fn canonicalize_env_refs(
    servers: Vec<NormalizedMcpServer>,
    style: EnvRefStyle,
) -> Vec<NormalizedMcpServer> {
    if matches!(
        style,
        EnvRefStyle::Dollar | EnvRefStyle::DollarNoDefault | EnvRefStyle::Literal
    ) {
        return servers;
    }
    servers
        .iter()
        .map(|s| map_server_strings(s, |v| env_refs_from_tool(v, style)))
        .collect()
}

/// The JSON shape of one tool's MCP server entries.
struct ServerShape {
    /// `type` written on stdio entries (`None`: no `type` field).
    stdio_type: Option<&'static str>,
    /// `type` written on remote entries (`None`: no `type` field).
    http_type: Option<&'static str>,
    /// Key of a remote server's URL (`url`, or Gemini's `httpUrl`).
    url_key: &'static str,
    /// Extra key written on remote entries (Devin's `transport: "http"`).
    http_extra: Option<(&'static str, &'static str)>,
    /// Whether a remote entry may carry `env` (most tools document `env` for
    /// stdio servers only, and Zoo Code rejects it on remote ones).
    env_on_http: bool,
    env_refs: EnvRefStyle,
}

/// Claude Code `.mcp.json`: `type` is `stdio` or `http`; remote servers take
/// `url` + `headers`, no `env`.
const CLAUDE_SHAPE: ServerShape = ServerShape {
    stdio_type: Some("stdio"),
    http_type: Some("http"),
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    env_refs: EnvRefStyle::Dollar,
};

/// Cursor `.cursor/mcp.json`: local servers need `type: "stdio"`, remote ones
/// are just `url` (+ `headers`) with no `type`; references are `${env:VAR}`.
const CURSOR_SHAPE: ServerShape = ServerShape {
    stdio_type: Some("stdio"),
    http_type: None,
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    env_refs: EnvRefStyle::EnvColon,
};

/// Kiro `.kiro/settings/mcp.json`: remote servers also accept `env`.
const KIRO_SHAPE: ServerShape = ServerShape {
    stdio_type: Some("stdio"),
    http_type: Some("http"),
    url_key: "url",
    http_extra: None,
    env_on_http: true,
    env_refs: EnvRefStyle::DollarNoDefault,
};

/// VS Code `.vscode/mcp.json` (`servers` key): remote fields are `type`,
/// `url`, `headers`, `oauth` — no `env`.
const COPILOT_SHAPE: ServerShape = ServerShape {
    stdio_type: Some("stdio"),
    http_type: Some("http"),
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    env_refs: EnvRefStyle::EnvColon,
};

/// Zoo Code `.roo/mcp.json`: HTTP servers are `type: "streamable-http"` (a
/// bare `"http"` is not accepted), and Zoo's schema rejects `env` on a remote
/// entry.
const ZOOCODE_SHAPE: ServerShape = ServerShape {
    stdio_type: Some("stdio"),
    http_type: Some("streamable-http"),
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    env_refs: EnvRefStyle::EnvColon,
};

/// Zed `context_servers`: flat shape with no `type`; the remote variant has no
/// `env`, and Zed expands no variable references.
const ZED_SHAPE: ServerShape = ServerShape {
    stdio_type: None,
    http_type: None,
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    env_refs: EnvRefStyle::Literal,
};

/// Amp `amp.mcpServers`: transport inferred from the shape (`command`/`args`/
/// `env` locally, `url`/`headers` remotely), no `type`.
const AMP_SHAPE: ServerShape = ServerShape {
    stdio_type: None,
    http_type: None,
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    env_refs: EnvRefStyle::DollarNoDefault,
};

/// Gemini CLI `mcpServers`: no `type`, streamable HTTP servers use `httpUrl`.
const GEMINI_SHAPE: ServerShape = ServerShape {
    stdio_type: None,
    http_type: None,
    url_key: "httpUrl",
    http_extra: None,
    env_on_http: true,
    env_refs: EnvRefStyle::DollarOrBare,
};

/// Devin `.devin/mcp_config.json`: no `type`; remote servers are `url` with an
/// optional `transport` (`http` or `sse`) and `headers`, no `env`.
const DEVIN_SHAPE: ServerShape = ServerShape {
    stdio_type: None,
    http_type: None,
    url_key: "url",
    http_extra: Some(("transport", "http")),
    env_on_http: false,
    env_refs: EnvRefStyle::EnvColon,
};

fn string_map(map: &BTreeMap<String, String>) -> serde_json::Value {
    serde_json::Value::Object(
        map.iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect(),
    )
}

fn build_servers_object(
    servers: &[NormalizedMcpServer],
    shape: &ServerShape,
) -> serde_json::Map<String, serde_json::Value> {
    use serde_json::Value as Json;
    let mut out = serde_json::Map::new();

    for server in servers {
        let server = map_server_strings(server, |s| env_refs_to_tool(s, shape.env_refs));
        let mut entry = serde_json::Map::new();

        let is_http = match &server.transport {
            McpTransport::Stdio { command, args } => {
                if let Some(t) = shape.stdio_type {
                    entry.insert("type".to_string(), Json::String(t.to_string()));
                }
                entry.insert("command".to_string(), Json::String(command.clone()));
                entry.insert(
                    "args".to_string(),
                    Json::Array(args.iter().map(|a| Json::String(a.clone())).collect()),
                );
                false
            }
            McpTransport::Http { url, headers } => {
                if let Some(t) = shape.http_type {
                    entry.insert("type".to_string(), Json::String(t.to_string()));
                }
                entry.insert(shape.url_key.to_string(), Json::String(url.clone()));
                if let Some((key, value)) = shape.http_extra {
                    entry.insert(key.to_string(), Json::String(value.to_string()));
                }
                if !headers.is_empty() {
                    entry.insert("headers".to_string(), string_map(headers));
                }
                true
            }
        };

        if !server.env.is_empty() && (shape.env_on_http || !is_http) {
            entry.insert("env".to_string(), string_map(&server.env));
        }

        out.insert(server.name.clone(), Json::Object(entry));
    }

    out
}

#[cfg(test)]
fn wrap_servers(key: &str, servers: serde_json::Map<String, serde_json::Value>) -> Result<String> {
    if servers.is_empty() {
        return Ok(String::new());
    }
    let mut root = serde_json::Map::new();
    root.insert(key.to_string(), serde_json::Value::Object(servers));
    serde_json::to_string_pretty(&serde_json::Value::Object(root))
        .context("failed to serialize MCP config")
}

/// Generate a fresh `.mcp.json` (Claude Code format) from normalized MCP
/// servers: `{ "mcpServers": { "name": { ... } } }`. The adapter merges into
/// an existing file through [`build_claude_servers_object`].
#[cfg(test)]
pub fn generate_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    wrap_servers("mcpServers", build_claude_servers_object(servers))
}

/// Build Claude Code's `mcpServers` object of `.mcp.json`.
pub fn build_claude_servers_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    build_servers_object(servers, &CLAUDE_SHAPE)
}

/// Build Cursor's `mcpServers` object of `.cursor/mcp.json`.
pub fn build_cursor_servers_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    build_servers_object(servers, &CURSOR_SHAPE)
}

/// Build Kiro's `mcpServers` object of `.kiro/settings/mcp.json`.
pub fn build_kiro_servers_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    build_servers_object(servers, &KIRO_SHAPE)
}

/// Generate Zoo Code's `.roo/mcp.json` (a fresh file; the adapter merges into
/// an existing one through [`build_zoocode_servers_object`]).
#[cfg(test)]
pub fn generate_zoocode_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    wrap_servers("mcpServers", build_zoocode_servers_object(servers))
}

/// Build Zoo Code's `mcpServers` object (see [`ZOOCODE_SHAPE`]).
pub fn build_zoocode_servers_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    build_servers_object(servers, &ZOOCODE_SHAPE)
}

/// Generate Copilot VS Code MCP format (uses `servers` key, not `mcpServers`).
#[cfg(test)]
pub fn generate_copilot_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    wrap_servers("servers", build_copilot_servers_object(servers))
}

/// Build the Copilot `servers` object of `.vscode/mcp.json` (the adapter
/// merges it so the file's `inputs` and `sandbox` keys survive).
pub fn build_copilot_servers_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    build_servers_object(servers, &COPILOT_SHAPE)
}

/// Build the OpenCode `mcp` object (not a full file — `opencode.json` is merged by the adapter).
/// OpenCode format: stdio uses `command: [cmd, ...args]` as a single array;
/// env var key is `environment` (not `env`), which only local servers accept;
/// remote servers use `url`. Variable references are written `{env:VAR}`.
pub fn build_opencode_mcp_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    let mut mcp = serde_json::Map::new();

    for server in servers {
        let server = map_server_strings(server, |s| env_refs_to_tool(s, EnvRefStyle::OpenCode));
        let mut entry = serde_json::Map::new();

        match &server.transport {
            McpTransport::Stdio { command, args } => {
                entry.insert(
                    "type".to_string(),
                    serde_json::Value::String("local".to_string()),
                );
                let mut combined: Vec<serde_json::Value> = Vec::with_capacity(args.len() + 1);
                combined.push(serde_json::Value::String(command.clone()));
                combined.extend(args.iter().map(|a| serde_json::Value::String(a.clone())));
                entry.insert("command".to_string(), serde_json::Value::Array(combined));
                if !server.env.is_empty() {
                    entry.insert("environment".to_string(), string_map(&server.env));
                }
            }
            McpTransport::Http { url, headers } => {
                entry.insert(
                    "type".to_string(),
                    serde_json::Value::String("remote".to_string()),
                );
                entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
                if !headers.is_empty() {
                    entry.insert("headers".to_string(), string_map(headers));
                }
            }
        }

        mcp.insert(server.name.clone(), serde_json::Value::Object(entry));
    }

    mcp
}

/// Parse the OpenCode `mcp` object from an `opencode.json` value back into
/// normalized servers. OpenCode's shape is unique enough that the generic
/// [`parse_mcp_json`] reader cannot handle it: `type` is `local`/`remote`
/// (not `stdio`/`http`), `command` is a single `[cmd, ...args]` array, and the
/// env key is `environment`.
pub fn parse_opencode_mcp_object(mcp: &serde_json::Value) -> Vec<NormalizedMcpServer> {
    let Some(obj) = mcp.as_object() else {
        return Vec::new();
    };

    let mut result = Vec::new();
    for (name, value) in obj {
        // `{ "enabled": false }` only toggles a server defined elsewhere (the
        // global config); the merge keeps it as is.
        if !crate::json_settings::is_expressible_server(value) {
            continue;
        }
        let Some(entry) = value.as_object() else {
            continue;
        };

        let server_type = entry.get("type").and_then(|v| v.as_str());
        let url = entry.get("url").and_then(|v| v.as_str());
        let is_remote = server_type == Some("remote") || (server_type.is_none() && url.is_some());

        let transport = if is_remote {
            let headers = entry
                .get("headers")
                .and_then(|v| v.as_object())
                .map(|h| {
                    h.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            McpTransport::Http {
                url: url.unwrap_or("").to_string(),
                headers,
            }
        } else {
            // `command` is a single array whose first element is the executable.
            let parts: Vec<String> = entry
                .get("command")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let mut parts = parts.into_iter();
            McpTransport::Stdio {
                command: parts.next().unwrap_or_default(),
                args: parts.collect(),
            }
        };

        let env: BTreeMap<String, String> = entry
            .get("environment")
            .and_then(|v| v.as_object())
            .map(|e| {
                e.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        result.push(NormalizedMcpServer {
            name: name.clone(),
            transport,
            env,
        });
    }

    result
}

/// OpenCode's built-in agents: an `agent.<name>` entry for one of them
/// overrides its settings rather than defining a new agent.
const OPENCODE_BUILTIN_AGENTS: &[&str] = &[
    "build",
    "plan",
    "general",
    "explore",
    "compaction",
    "title",
    "summary",
];

/// Whether an agent of this name would be one of OpenCode's built-in agents
/// (in `opencode.json` or as `.opencode/agents/<name>.md`): written there it
/// would override, and demote to a subagent, OpenCode's own Build or Plan.
pub fn is_opencode_builtin_agent(name: &str) -> bool {
    OPENCODE_BUILTIN_AGENTS.contains(&crate::config::sanitize_name(name).as_str())
}

/// Parse the OpenCode `agent` object from an `opencode.json` value back into
/// normalized agents (the inverse of [`build_opencode_agent_object`]).
pub fn parse_opencode_agent_object(
    agent: &serde_json::Value,
) -> Vec<crate::config::NormalizedAgent> {
    let Some(obj) = agent.as_object() else {
        return Vec::new();
    };

    let mut result = Vec::new();
    for (name, value) in obj {
        let Some(entry) = value.as_object() else {
            continue;
        };
        // Overrides of OpenCode's built-in agents, and entries that only tune
        // an agent defined elsewhere (no prompt, no description), are not
        // agents another tool could load.
        let has_text = |key: &str| {
            entry
                .get(key)
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.trim().is_empty())
        };
        if OPENCODE_BUILTIN_AGENTS.contains(&name.as_str())
            || !(has_text("prompt") || has_text("description"))
        {
            continue;
        }
        result.push(crate::config::NormalizedAgent {
            name: name.clone(),
            description: entry
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            content: entry
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            model: entry
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            ..Default::default()
        });
    }

    result
}

// Keys conforme may write into a server entry, per tool, across every
// transport. When a server is re-synced these are regenerated; any other key
// an existing entry carries is tool-specific configuration the user (or the
// tool itself) added, and `json_settings::merge_server_entries` keeps it.
// Enable/disable flags are listed so a sync re-enables the server, matching
// the Codex merge: leaving it disabled would make `check` pass while the tool
// still hides the server.
pub const CLAUDE_OWNED_SERVER_KEYS: &[&str] = &["type", "command", "args", "env", "url", "headers"];
pub const CURSOR_OWNED_SERVER_KEYS: &[&str] = &["type", "command", "args", "env", "url", "headers"];
pub const KIRO_OWNED_SERVER_KEYS: &[&str] = &[
    "type", "command", "args", "env", "url", "headers", "disabled",
];
pub const ZED_OWNED_SERVER_KEYS: &[&str] = &["command", "args", "env", "url", "headers", "enabled"];
pub const AMP_OWNED_SERVER_KEYS: &[&str] = &["command", "args", "env", "url", "headers"];
// `type` is listed because `gemini mcp add -t http|sse` writes one next to
// `url`; left in place it would contradict the `httpUrl` conforme writes.
pub const GEMINI_OWNED_SERVER_KEYS: &[&str] = &[
    "type", "command", "args", "env", "url", "httpUrl", "headers",
];
pub const OPENCODE_OWNED_SERVER_KEYS: &[&str] = &[
    "type",
    "command",
    "environment",
    "url",
    "headers",
    "enabled",
];
pub const COPILOT_OWNED_SERVER_KEYS: &[&str] =
    &["type", "command", "args", "env", "url", "headers"];
pub const ZOOCODE_OWNED_SERVER_KEYS: &[&str] = &[
    "type", "command", "args", "env", "url", "headers", "disabled",
];
pub const DEVIN_OWNED_SERVER_KEYS: &[&str] = &[
    "command",
    "args",
    "env",
    "url",
    "serverUrl",
    "transport",
    "headers",
    "disabled",
];
/// Keys conforme writes into an OpenCode `agent.<name>` entry. Anything else
/// (`permission`, `temperature`, `steps`, `color`, …) is the user's.
pub const OPENCODE_OWNED_AGENT_KEYS: &[&str] = &["description", "mode", "model", "prompt"];

/// Build the Zed `context_servers` object (not a full file — `.zed/settings.json`
/// is merged by the adapter so user-authored settings such as theme and
/// keybindings are preserved). Flat shape: stdio uses `command`/`args`/`env`,
/// remote uses `url`/`headers`; no `type` field.
pub fn build_zed_context_servers_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    build_servers_object(servers, &ZED_SHAPE)
}

/// Build the Amp `amp.mcpServers` object (not a full file — `.amp/settings.json`
/// is merged by the adapter so user-authored workspace settings are preserved).
/// Amp infers the transport from the entry's shape: stdio uses `command`/`args`,
/// remote uses `url` (+ optional `headers`). No `type` field is emitted.
pub fn build_amp_mcp_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    build_servers_object(servers, &AMP_SHAPE)
}

/// Build the Gemini CLI `mcpServers` object (not a full file — `.gemini/settings.json`
/// is merged by the adapter so user-authored settings are preserved).
/// Gemini does NOT use a `type` field and uses `httpUrl` for HTTP servers.
pub fn build_gemini_mcp_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    build_servers_object(servers, &GEMINI_SHAPE)
}

/// Build Devin's `mcpServers` object of `.devin/mcp_config.json` (merged by
/// the adapter so per-server OAuth settings and `disabled` flags survive).
pub fn build_devin_servers_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    build_servers_object(servers, &DEVIN_SHAPE)
}

/// Build the OpenCode `agent` object for `opencode.json` (merged by the adapter).
pub fn build_opencode_agent_object(
    agents: &[crate::config::NormalizedAgent],
) -> serde_json::Map<String, serde_json::Value> {
    let mut agent_map = serde_json::Map::new();

    for agent in agents {
        let mut entry = serde_json::Map::new();
        entry.insert(
            "description".to_string(),
            serde_json::Value::String(crate::skills::description_or_name(
                &agent.description,
                &agent.name,
            )),
        );
        entry.insert(
            "mode".to_string(),
            serde_json::Value::String("subagent".to_string()),
        );
        if let Some(model) = crate::skills::opencode_model(agent.model.as_deref()) {
            entry.insert(
                "model".to_string(),
                serde_json::Value::String(model.to_string()),
            );
        }
        if !agent.content.is_empty() {
            entry.insert(
                "prompt".to_string(),
                serde_json::Value::String(agent.content.clone()),
            );
        }

        agent_map.insert(
            crate::config::sanitize_name(&agent.name),
            serde_json::Value::Object(entry),
        );
    }

    agent_map
}

/// Merge freshly generated OpenCode agents into the `agent` object on disk.
///
/// Unlike MCP servers, `agent` is shared with the user: it is where OpenCode
/// documents overrides of its built-in agents (`build`, `plan`), per-agent
/// `permission`, and agents written by hand. So every existing entry the
/// source does not define is kept (an agent removed from the source stays
/// here until deleted by hand; its `.opencode/agents/<name>.md` is cleaned).
/// For a synced agent, `description`, `mode` and `prompt` are replaced,
/// `model` only when the source has one OpenCode can use, and the user's own
/// keys (`permission`, `temperature`, …) are kept.
pub fn merge_opencode_agents(
    existing: Option<&serde_json::Value>,
    generated: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut merged = existing
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    for (name, entry) in generated {
        let serde_json::Value::Object(new) = entry else {
            continue;
        };
        let mut agent = match merged.remove(&name) {
            Some(serde_json::Value::Object(old)) => old,
            _ => serde_json::Map::new(),
        };
        for key in OPENCODE_OWNED_AGENT_KEYS {
            if *key != "model" {
                agent.remove(*key);
            }
        }
        agent.extend(new);
        merged.insert(name, serde_json::Value::Object(agent));
    }
    merged
}

/// Parse an MCP config file into normalized servers. Handles every key conforme
/// emits — `mcpServers` (standard), `servers` (Copilot/VS Code),
/// `context_servers` (Zed) and `amp.mcpServers` (Amp) — and infers the transport
/// from the entry's shape when there is no explicit `type` field (Gemini
/// `httpUrl`, Zed/Amp remote `url`; a Devin Desktop-style `serverUrl` is also
/// accepted for hand-written files).
pub fn parse_mcp_json(content: &str) -> Result<Vec<NormalizedMcpServer>> {
    // JSONC-tolerant: Zed, VS Code and Zoo Code settings may hold comments
    // and trailing commas.
    let root = crate::json_settings::parse_jsonc(content).context("failed to parse MCP JSON")?;

    let servers_key = if root.get("mcpServers").is_some() {
        "mcpServers"
    } else if root.get("servers").is_some() {
        "servers"
    } else if root.get("context_servers").is_some() {
        "context_servers"
    } else if root.get("amp.mcpServers").is_some() {
        "amp.mcpServers"
    } else {
        return Ok(Vec::new());
    };

    let servers_obj = root[servers_key]
        .as_object()
        .unwrap_or(&serde_json::Map::new())
        .clone();

    let mut result = Vec::new();
    for (name, value) in servers_obj {
        // Zed extension servers and Claude SDK servers have no portable form;
        // the merge keeps them in place on the target side.
        if !crate::json_settings::is_expressible_server(&value) {
            continue;
        }
        let obj = value.as_object();
        let Some(obj) = obj else { continue };

        let transport_type = obj.get("type").and_then(|v| v.as_str());
        let url_value = obj
            .get("url")
            .or_else(|| obj.get("httpUrl"))
            .or_else(|| obj.get("serverUrl"))
            .and_then(|v| v.as_str());
        // An entry is HTTP if it declares a remote transport type OR — when no
        // `type` is present — if it carries a URL rather than a command.
        let is_http = matches!(
            transport_type,
            Some("http") | Some("https") | Some("sse") | Some("streamable-http") | Some("ws")
        ) || (transport_type.is_none() && url_value.is_some());

        let transport = if is_http {
            let url = url_value.unwrap_or("").to_string();
            let headers = obj
                .get("headers")
                .and_then(|v| v.as_object())
                .map(|h| {
                    h.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            McpTransport::Http { url, headers }
        } else {
            let command = obj
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = obj
                .get("args")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            McpTransport::Stdio { command, args }
        };

        let env: BTreeMap<String, String> = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|e| {
                e.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        result.push(NormalizedMcpServer {
            name,
            transport,
            env,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_codex_mcp_toml_preserves_existing_config() {
        let existing = r#"# User comment
model = "gpt-test"

[mcp_servers.existing]
url = "https://existing.example/mcp"
startup_timeout_sec = 20
"#;
        let servers = vec![NormalizedMcpServer {
            name: "filesystem".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
            },
            env: BTreeMap::from([("ROOT".to_string(), "/workspace".to_string())]),
        }];

        let result = merge_codex_mcp_toml(existing, &servers).unwrap();

        assert!(result.contains("# User comment"));
        assert!(result.contains("model = \"gpt-test\""));
        assert!(result.contains("[mcp_servers.existing]"));
        assert!(result.contains("startup_timeout_sec = 20"));
        assert!(result.contains("[mcp_servers.filesystem]"));
        assert!(result.contains("command = \"npx\""));
        assert!(result.contains("[mcp_servers.filesystem.env]"));
        assert!(result.contains("ROOT = \"/workspace\""));
    }

    #[test]
    fn test_codex_mcp_toml_roundtrip() {
        let servers = vec![
            NormalizedMcpServer {
                name: "filesystem".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
                },
                env: BTreeMap::from([("ROOT".to_string(), "/workspace".to_string())]),
            },
            NormalizedMcpServer {
                name: "api".to_string(),
                transport: McpTransport::Http {
                    url: "https://example.com/mcp".to_string(),
                    headers: BTreeMap::from([("X-Region".to_string(), "eu".to_string())]),
                },
                env: BTreeMap::new(),
            },
        ];

        let toml = merge_codex_mcp_toml("", &servers).unwrap();
        let parsed = parse_codex_mcp_toml(&toml).unwrap();

        assert_eq!(parsed.len(), 2);
        let filesystem = parsed
            .iter()
            .find(|server| server.name == "filesystem")
            .unwrap();
        assert_eq!(
            filesystem.env.get("ROOT").map(String::as_str),
            Some("/workspace")
        );
        let api = parsed.iter().find(|server| server.name == "api").unwrap();
        match &api.transport {
            McpTransport::Http { url, headers } => {
                assert_eq!(url, "https://example.com/mcp");
                assert_eq!(headers.get("X-Region").map(String::as_str), Some("eu"));
            }
            other => panic!("expected HTTP transport, got {other:?}"),
        }
    }

    #[test]
    fn test_merge_codex_mcp_toml_supports_inline_tables() {
        let existing = r#"model = "gpt-test"
mcp_servers = { existing = { command = "printf", args = ["ok"] } }
"#;
        let servers = vec![NormalizedMcpServer {
            name: "filesystem".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
            },
            env: BTreeMap::new(),
        }];

        let result = merge_codex_mcp_toml(existing, &servers).unwrap();
        let parsed = parse_codex_mcp_toml(&result).unwrap();

        assert!(result.contains("model = \"gpt-test\""));
        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().any(|server| server.name == "existing"));
        assert!(parsed.iter().any(|server| server.name == "filesystem"));
    }

    #[test]
    fn test_merge_codex_mcp_toml_enables_synced_server() {
        let existing = r#"[mcp_servers.filesystem]
command = "old-command"
enabled = false
startup_timeout_sec = 20
"#;
        let servers = vec![NormalizedMcpServer {
            name: "filesystem".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["server-filesystem".to_string()],
            },
            env: BTreeMap::new(),
        }];

        let result = merge_codex_mcp_toml(existing, &servers).unwrap();
        assert!(!result.contains("enabled = false"));
        assert!(result.contains("startup_timeout_sec = 20"));
        assert!(result.contains("command = \"npx\""));
    }

    #[test]
    fn test_merge_codex_mcp_toml_cleans_fields_when_transport_changes() {
        let existing_http = r#"[mcp_servers.server]
url = "https://example.com/mcp"
auth = "oauth"
bearer_token_env_var = "TOKEN"
bearer_token = "legacy"
env_http_headers = { Authorization = "TOKEN" }
http_headers_helper = "./headers.sh"
oauth = { client_id = "conforme" }
oauth_resource = "https://example.com"
"#;
        let stdio_server = NormalizedMcpServer {
            name: "server".to_string(),
            transport: McpTransport::Stdio {
                command: "node".to_string(),
                args: vec!["server.js".to_string()],
            },
            env: BTreeMap::new(),
        };

        let stdio_result = merge_codex_mcp_toml(existing_http, &[stdio_server]).unwrap();
        // Codex rejects each of these on a stdio server.
        for key in [
            "auth",
            "bearer_token",
            "env_http_headers",
            "http_headers_helper",
            "oauth",
        ] {
            assert!(!stdio_result.contains(key), "{key} kept:\n{stdio_result}");
        }
        assert_eq!(parse_codex_mcp_toml(&stdio_result).unwrap().len(), 1);

        let existing_stdio = r#"[mcp_servers.server]
command = "node"
cwd = "tools"
env_vars = ["TOKEN"]
"#;
        let http_server = NormalizedMcpServer {
            name: "server".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::new(),
        };

        let http_result = merge_codex_mcp_toml(existing_stdio, &[http_server]).unwrap();
        assert!(!http_result.contains("cwd"));
        assert!(!http_result.contains("env_vars"));
        assert_eq!(parse_codex_mcp_toml(&http_result).unwrap().len(), 1);
    }

    #[test]
    fn test_parse_codex_mcp_toml_skips_disabled_servers() {
        let content = r#"[mcp_servers.disabled]
command = "npx"
enabled = false

[mcp_servers.enabled]
command = "node"
"#;

        let parsed = parse_codex_mcp_toml(content).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "enabled");
    }

    fn vibe_servers() -> Vec<NormalizedMcpServer> {
        vec![
            NormalizedMcpServer {
                name: "files".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
                },
                env: BTreeMap::from([("ROOT".to_string(), "/workspace".to_string())]),
            },
            NormalizedMcpServer {
                name: "github".to_string(),
                transport: McpTransport::Http {
                    url: "https://api.githubcopilot.com/mcp/".to_string(),
                    headers: BTreeMap::from([
                        (
                            "Authorization".to_string(),
                            "Bearer ${GITHUB_TOKEN}".to_string(),
                        ),
                        ("X-Region".to_string(), "eu".to_string()),
                    ]),
                },
                env: BTreeMap::new(),
            },
        ]
    }

    #[test]
    fn test_vibe_mcp_toml_round_trips() {
        // Vibe expands no `${VAR}`: the bearer token is read from the named
        // variable through a static `auth`, never written as text.
        let toml = merge_vibe_mcp_toml("", &vibe_servers()).unwrap();
        assert!(toml.contains("[[mcp_servers]]"), "{toml}");
        assert!(toml.contains("transport = \"streamable-http\""), "{toml}");
        assert!(toml.contains("api_key_env = \"GITHUB_TOKEN\""), "{toml}");
        assert!(!toml.contains("${GITHUB_TOKEN}"), "{toml}");
        assert_eq!(parse_vibe_mcp_toml(&toml).unwrap(), vibe_servers());
    }

    #[test]
    fn test_merge_vibe_mcp_toml_keeps_the_users_settings() {
        let existing = r#"# my Vibe settings
active_model = "devstral"

[[mcp_servers]]
name = "files"
transport = "stdio"
command = "old"
disabled = true
tool_timeout_sec = 90

[[mcp_servers]]
name = "mine"
transport = "stdio"
command = "my-server"

[[mcp_servers]]
name = "github"
transport = "streamable-http"
url = "https://old.example.com/mcp"
auth = { type = "oauth", scopes = ["repo"] }
"#;
        let servers = vec![
            vibe_servers().remove(0),
            NormalizedMcpServer {
                name: "github".to_string(),
                transport: McpTransport::Http {
                    url: "https://api.githubcopilot.com/mcp/".to_string(),
                    headers: BTreeMap::new(),
                },
                env: BTreeMap::new(),
            },
        ];

        let toml = merge_vibe_mcp_toml(existing, &servers).unwrap();

        assert!(toml.contains("# my Vibe settings"), "{toml}");
        assert!(toml.contains("active_model = \"devstral\""), "{toml}");
        assert!(toml.contains("tool_timeout_sec = 90"), "{toml}");
        assert!(!toml.contains("disabled = true"), "{toml}");
        assert!(toml.contains("command = \"npx\""), "{toml}");
        assert!(toml.contains("name = \"mine\""), "{toml}");
        // An OAuth login the user set up survives a server without headers.
        assert!(toml.contains("type = \"oauth\""), "{toml}");
        assert!(
            toml.contains("https://api.githubcopilot.com/mcp/"),
            "{toml}"
        );
        assert_eq!(parse_vibe_mcp_toml(&toml).unwrap().len(), 3);
    }

    #[test]
    fn test_parse_vibe_mcp_toml_reads_command_lists_and_skips_disabled() {
        let toml = r#"[[mcp_servers]]
name = "cmd"
transport = "stdio"
command = ["uvx", "server", "--flag"]

[[mcp_servers]]
name = "off"
transport = "stdio"
command = "x"
disabled = true
"#;
        let parsed = parse_vibe_mcp_toml(toml).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].transport,
            McpTransport::Stdio {
                command: "uvx".to_string(),
                args: vec!["server".to_string(), "--flag".to_string()],
            }
        );
    }

    #[test]
    fn test_codex_env_references_use_codex_env_keys() {
        // Codex expands no `${VAR}` in config.toml: a reference must become
        // `env_vars` (stdio), `bearer_token_env_var` or `env_http_headers`.
        let servers = vec![
            NormalizedMcpServer {
                name: "fs".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec![],
                },
                env: BTreeMap::from([
                    ("ROOT".to_string(), "${ROOT}".to_string()),
                    ("MODE".to_string(), "fast".to_string()),
                ]),
            },
            NormalizedMcpServer {
                name: "github".to_string(),
                transport: McpTransport::Http {
                    url: "https://api.githubcopilot.com/mcp/".to_string(),
                    headers: BTreeMap::from([
                        (
                            "Authorization".to_string(),
                            "Bearer ${GITHUB_TOKEN}".to_string(),
                        ),
                        ("X-Api-Key".to_string(), "${API_KEY}".to_string()),
                        ("X-Region".to_string(), "eu".to_string()),
                    ]),
                },
                env: BTreeMap::new(),
            },
        ];

        let toml = merge_codex_mcp_toml("", &servers).unwrap();
        assert!(!toml.contains("${"), "{toml}");
        let root: toml::Value = toml::from_str(&toml).unwrap();
        let fs = &root["mcp_servers"]["fs"];
        assert_eq!(fs["env_vars"], toml::Value::Array(vec!["ROOT".into()]));
        assert_eq!(fs["env"]["MODE"].as_str(), Some("fast"));
        assert!(fs["env"].get("ROOT").is_none());
        let github = &root["mcp_servers"]["github"];
        assert_eq!(
            github["bearer_token_env_var"].as_str(),
            Some("GITHUB_TOKEN")
        );
        assert_eq!(
            github["env_http_headers"]["X-Api-Key"].as_str(),
            Some("API_KEY")
        );
        assert_eq!(github["http_headers"]["X-Region"].as_str(), Some("eu"));
        assert!(github["http_headers"].get("Authorization").is_none());

        let parsed = parse_codex_mcp_toml(&toml).unwrap();
        let by_name = |n: &str| parsed.iter().find(|s| s.name == n).unwrap().clone();
        assert_eq!(by_name("fs"), servers[0]);
        assert_eq!(by_name("github"), servers[1]);
    }

    #[test]
    fn test_parse_codex_mcp_toml_ignores_tuning_keys() {
        let content = r#"[mcp_servers.fs]
command = "npx"
startup_timeout_sec = 20
tool_timeout_sec = 120
required = true
enabled_tools = ["read"]
disabled_tools = ["write"]
default_tools_approval_mode = "approve"

[mcp_servers.api]
url = "https://example.com/mcp"
startup_timeout_ms = 20000
scopes = ["read"]
oauth_resource = "https://example.com"
startup_readiness = "lazy"
supports_parallel_tool_calls = true
tool_input_schema_max_bytes = 4096
omit_tools_from = ["review"]
name = "api"
environment_id = "local"
"#;
        let parsed = parse_codex_mcp_toml(content).unwrap();
        assert_eq!(parsed.len(), 2);

        // A server bound to a remote environment has no portable form.
        let remote = "[mcp_servers.x]\ncommand = \"node\"\nenvironment_id = \"devbox\"\n";
        let error = parse_codex_mcp_toml(remote).unwrap_err();
        assert!(
            error.to_string().contains("cannot safely migrate"),
            "{error}"
        );
    }

    #[test]
    fn test_parse_codex_mcp_toml_rejects_unrepresentable_stdio_options() {
        for field in [
            "cwd = \"tools\"",
            "env_vars = [{ name = \"TOKEN\", source = \"remote\" }]",
        ] {
            let content = format!("[mcp_servers.private]\ncommand = \"node\"\n{field}\n");
            let error = parse_codex_mcp_toml(&content).unwrap_err();

            assert!(
                error.to_string().contains("cannot safely migrate"),
                "{error}"
            );
        }
    }

    #[test]
    fn test_parse_codex_mcp_toml_rejects_transport_incompatible_fields() {
        let http_with_args =
            "[mcp_servers.broken]\nurl = \"https://example.com/mcp\"\nargs = [\"one\"]\n";
        let stdio_with_headers =
            "[mcp_servers.broken]\ncommand = \"node\"\nhttp_headers = { X = \"one\" }\n";

        let http_error = parse_codex_mcp_toml(http_with_args).unwrap_err();
        let stdio_error = parse_codex_mcp_toml(stdio_with_headers).unwrap_err();

        assert!(http_error.to_string().contains("cannot safely migrate"));
        assert!(http_error.to_string().contains("args"));
        assert!(stdio_error.to_string().contains("cannot safely migrate"));
        assert!(stdio_error.to_string().contains("http_headers"));
    }

    #[test]
    fn test_merge_codex_mcp_toml_drops_env_on_http() {
        // Codex rejects `env` on an HTTP server; it is dropped, as every JSON
        // shape without `env` on remote servers does, instead of aborting the
        // whole sync.
        let server = NormalizedMcpServer {
            name: "api".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::from([("TOKEN".to_string(), "secret".to_string())]),
        };

        let toml = merge_codex_mcp_toml("", &[server]).unwrap();

        assert!(!toml.contains("TOKEN"), "{toml}");
        assert_eq!(parse_codex_mcp_toml(&toml).unwrap().len(), 1);
    }

    #[test]
    fn test_parse_codex_mcp_toml_rejects_malformed_server() {
        let missing_transport = "[mcp_servers.broken]\nargs = [\"one\"]\n";
        let mixed_args = "[mcp_servers.broken]\ncommand = \"npx\"\nargs = [\"one\", 2]\n";
        let invalid_root = "mcp_servers = \"broken\"\n";
        let invalid_entry = "[mcp_servers]\nbroken = \"not a table\"\n";
        let blank_command = "[mcp_servers.broken]\ncommand = \"   \"\n";

        let missing_error = parse_codex_mcp_toml(missing_transport).unwrap_err();
        let args_error = parse_codex_mcp_toml(mixed_args).unwrap_err();
        let root_error = parse_codex_mcp_toml(invalid_root).unwrap_err();
        let entry_error = parse_codex_mcp_toml(invalid_entry).unwrap_err();
        let blank_error = parse_codex_mcp_toml(blank_command).unwrap_err();

        assert!(missing_error
            .to_string()
            .contains("either `url` or `command`"));
        assert!(args_error.to_string().contains("only strings"));
        assert!(root_error
            .to_string()
            .contains("`mcp_servers` must be a table"));
        assert!(entry_error
            .to_string()
            .contains("server `broken` must be a table"));
        assert!(blank_error.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_generate_mcp_json_stdio() {
        let servers = vec![NormalizedMcpServer {
            name: "filesystem".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
            },
            env: BTreeMap::new(),
        }];
        let result = generate_mcp_json(&servers).unwrap();
        assert!(result.contains("mcpServers"));
        assert!(result.contains("filesystem"));
        assert!(result.contains("npx"));
    }

    #[test]
    fn test_generate_mcp_json_http() {
        let servers = vec![NormalizedMcpServer {
            name: "github".to_string(),
            transport: McpTransport::Http {
                url: "https://api.github.com/mcp".to_string(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::new(),
        }];
        let result = generate_mcp_json(&servers).unwrap();
        assert!(result.contains("http"));
        assert!(result.contains("api.github.com"));
    }

    #[test]
    fn test_generate_zoocode_mcp_json_http_uses_streamable_http() {
        let servers = vec![NormalizedMcpServer {
            name: "remote".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: BTreeMap::from([("x-api-key".to_string(), "secret".to_string())]),
            },
            env: BTreeMap::new(),
        }];
        let result = generate_zoocode_mcp_json(&servers).unwrap();
        // Zoo Code requires `streamable-http`, never a bare `http` type value.
        assert!(result.contains("\"type\": \"streamable-http\""));
        assert!(!result.contains("\"type\": \"http\""));
        assert!(result.contains("mcpServers"));
        assert!(result.contains("https://example.com/mcp"));
        assert!(result.contains("x-api-key"));
    }

    #[test]
    fn test_zoocode_remote_server_carries_no_env() {
        // Zoo Code's schema requires `env` to be absent on sse/streamable-http
        // entries and rejects the server otherwise; stdio keeps its env.
        let env = BTreeMap::from([("TOKEN".to_string(), "x".to_string())]);
        let servers = vec![
            NormalizedMcpServer {
                name: "remote".to_string(),
                transport: McpTransport::Http {
                    url: "https://example.com/mcp".to_string(),
                    headers: BTreeMap::new(),
                },
                env: env.clone(),
            },
            NormalizedMcpServer {
                name: "local".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec![],
                },
                env,
            },
        ];
        let servers = build_zoocode_servers_object(&servers);
        assert!(servers["remote"].get("env").is_none());
        assert_eq!(servers["local"]["env"]["TOKEN"], "x");
    }

    #[test]
    fn test_generate_zoocode_mcp_json_stdio_matches_standard() {
        // For stdio servers Zoo Code uses the same shape as the standard format.
        let servers = vec![NormalizedMcpServer {
            name: "fs".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/fs".to_string()],
            },
            env: BTreeMap::new(),
        }];
        assert_eq!(
            generate_zoocode_mcp_json(&servers).unwrap(),
            generate_mcp_json(&servers).unwrap()
        );
    }

    #[test]
    fn test_parse_mcp_json_ws_transport() {
        // A WebSocket MCP server (`type: "ws"`) must parse as an HTTP transport,
        // not fall through to a broken stdio server with an empty command.
        let json = r#"{
            "mcpServers": {
                "socket": { "type": "ws", "url": "wss://example.com/mcp" }
            }
        }"#;
        let parsed = parse_mcp_json(json).unwrap();
        assert_eq!(parsed.len(), 1);
        match &parsed[0].transport {
            McpTransport::Http { url, .. } => assert_eq!(url, "wss://example.com/mcp"),
            other => panic!("expected HTTP transport for ws, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_opencode_mcp_object_skips_toggle_entries() {
        // `mcp.<name>` may be just `{ "enabled": false }`, toggling a server
        // defined in the global config.
        let mcp = serde_json::json!({
            "github": {"enabled": false},
            "fs": {"type": "local", "command": ["npx", "-y", "fs"]}
        });
        let parsed = parse_opencode_mcp_object(&mcp);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "fs");
    }

    #[test]
    fn test_parse_opencode_agent_object_skips_builtin_overrides() {
        let agent = serde_json::json!({
            "build": {"model": "anthropic/claude-sonnet-4-5", "permission": {"edit": "ask"}},
            "plan": {"temperature": 0.1},
            "tuned": {"temperature": 0.2},
            "reviewer": {"description": "Review", "prompt": "Review."}
        });
        let parsed = parse_opencode_agent_object(&agent);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "reviewer");
    }

    #[test]
    fn test_parse_mcp_json_skips_extension_and_sdk_servers() {
        // A Zed extension server has only `settings`; a Claude SDK server is
        // in-process. Neither must become a stdio server with no command.
        let zed = r#"{"context_servers": {
            "ext": {"settings": {"token": "x"}},
            "fs": {"command": "npx", "args": ["-y"]}
        }}"#;
        let parsed = parse_mcp_json(zed).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "fs");

        let claude = r#"{"mcpServers": {"inproc": {"type": "sdk", "name": "inproc"}}}"#;
        assert!(parse_mcp_json(claude).unwrap().is_empty());
    }

    #[test]
    fn test_parse_mcp_json_streamable_http_roundtrip() {
        // A Zoo Code config with `streamable-http` must parse back to an HTTP transport.
        let servers = vec![NormalizedMcpServer {
            name: "ctx".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::new(),
        }];
        let json = generate_zoocode_mcp_json(&servers).unwrap();
        let parsed = parse_mcp_json(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        match &parsed[0].transport {
            McpTransport::Http { url, .. } => assert_eq!(url, "https://example.com/mcp"),
            other => panic!("expected HTTP transport, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_mcp_json_roundtrip() {
        let servers = vec![NormalizedMcpServer {
            name: "test".to_string(),
            transport: McpTransport::Stdio {
                command: "node".to_string(),
                args: vec!["server.js".to_string()],
            },
            env: BTreeMap::from([("KEY".to_string(), "val".to_string())]),
        }];
        let json = generate_mcp_json(&servers).unwrap();
        let parsed = parse_mcp_json(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "test");
    }

    #[test]
    fn test_generate_copilot_mcp() {
        let servers = vec![NormalizedMcpServer {
            name: "fs".to_string(),
            transport: McpTransport::Stdio {
                command: "node".to_string(),
                args: vec![],
            },
            env: BTreeMap::new(),
        }];
        let result = generate_copilot_mcp_json(&servers).unwrap();
        assert!(result.contains("\"servers\""));
        assert!(!result.contains("mcpServers"));
    }

    #[test]
    fn test_build_opencode_mcp_object() {
        let servers = vec![NormalizedMcpServer {
            name: "filesystem".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/fs".to_string()],
            },
            env: BTreeMap::from([("API_KEY".to_string(), "secret".to_string())]),
        }];
        let mcp = build_opencode_mcp_object(&servers);
        let entry = mcp.get("filesystem").unwrap().as_object().unwrap();
        assert_eq!(entry.get("type").unwrap().as_str().unwrap(), "local");
        let command = entry.get("command").unwrap().as_array().unwrap();
        assert_eq!(command.len(), 3);
        assert_eq!(command[0].as_str().unwrap(), "npx");
        assert_eq!(command[1].as_str().unwrap(), "-y");
        assert_eq!(command[2].as_str().unwrap(), "@mcp/fs");
        assert!(entry.get("environment").is_some());
        assert!(entry.get("env").is_none());
    }

    #[test]
    fn test_build_opencode_mcp_object_http() {
        let servers = vec![NormalizedMcpServer {
            name: "api".to_string(),
            transport: McpTransport::Http {
                url: "https://api.example.com/mcp".to_string(),
                headers: BTreeMap::from([("Authorization".to_string(), "Bearer x".to_string())]),
            },
            env: BTreeMap::new(),
        }];
        let mcp = build_opencode_mcp_object(&servers);
        let entry = mcp.get("api").unwrap().as_object().unwrap();
        assert_eq!(entry.get("type").unwrap().as_str().unwrap(), "remote");
        assert_eq!(
            entry.get("url").unwrap().as_str().unwrap(),
            "https://api.example.com/mcp"
        );
        assert!(entry.get("headers").is_some());
    }

    #[test]
    fn test_generate_copilot_mcp_emits_env_and_headers() {
        let servers = vec![
            NormalizedMcpServer {
                name: "local".to_string(),
                transport: McpTransport::Stdio {
                    command: "node".to_string(),
                    args: vec![],
                },
                env: BTreeMap::from([("X".to_string(), "1".to_string())]),
            },
            NormalizedMcpServer {
                name: "remote".to_string(),
                transport: McpTransport::Http {
                    url: "https://example.com/mcp".to_string(),
                    headers: BTreeMap::from([("Auth".to_string(), "Bearer y".to_string())]),
                },
                env: BTreeMap::new(),
            },
        ];
        let result = generate_copilot_mcp_json(&servers).unwrap();
        assert!(result.contains("\"env\""));
        assert!(result.contains("\"headers\""));
        assert!(result.contains("Bearer y"));
    }

    #[test]
    fn test_generate_zed_mcp() {
        let servers = vec![NormalizedMcpServer {
            name: "fs".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/fs".to_string()],
            },
            env: BTreeMap::new(),
        }];
        let obj = build_zed_context_servers_object(&servers);
        let result =
            serde_json::to_string_pretty(&serde_json::json!({ "context_servers": obj })).unwrap();
        assert!(result.contains("\"context_servers\""));
        assert!(result.contains("\"command\": \"npx\""));
        assert!(!result.contains("\"source\""));
        assert!(result.contains("fs"));
        assert!(!result.contains("mcpServers"));
        assert!(!result.contains("\"type\""));
    }

    #[test]
    fn test_generate_gemini_mcp_stdio() {
        let servers = vec![NormalizedMcpServer {
            name: "fs".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/fs".to_string()],
            },
            env: BTreeMap::new(),
        }];
        let obj = build_gemini_mcp_object(&servers);
        let result =
            serde_json::to_string_pretty(&serde_json::json!({ "mcpServers": obj })).unwrap();
        assert!(result.contains("\"mcpServers\""));
        assert!(result.contains("\"command\": \"npx\""));
        assert!(result.contains("\"fs\""));
        // Gemini does NOT use "type" field
        assert!(!result.contains("\"type\""));
    }

    #[test]
    fn test_generate_gemini_mcp_http() {
        let servers = vec![NormalizedMcpServer {
            name: "api".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::new(),
        }];
        let obj = build_gemini_mcp_object(&servers);
        let result =
            serde_json::to_string_pretty(&serde_json::json!({ "mcpServers": obj })).unwrap();
        // Gemini uses "httpUrl" not "url"
        assert!(result.contains("\"httpUrl\": \"https://example.com/mcp\""));
        assert!(!result.contains("\"url\""));
        assert!(!result.contains("\"type\""));
    }

    #[test]
    fn test_parse_opencode_mcp_object_roundtrip() {
        // OpenCode's shape (`type: local`, `command` array, `environment`) is not
        // parseable by the generic reader, so it has its own inverse.
        let servers = vec![
            NormalizedMcpServer {
                name: "fs".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/fs".to_string()],
                },
                env: BTreeMap::from([("API_KEY".to_string(), "secret".to_string())]),
            },
            NormalizedMcpServer {
                name: "api".to_string(),
                transport: McpTransport::Http {
                    url: "https://example.com/mcp".to_string(),
                    headers: BTreeMap::from([("Auth".to_string(), "Bearer x".to_string())]),
                },
                env: BTreeMap::new(),
            },
        ];
        let obj = build_opencode_mcp_object(&servers);
        let parsed = parse_opencode_mcp_object(&serde_json::Value::Object(obj));
        assert_eq!(parsed.len(), 2);

        let fs = parsed.iter().find(|s| s.name == "fs").unwrap();
        match &fs.transport {
            McpTransport::Stdio { command, args } => {
                assert_eq!(command, "npx");
                assert_eq!(args, &["-y".to_string(), "@mcp/fs".to_string()]);
            }
            other => panic!("expected stdio, got {other:?}"),
        }
        assert_eq!(fs.env.get("API_KEY").map(String::as_str), Some("secret"));

        let api = parsed.iter().find(|s| s.name == "api").unwrap();
        match &api.transport {
            McpTransport::Http { url, headers } => {
                assert_eq!(url, "https://example.com/mcp");
                assert_eq!(headers.get("Auth").map(String::as_str), Some("Bearer x"));
            }
            other => panic!("expected http, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_mcp_json_amp_key() {
        // Amp keys its servers under the dotted `amp.mcpServers` and emits no
        // `type` field — both must survive the read path.
        let servers = vec![NormalizedMcpServer {
            name: "linear".to_string(),
            transport: McpTransport::Http {
                url: "https://mcp.linear.app/mcp".to_string(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::new(),
        }];
        let json = serde_json::to_string_pretty(
            &serde_json::json!({ "amp.mcpServers": build_amp_mcp_object(&servers) }),
        )
        .unwrap();
        assert!(!json.contains("\"type\""));

        let parsed = parse_mcp_json(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "linear");
        match &parsed[0].transport {
            McpTransport::Http { url, .. } => assert_eq!(url, "https://mcp.linear.app/mcp"),
            other => panic!("expected http, got {other:?}"),
        }
    }

    #[test]
    fn test_build_opencode_agent_object() {
        let agents = vec![crate::config::NormalizedAgent {
            name: "reviewer".to_string(),
            description: "Code review".to_string(),
            content: "Review code.".to_string(),
            model: Some("openai/gpt-4o".to_string()),
            tools: vec![],
            ..Default::default()
        }];
        let map = build_opencode_agent_object(&agents);
        let entry = map.get("reviewer").unwrap().as_object().unwrap();
        assert_eq!(
            entry.get("description").unwrap().as_str().unwrap(),
            "Code review"
        );
        assert_eq!(entry.get("mode").unwrap().as_str().unwrap(), "subagent");
        assert_eq!(
            entry.get("model").unwrap().as_str().unwrap(),
            "openai/gpt-4o"
        );
        assert_eq!(
            entry.get("prompt").unwrap().as_str().unwrap(),
            "Review code."
        );
    }

    #[test]
    fn test_opencode_agent_drops_bare_model_and_fills_description() {
        // OpenCode resolves `model` as `provider/model`: a bare id would become
        // provider `gpt-4o` with an empty model, so it is left out. An agent
        // without a description is not listed, so the name stands in.
        let agents = vec![crate::config::NormalizedAgent {
            name: "reviewer".to_string(),
            model: Some("gpt-4o".to_string()),
            ..Default::default()
        }];
        let map = build_opencode_agent_object(&agents);
        let entry = map["reviewer"].as_object().unwrap();
        assert!(entry.get("model").is_none());
        assert_eq!(entry["description"], "reviewer");
    }

    fn remote_with_env(url: &str) -> NormalizedMcpServer {
        NormalizedMcpServer {
            name: "remote".to_string(),
            transport: McpTransport::Http {
                url: url.to_string(),
                headers: BTreeMap::from([(
                    "Authorization".to_string(),
                    "Bearer ${TOKEN}".to_string(),
                )]),
            },
            env: BTreeMap::from([("X".to_string(), "1".to_string())]),
        }
    }

    #[test]
    fn test_remote_entries_carry_env_only_where_documented() {
        let servers = vec![remote_with_env("https://example.com/mcp")];
        // Documented for stdio only (or rejected) on these tools.
        for (tool, map) in [
            ("claude", build_claude_servers_object(&servers)),
            ("cursor", build_cursor_servers_object(&servers)),
            ("copilot", build_copilot_servers_object(&servers)),
            ("zoocode", build_zoocode_servers_object(&servers)),
            ("zed", build_zed_context_servers_object(&servers)),
            ("amp", build_amp_mcp_object(&servers)),
            ("devin", build_devin_servers_object(&servers)),
        ] {
            assert!(map["remote"].get("env").is_none(), "{tool}: {map:?}");
        }
        let opencode = build_opencode_mcp_object(&servers);
        assert!(opencode["remote"].get("environment").is_none());
        // Kiro documents `env` on remote servers.
        assert_eq!(
            build_kiro_servers_object(&servers)["remote"]["env"]["X"],
            "1"
        );
    }

    #[test]
    fn test_cursor_remote_entry_has_no_type() {
        let map = build_cursor_servers_object(&[remote_with_env("https://e.x/mcp")]);
        assert!(map["remote"].get("type").is_none(), "{map:?}");
        assert_eq!(map["remote"]["url"], "https://e.x/mcp");
    }

    #[test]
    fn test_env_refs_are_written_in_each_tool_syntax() {
        let header = |map: &serde_json::Map<String, serde_json::Value>| {
            map["remote"]["headers"]["Authorization"].clone()
        };
        let servers = vec![remote_with_env("${API_URL:-https://default}/mcp")];
        assert_eq!(
            header(&build_claude_servers_object(&servers)),
            "Bearer ${TOKEN}"
        );
        assert_eq!(
            build_claude_servers_object(&servers)["remote"]["url"],
            "${API_URL:-https://default}/mcp"
        );
        for map in [
            build_cursor_servers_object(&servers),
            build_copilot_servers_object(&servers),
            build_zoocode_servers_object(&servers),
            build_devin_servers_object(&servers),
        ] {
            assert_eq!(header(&map), "Bearer ${env:TOKEN}");
            assert_eq!(map["remote"]["url"], "${env:API_URL}/mcp");
        }
        let kiro = build_kiro_servers_object(&servers);
        assert_eq!(kiro["remote"]["url"], "${API_URL}/mcp");
        let opencode = build_opencode_mcp_object(&servers);
        assert_eq!(
            opencode["remote"]["headers"]["Authorization"],
            "Bearer {env:TOKEN}"
        );
        // Zed expands nothing: the text is copied as is.
        assert_eq!(
            header(&build_zed_context_servers_object(&servers)),
            "Bearer ${TOKEN}"
        );
    }

    #[test]
    fn test_env_refs_are_read_back_to_the_normalized_spelling() {
        use EnvRefStyle::*;
        assert_eq!(env_refs_from_tool("${env:TOKEN}", EnvColon), "${TOKEN}");
        assert_eq!(
            env_refs_from_tool("${workspaceFolder}/x ${input:key}", EnvColon),
            "${workspaceFolder}/x ${input:key}"
        );
        assert_eq!(
            env_refs_from_tool("a {env:TOKEN} b", OpenCode),
            "a ${TOKEN} b"
        );
        assert_eq!(
            env_refs_from_tool("$HOME/x and ${Y}", DollarOrBare),
            "${HOME}/x and ${Y}"
        );
        assert_eq!(env_refs_from_tool("cost $5", DollarOrBare), "cost $5");
        // A nested default is one reference: no stray brace is left behind.
        assert_eq!(
            env_refs_to_tool("${VAR:-${OTHER}}/x", EnvColon),
            "${env:VAR}/x"
        );
        assert_eq!(env_refs_to_tool("${VAR:-${OTHER}}", OpenCode), "{env:VAR}");
        assert_eq!(
            env_refs_to_tool("${VAR:-${OTHER}}", DollarNoDefault),
            "${VAR}"
        );
        // A predefined variable is never turned into an environment variable.
        assert_eq!(
            env_refs_to_tool("${workspaceFolder}/${TOKEN}", EnvColon),
            "${workspaceFolder}/${env:TOKEN}"
        );
        // Round trip through every style.
        for style in [EnvColon, OpenCode, DollarNoDefault, DollarOrBare, Dollar] {
            assert_eq!(
                env_refs_from_tool(&env_refs_to_tool("x ${A_1} y", style), style),
                "x ${A_1} y",
                "{style:?}"
            );
        }
    }

    #[test]
    fn test_sync_resets_flags_that_would_contradict_the_synced_entry() {
        let merge = |existing: serde_json::Value, generated, owned| {
            serde_json::Value::Object(crate::json_settings::merge_server_entries(
                Some(&existing),
                generated,
                owned,
            ))
        };
        let server = vec![remote_with_env("https://e.x/mcp")];
        // A hand-set `"enabled": false` would keep Zed from starting the
        // server while `check` reports everything in sync.
        let zed = merge(
            serde_json::json!({"remote": {"enabled": false, "timeout": 5}}),
            build_zed_context_servers_object(&server),
            ZED_OWNED_SERVER_KEYS,
        );
        assert!(zed["remote"].get("enabled").is_none());
        assert_eq!(zed["remote"]["timeout"], 5);
        // `gemini mcp add -t sse` writes `type` next to `url`; it must not
        // survive beside the `httpUrl` conforme writes.
        let gemini = merge(
            serde_json::json!({"remote": {"type": "sse", "url": "https://old", "trust": true}}),
            build_gemini_mcp_object(&server),
            GEMINI_OWNED_SERVER_KEYS,
        );
        assert!(gemini["remote"].get("type").is_none());
        assert!(gemini["remote"].get("url").is_none());
        assert_eq!(gemini["remote"]["httpUrl"], "https://e.x/mcp");
        assert_eq!(gemini["remote"]["trust"], true);
    }

    #[test]
    fn test_merge_opencode_agents_keeps_user_entries() {
        let existing = serde_json::json!({
            "build": {"permission": {"bash": "ask"}},
            "reviewer": {"description": "old", "mode": "subagent", "temperature": 0.1, "model": "anthropic/claude-sonnet-4-5"},
            "stale": {"description": "gone", "mode": "subagent", "prompt": "x"},
            "mine": {"description": "hand-made", "mode": "subagent", "prompt": "y", "steps": 5}
        });
        let generated = build_opencode_agent_object(&[crate::config::NormalizedAgent {
            name: "reviewer".to_string(),
            description: "Review".to_string(),
            content: "Review.".to_string(),
            ..Default::default()
        }]);
        let merged = merge_opencode_agents(Some(&existing), generated);
        assert_eq!(merged["build"], existing["build"]);
        assert_eq!(merged["reviewer"]["description"], "Review");
        assert_eq!(merged["reviewer"]["temperature"], 0.1);
        // The source's model is no `provider/model`, so the user's is kept.
        assert_eq!(merged["reviewer"]["model"], "anthropic/claude-sonnet-4-5");
        // Nothing the source does not define is deleted: a hand-written agent
        // looks exactly like a generated one.
        assert_eq!(merged["stale"], existing["stale"]);
        assert_eq!(merged["mine"], existing["mine"]);
    }
}
