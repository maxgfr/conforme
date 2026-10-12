use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use toml_edit::{value, Array, DocumentMut, InlineTable, Item, Table, Value};

use crate::config::{McpTransport, NormalizedMcpServer};

/// Merge normalized MCP servers into Codex's project-scoped `.codex/config.toml`.
/// Existing non-MCP settings, comments and Codex-specific server options are
/// preserved; a server the source no longer defines is dropped unless it is
/// disabled.
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

    // The source decides which servers exist: one it no longer defines is
    // dropped, unless it is disabled (`enabled = false`) or a kind conforme
    // cannot express.
    let stale: Vec<String> = mcp_servers
        .iter()
        .filter(|(name, item)| {
            let disabled = item.get("enabled").and_then(Item::as_bool) == Some(false);
            let expressible = item.get("command").is_some() || item.get("url").is_some();
            !servers.iter().any(|s| s.name == *name) && expressible && !disabled
        })
        .map(|(name, _)| name.to_string())
        .collect();
    for name in stale {
        mcp_servers.remove(&name);
    }

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
            McpTransport::Http { url, headers } | McpTransport::Sse { url, headers } => {
                // Codex rejects `env` on an HTTP server: it is dropped, as in
                // every JSON shape without `env` on remote servers. It rejects
                // the legacy `bearer_token` there too.
                entry.remove("command");
                entry.remove("args");
                entry.remove("env");
                entry.remove("cwd");
                entry.remove("env_vars");
                entry.remove("bearer_token");
                entry.insert("url", value(url.clone()));

                // Codex expands no `${VAR}` in headers: `Bearer ${VAR}` on
                // `Authorization` becomes `bearer_token_env_var`, and a header
                // that is exactly `${VAR}` becomes `env_http_headers`.
                let mut bearer = None;
                let mut from_env = BTreeMap::new();
                let mut literal = BTreeMap::new();
                for (name, header_value) in headers {
                    let bearer_ref = bearer_env_var(header_value)
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

/// The strings of a server a tool copies as they are: `command` and `args`
/// (with `env` values) for stdio, `url` and header values for HTTP. `env` on
/// an HTTP server is dropped by every target, so it is not listed.
pub(crate) fn server_strings(server: &NormalizedMcpServer) -> Vec<(Option<&str>, &str)> {
    match &server.transport {
        McpTransport::Stdio { command, args } => std::iter::once(command)
            .chain(args)
            .map(|s| (None, s.as_str()))
            .chain(
                server
                    .env
                    .iter()
                    .map(|(k, v)| (Some(k.as_str()), v.as_str())),
            )
            .collect(),
        McpTransport::Http { url, headers } | McpTransport::Sse { url, headers } => {
            std::iter::once((None, url.as_str()))
                .chain(headers.iter().map(|(k, v)| (Some(k.as_str()), v.as_str())))
                .collect()
        }
    }
}

/// Whether Codex writes this string as is although it holds a `${VAR}`:
/// Codex expands none, and only forwards by name a stdio `NAME=${NAME}`, an
/// `Authorization: Bearer ${VAR}` and a header that is exactly `${VAR}`.
pub(crate) fn codex_keeps_literal(server: &NormalizedMcpServer) -> bool {
    let http = matches!(
        server.transport,
        McpTransport::Http { .. } | McpTransport::Sse { .. }
    );
    server_strings(server)
        .into_iter()
        .any(|(key, value)| match key {
            _ if !value.contains("${") => false,
            Some(key) if !http => env_ref_name(value) != Some(key),
            Some(key) => {
                env_ref_name(value).is_none()
                    && !(key.eq_ignore_ascii_case("authorization")
                        && bearer_env_var(value).is_some())
            }
            None => true,
        })
}

/// `VAR` for a header value that is exactly `Bearer ${VAR}`.
pub(crate) fn bearer_env_var(value: &str) -> Option<&str> {
    value.strip_prefix("Bearer ").and_then(env_ref_name)
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
/// Split a command string the way Python's `shlex.split` does (POSIX mode,
/// no comments), as Vibe does with a string `command`. `None` for an
/// unterminated quote or escape, which `shlex` refuses too.
fn shell_split(text: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if let Some(done) = word.take() {
                    words.push(done);
                }
            }
            '\'' => {
                let current = word.get_or_insert_with(String::new);
                loop {
                    match chars.next()? {
                        '\'' => break,
                        c => current.push(c),
                    }
                }
            }
            '"' => {
                let current = word.get_or_insert_with(String::new);
                loop {
                    match chars.next()? {
                        '"' => break,
                        // Only `\` and `"` are escaped inside double quotes.
                        '\\' => match chars.next()? {
                            c @ ('\\' | '"') => current.push(c),
                            c => {
                                current.push('\\');
                                current.push(c);
                            }
                        },
                        c => current.push(c),
                    }
                }
            }
            '\\' => word.get_or_insert_with(String::new).push(chars.next()?),
            c => word.get_or_insert_with(String::new).push(c),
        }
    }
    words.extend(word);
    Some(words)
}

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
    // An inline array of inline tables (`mcp_servers = []`) is the same
    // setting: it becomes `[[mcp_servers]]` tables.
    if let Some(array) = document["mcp_servers"].as_array() {
        let mut tables = toml_edit::ArrayOfTables::new();
        for item in array.iter() {
            let table = item.as_inline_table().context(
                "Vibe config `mcp_servers` must be an array of tables ([[mcp_servers]])",
            )?;
            tables.push(table.clone().into_table());
        }
        document["mcp_servers"] = Item::ArrayOfTables(tables);
    }
    let entries = document["mcp_servers"]
        .as_array_of_tables_mut()
        .context("Vibe config `mcp_servers` must be an array of tables ([[mcp_servers]])")?;

    // The source decides which servers exist: one it no longer defines is
    // dropped, unless it is disabled or a kind conforme cannot express.
    let names: Vec<&str> = servers.iter().map(|s| s.name.as_str()).collect();
    entries.retain(|table| {
        let named = table
            .get("name")
            .and_then(Item::as_str)
            .is_some_and(|name| names.contains(&name));
        let disabled = table.get("disabled").and_then(Item::as_bool) == Some(true);
        let expressible = table.contains_key("command") || table.contains_key("url");
        named || disabled || !expressible
    });

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
                // Vibe shell-splits a string command: one that would not
                // come back whole (a path with a space) is written as a list.
                if shell_split(command).as_deref() == Some(std::slice::from_ref(command)) {
                    entry.insert("command", value(command.clone()));
                } else {
                    let mut list = Array::new();
                    list.push(command.clone());
                    entry.insert("command", value(list));
                }
                let mut toml_args = Array::new();
                for arg in args {
                    toml_args.push(arg);
                }
                entry.insert("args", value(toml_args));
                set_string_map(entry, "env", &server.env, true);
            }
            McpTransport::Http { url, headers } | McpTransport::Sse { url, headers } => {
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

                let api_key = vibe_api_key(headers);
                let literal: BTreeMap<String, String> = headers
                    .iter()
                    .filter(|(name, _)| api_key.as_ref().is_none_or(|key| key.header != **name))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                let oauth = entry
                    .get("auth")
                    .and_then(|a| a.get("type"))
                    .and_then(Item::as_str)
                    == Some("oauth");
                if api_key.is_none() && literal.is_empty() {
                    if !oauth {
                        entry.remove("auth");
                    }
                } else {
                    let mut auth = InlineTable::new();
                    auth.insert("type", "static".into());
                    if let Some(key) = api_key {
                        auth.insert("api_key_env", key.var.into());
                        auth.insert("api_key_header", key.header.into());
                        auth.insert("api_key_format", key.format.into());
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

/// The one header Vibe reads from an environment variable (a static `auth`
/// with `api_key_env`): `Authorization: Bearer ${VAR}`, else the first header
/// that is exactly `${VAR}`.
pub(crate) struct VibeApiKey<'a> {
    pub header: &'a str,
    pub var: &'a str,
    pub format: &'static str,
}

pub(crate) fn vibe_api_key(headers: &BTreeMap<String, String>) -> Option<VibeApiKey<'_>> {
    let bearer = headers.iter().find_map(|(name, value)| {
        let var = bearer_env_var(value).filter(|_| name.eq_ignore_ascii_case("Authorization"))?;
        Some(VibeApiKey {
            header: name,
            var,
            format: "Bearer {token}",
        })
    });
    bearer.or_else(|| {
        headers.iter().find_map(|(name, value)| {
            Some(VibeApiKey {
                header: name,
                var: env_ref_name(value)?,
                format: "{token}",
            })
        })
    })
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
            // `command` is a string Vibe shell-splits, or a list holding the
            // arguments too.
            let mut parts: Vec<String> = match entry.get("command") {
                Some(toml::Value::String(command)) => {
                    shell_split(command).unwrap_or_else(|| vec![command.clone()])
                }
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
    "oauth",
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
    /// `${VAR}` with no default form (Kiro); a default is dropped.
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
fn rewrite_braced(s: &str, open: &str, mut f: impl FnMut(&str) -> Option<String>) -> String {
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
            split(body)
                .filter(|(n, _)| !PREDEFINED_VARIABLES.contains(&n.as_str()))
                .map(|(n, _)| format!("{{env:{n}}}"))
        }),
    }
}

/// What writing `servers` in `style` loses, one warning per server: the
/// default of a `${VAR:-default}` reference the style has no form for, and
/// an editor variable (`${workspaceFolder}`) for a tool that has none
/// (`editor_variables` false), which reaches it as an environment variable
/// or as text.
pub(crate) fn env_ref_losses(
    servers: &[NormalizedMcpServer],
    style: EnvRefStyle,
    editor_variables: bool,
    tool: &str,
) -> Vec<String> {
    let drops_defaults = matches!(
        style,
        EnvRefStyle::DollarNoDefault | EnvRefStyle::EnvColon | EnvRefStyle::OpenCode
    );
    let mut warnings = Vec::new();
    for server in servers {
        let mut defaults = Vec::new();
        let mut editor = Vec::new();
        for (_, value) in server_strings(server) {
            rewrite_braced(value, "${", |body| {
                let (name, default) = match body.split_once(":-") {
                    Some((name, default)) => (name, Some(default)),
                    None => (body, None),
                };
                if PREDEFINED_VARIABLES.contains(&name) {
                    if !editor_variables && !editor.contains(&name.to_string()) {
                        editor.push(name.to_string());
                    }
                } else if let Some(default) = default.filter(|_| drops_defaults) {
                    defaults.push(format!("${{{body}}} loses its default `{default}`"));
                }
                None
            });
        }
        if !defaults.is_empty() {
            warnings.push(format!(
                "MCP server {}: {tool} has no default value for a variable reference: {}",
                server.name,
                defaults.join(", ")
            ));
        }
        if !editor.is_empty() {
            warnings.push(format!(
                "MCP server {}: {tool} has no editor variables, so {} is not resolved there; \
                 use an absolute path or an environment variable",
                server.name,
                editor
                    .iter()
                    .map(|n| format!("${{{n}}}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    warnings
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
fn rewrite_server_strings(server: &mut NormalizedMcpServer, f: impl Fn(&str) -> String) {
    let rewrite = |m: &mut BTreeMap<String, String>| m.values_mut().for_each(|v| *v = f(v));
    match &mut server.transport {
        McpTransport::Stdio { command, args } => {
            *command = f(command);
            args.iter_mut().for_each(|a| *a = f(a));
        }
        McpTransport::Http { url, headers } | McpTransport::Sse { url, headers } => {
            *url = f(url);
            rewrite(headers);
        }
    }
    rewrite(&mut server.env);
}

/// Rewrite the variable references of servers read from a tool written in
/// `style` into the normalized `${VAR}` spelling.
pub fn canonicalize_env_refs(
    mut servers: Vec<NormalizedMcpServer>,
    style: EnvRefStyle,
) -> Vec<NormalizedMcpServer> {
    if !matches!(
        style,
        EnvRefStyle::Dollar | EnvRefStyle::DollarNoDefault | EnvRefStyle::Literal
    ) {
        for server in &mut servers {
            rewrite_server_strings(server, |v| env_refs_from_tool(v, style));
        }
    }
    servers
}

/// How a legacy SSE server is written; `None`: as the HTTP variant (the tool has no SSE).
pub struct SseShape {
    /// Key of the server's URL.
    url_key: &'static str,
    /// Marker written on the entry (`type: "sse"`, Devin's `transport: "sse"`).
    marker: Option<(&'static str, &'static str)>,
}

/// Where an entry keeps its command and environment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Layout {
    /// `command` + `args` + `env`.
    Flat,
    /// OpenCode and Kilo Code: `command: [cmd, ...args]` and `environment`.
    OpenCode,
}

/// The JSON shape of one tool's MCP server entries: how conforme writes them
/// ([`ServerShape::build`]), which keys of an existing entry it owns
/// ([`ServerShape::owned_keys`]), and how it reads them back
/// ([`ServerShape::read`]).
pub struct ServerShape {
    layout: Layout,
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
    sse: Option<SseShape>,
    env_refs: EnvRefStyle,
    /// The key that turns a server off (`disabled`; `enabled` for Zed and
    /// OpenCode): owned, so a synced server is enabled again — leaving it
    /// disabled would make `check` pass while the tool still hides it.
    toggle: Option<&'static str>,
    /// Kilo Code: a local server inherits Kilo's environment, so an `env`
    /// entry that only passes a variable through (`TOKEN=${TOKEN}`) is left
    /// out instead of being set to the literal text.
    drop_passthrough_env: bool,
}

/// Claude Code `.mcp.json`: `type` is `stdio` or `http`; remote servers take
/// `url` + `headers`, no `env`.
pub const CLAUDE_SHAPE: ServerShape = ServerShape {
    layout: Layout::Flat,
    toggle: None,
    drop_passthrough_env: false,
    stdio_type: Some("stdio"),
    http_type: Some("http"),
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    sse: Some(SseShape {
        url_key: "url",
        marker: Some(("type", "sse")),
    }),
    env_refs: EnvRefStyle::Dollar,
};

/// Cursor `.cursor/mcp.json`: local servers need `type: "stdio"`, remote ones
/// are just `url` (+ `headers`) with no `type`; references are `${env:VAR}`.
pub const CURSOR_SHAPE: ServerShape = ServerShape {
    layout: Layout::Flat,
    toggle: None,
    drop_passthrough_env: false,
    stdio_type: Some("stdio"),
    http_type: None,
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    sse: Some(SseShape {
        url_key: "url",
        marker: Some(("type", "sse")),
    }),
    env_refs: EnvRefStyle::EnvColon,
};

/// Kiro `.kiro/settings/mcp.json`: remote servers also accept `env`.
pub const KIRO_SHAPE: ServerShape = ServerShape {
    layout: Layout::Flat,
    toggle: Some("disabled"),
    drop_passthrough_env: false,
    stdio_type: Some("stdio"),
    http_type: Some("http"),
    url_key: "url",
    http_extra: None,
    env_on_http: true,
    sse: Some(SseShape {
        url_key: "url",
        marker: None,
    }),
    env_refs: EnvRefStyle::DollarNoDefault,
};

/// VS Code `.vscode/mcp.json` (`servers` key): remote fields are `type`,
/// `url`, `headers`, `oauth` — no `env`.
pub const COPILOT_SHAPE: ServerShape = ServerShape {
    layout: Layout::Flat,
    toggle: None,
    drop_passthrough_env: false,
    stdio_type: Some("stdio"),
    http_type: Some("http"),
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    sse: Some(SseShape {
        url_key: "url",
        marker: Some(("type", "sse")),
    }),
    env_refs: EnvRefStyle::EnvColon,
};

/// Zoo Code `.roo/mcp.json`: HTTP servers are `type: "streamable-http"` (a
/// bare `"http"` is not accepted), and Zoo's schema rejects `env` on a remote
/// entry.
pub const ZOOCODE_SHAPE: ServerShape = ServerShape {
    layout: Layout::Flat,
    toggle: Some("disabled"),
    drop_passthrough_env: false,
    stdio_type: Some("stdio"),
    http_type: Some("streamable-http"),
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    sse: Some(SseShape {
        url_key: "url",
        marker: Some(("type", "sse")),
    }),
    env_refs: EnvRefStyle::EnvColon,
};

/// Zed `context_servers`: flat shape with no `type`; the remote variant has no
/// `env`, and Zed expands no variable references.
pub const ZED_SHAPE: ServerShape = ServerShape {
    layout: Layout::Flat,
    toggle: Some("enabled"),
    drop_passthrough_env: false,
    stdio_type: None,
    http_type: None,
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    sse: None,
    env_refs: EnvRefStyle::Literal,
};

/// Gemini CLI `mcpServers`: no `type`, streamable HTTP servers use `httpUrl`.
pub const GEMINI_SHAPE: ServerShape = ServerShape {
    layout: Layout::Flat,
    toggle: None,
    drop_passthrough_env: false,
    stdio_type: None,
    http_type: None,
    url_key: "httpUrl",
    http_extra: None,
    env_on_http: true,
    sse: Some(SseShape {
        url_key: "url",
        marker: Some(("type", "sse")),
    }),
    env_refs: EnvRefStyle::DollarOrBare,
};

/// Devin `.devin/mcp_config.json`: no `type`; remote servers are `url` with an
/// optional `transport` (`http` or `sse`) and `headers`, no `env`.
pub const DEVIN_SHAPE: ServerShape = ServerShape {
    layout: Layout::Flat,
    toggle: Some("disabled"),
    drop_passthrough_env: false,
    stdio_type: None,
    http_type: None,
    url_key: "url",
    http_extra: Some(("transport", "http")),
    env_on_http: false,
    sse: Some(SseShape {
        url_key: "url",
        marker: Some(("transport", "sse")),
    }),
    env_refs: EnvRefStyle::EnvColon,
};

/// Antigravity `.agents/mcp_config.json`: no `type`; every remote server (HTTP
/// or SSE) is `serverUrl` + `headers`, no `env`. No variable reference is
/// documented, so `${VAR}` is written as is.
pub const ANTIGRAVITY_SHAPE: ServerShape = ServerShape {
    layout: Layout::Flat,
    toggle: None,
    drop_passthrough_env: false,
    stdio_type: None,
    http_type: None,
    url_key: "serverUrl",
    http_extra: None,
    env_on_http: false,
    sse: Some(SseShape {
        url_key: "serverUrl",
        marker: None,
    }),
    env_refs: EnvRefStyle::Dollar,
};

/// OpenCode `opencode.json` `mcp`: `type` is `local` / `remote` (SSE is
/// `remote`, OpenCode tries it itself), `command` is one `[cmd, ...args]`
/// array, the environment is `environment` (local servers only), and
/// references are written `{env:VAR}`.
pub const OPENCODE_SHAPE: ServerShape = ServerShape {
    layout: Layout::OpenCode,
    stdio_type: Some("local"),
    http_type: Some("remote"),
    url_key: "url",
    http_extra: None,
    env_on_http: false,
    sse: None,
    env_refs: EnvRefStyle::OpenCode,
    toggle: Some("enabled"),
    drop_passthrough_env: false,
};

/// Kilo Code `mcp`: OpenCode's shape with no variable reference — Kilo
/// refuses a project config holding `{env:VAR}`, so strings are copied
/// verbatim (see [`ServerShape::drop_passthrough_env`]).
pub const KILO_SHAPE: ServerShape = ServerShape {
    env_refs: EnvRefStyle::Literal,
    drop_passthrough_env: true,
    ..OPENCODE_SHAPE
};

fn string_map(map: &BTreeMap<String, String>) -> serde_json::Value {
    serde_json::Value::Object(
        map.iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect(),
    )
}

/// The string values of a JSON object (`env`, `headers`).
fn json_string_map(value: Option<&serde_json::Value>) -> BTreeMap<String, String> {
    value
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

impl ServerShape {
    /// The servers object conforme writes for `servers`.
    pub fn build(
        &self,
        servers: &[NormalizedMcpServer],
    ) -> serde_json::Map<String, serde_json::Value> {
        use serde_json::Value as Json;
        let text = |s: &str| Json::String(s.to_string());
        let mut out = serde_json::Map::new();

        for server in servers {
            let mut server = server.clone();
            if self.drop_passthrough_env {
                server
                    .env
                    .retain(|key, value| value.trim() != format!("${{{key}}}"));
            }
            rewrite_server_strings(&mut server, |s| env_refs_to_tool(s, self.env_refs));
            let mut entry = serde_json::Map::new();

            let is_http = match &server.transport {
                McpTransport::Stdio { command, args } => {
                    if let Some(t) = self.stdio_type {
                        entry.insert("type".to_string(), text(t));
                    }
                    let args = args.iter().map(|a| text(a));
                    match self.layout {
                        Layout::Flat => {
                            entry.insert("command".to_string(), text(command));
                            entry.insert("args".to_string(), Json::Array(args.collect()));
                        }
                        Layout::OpenCode => {
                            let command = std::iter::once(text(command)).chain(args).collect();
                            entry.insert("command".to_string(), Json::Array(command));
                        }
                    }
                    false
                }
                McpTransport::Sse { url, headers } if self.sse.is_some() => {
                    let sse = self.sse.as_ref().expect("checked by the guard");
                    if let Some((key, value)) = sse.marker {
                        entry.insert(key.to_string(), text(value));
                    }
                    entry.insert(sse.url_key.to_string(), text(url));
                    if !headers.is_empty() {
                        entry.insert("headers".to_string(), string_map(headers));
                    }
                    true
                }
                McpTransport::Http { url, headers } | McpTransport::Sse { url, headers } => {
                    if let Some(t) = self.http_type {
                        entry.insert("type".to_string(), text(t));
                    }
                    entry.insert(self.url_key.to_string(), text(url));
                    if let Some((key, value)) = self.http_extra {
                        entry.insert(key.to_string(), text(value));
                    }
                    if !headers.is_empty() {
                        entry.insert("headers".to_string(), string_map(headers));
                    }
                    true
                }
            };

            if !server.env.is_empty() && (self.env_on_http || !is_http) {
                entry.insert(self.env_key().to_string(), string_map(&server.env));
            }

            out.insert(server.name, Json::Object(entry));
        }

        out
    }

    fn env_key(&self) -> &'static str {
        match self.layout {
            Layout::Flat => "env",
            Layout::OpenCode => "environment",
        }
    }

    /// The keys of a server entry conforme owns, across every transport:
    /// regenerated when the server is synced. Any other key an existing entry
    /// carries is tool-specific configuration the user (or the tool) added,
    /// and `json_settings::merge_server_entries` keeps it. Every URL spelling
    /// and `type` are owned, so a stale one (a hand-written `serverUrl`, the
    /// `type` `gemini mcp add -t http` writes) never contradicts what conforme
    /// writes.
    pub fn owned_keys(&self) -> Vec<&'static str> {
        let mut keys = vec!["type", "command", self.env_key()];
        if self.layout == Layout::Flat {
            keys.push("args");
        }
        keys.extend(["url", "httpUrl", "serverUrl", "headers"]);
        keys.extend(self.http_extra.map(|(key, _)| key));
        keys.extend(
            self.sse
                .as_ref()
                .and_then(|sse| sse.marker)
                .map(|(key, _)| key),
        );
        keys.extend(self.toggle);
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    /// Read a servers object written in this shape back into normalized
    /// servers, with conforme's `${VAR}` spelling. An entry conforme cannot
    /// express (see `json_settings::is_expressible_server`) or that is
    /// disabled is skipped; the merge keeps it in place.
    pub fn read(&self, servers: &serde_json::Value) -> Vec<NormalizedMcpServer> {
        canonicalize_env_refs(parse_servers(servers), self.env_refs)
    }

    /// Read the servers of an MCP JSON file written in this shape (see
    /// [`parse_mcp_json`] for the keys it looks under).
    pub fn read_file(&self, content: &str) -> Result<Vec<NormalizedMcpServer>> {
        Ok(canonicalize_env_refs(
            parse_mcp_json(content)?,
            self.env_refs,
        ))
    }
}

/// Parse a servers object, in any shape conforme writes: the transport is
/// read from `type` (`stdio`/`local`, `http`/`sse`/`remote`/…), Devin's
/// `transport`, or the presence of a URL (`url`, Gemini `httpUrl`,
/// Antigravity `serverUrl`); `command` is a string with `args`, or
/// OpenCode's `[cmd, ...args]`; the environment is `env` or OpenCode's
/// `environment`.
fn parse_servers(servers: &serde_json::Value) -> Vec<NormalizedMcpServer> {
    let Some(servers) = servers.as_object() else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for (name, value) in servers {
        // Zed extension servers, Claude SDK servers and OpenCode's
        // `{ "enabled": false }` toggles have no portable form, and a disabled
        // server is not part of the config.
        if !crate::json_settings::is_expressible_server(value)
            || crate::json_settings::is_disabled_server(value)
        {
            continue;
        }
        let Some(obj) = value.as_object() else {
            continue;
        };

        let transport_type = obj.get("type").and_then(|v| v.as_str());
        // Devin declares its transport under `transport` (`http`, `sse`), not `type`.
        let transport_key = obj.get("transport").and_then(|v| v.as_str());
        let url_value = ["url", "httpUrl", "serverUrl"]
            .iter()
            .find_map(|key| obj.get(*key).and_then(|v| v.as_str()));
        // An entry is remote if it declares a remote transport type OR — when
        // no `type` is present — if it carries a URL rather than a command.
        let is_http = matches!(
            transport_type,
            Some("http" | "https" | "sse" | "streamable-http" | "ws" | "remote")
        ) || (transport_type.is_none() && url_value.is_some());

        let transport = if is_http {
            let url = url_value.unwrap_or("").to_string();
            let headers = json_string_map(obj.get("headers"));
            // SSE servers declare `sse` as their type (Claude, Gemini) or their
            // transport (Devin); every other remote entry stays HTTP.
            let is_sse = url_value.is_some()
                && (transport_type == Some("sse") || transport_key == Some("sse"));
            if is_sse {
                McpTransport::Sse { url, headers }
            } else {
                McpTransport::Http { url, headers }
            }
        } else {
            let strings = |v: &serde_json::Value| -> Vec<String> {
                v.as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let mut parts = match obj.get("command") {
                Some(serde_json::Value::String(command)) => vec![command.clone()],
                Some(command) => strings(command),
                None => Vec::new(),
            };
            parts.extend(obj.get("args").map(strings).unwrap_or_default());
            let mut parts = parts.into_iter();
            McpTransport::Stdio {
                command: parts.next().unwrap_or_default(),
                args: parts.collect(),
            }
        };

        let env = json_string_map(obj.get("env").or_else(|| obj.get("environment")));

        result.push(NormalizedMcpServer {
            name: name.clone(),
            transport,
            env,
        });
    }
    result
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

/// A fresh `.mcp.json` (Claude Code format), for tests.
#[cfg(test)]
pub fn generate_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    wrap_servers("mcpServers", CLAUDE_SHAPE.build(servers))
}

/// A fresh `.roo/mcp.json` (Zoo Code format), for tests.
#[cfg(test)]
pub fn generate_zoocode_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    wrap_servers("mcpServers", ZOOCODE_SHAPE.build(servers))
}

/// A fresh `.vscode/mcp.json` (`servers` key), for tests.
#[cfg(test)]
pub fn generate_copilot_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    wrap_servers("servers", COPILOT_SHAPE.build(servers))
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

/// Kilo Code's built-in agents: OpenCode's, plus its own modes and the
/// `scout` subagent (behind its `experimentalScout` flag).
pub fn is_kilo_builtin_agent(name: &str) -> bool {
    is_opencode_builtin_agent(name)
        || ["code", "ask", "debug", "orchestrator", "scout"]
            .contains(&crate::config::sanitize_name(name).as_str())
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

/// Keys conforme writes into an OpenCode `agent.<name>` entry. Anything else
/// (`permission`, `temperature`, `steps`, `color`, …) is the user's.
pub const OPENCODE_OWNED_AGENT_KEYS: &[&str] = &["description", "mode", "model", "prompt"];

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
/// source does not define is kept here; the OpenCode adapter drops the ones
/// an earlier sync wrote for an agent that has left the source
/// (`stale_agent_entries`).
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
/// and `context_servers` (Zed) — and infers the transport
/// from the entry's shape when there is no explicit `type` field (Gemini
/// `httpUrl`, Zed remote `url`; a Devin Desktop-style `serverUrl` is also
/// accepted for hand-written files).
pub fn parse_mcp_json(content: &str) -> Result<Vec<NormalizedMcpServer>> {
    // JSONC-tolerant: Zed, VS Code and Zoo Code settings may hold comments
    // and trailing commas.
    let root = crate::json_settings::parse_jsonc(content).context("failed to parse MCP JSON")?;
    let servers = ["mcpServers", "servers", "context_servers"]
        .iter()
        .find_map(|key| root.get(*key));
    Ok(servers.map(parse_servers).unwrap_or_default())
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
        let servers = vec![
            NormalizedMcpServer {
                name: "existing".to_string(),
                transport: McpTransport::Http {
                    url: "https://existing.example/mcp".to_string(),
                    headers: BTreeMap::new(),
                },
                env: BTreeMap::new(),
            },
            NormalizedMcpServer {
                name: "filesystem".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
                },
                env: BTreeMap::from([("ROOT".to_string(), "/workspace".to_string())]),
            },
        ];

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
        let servers = vec![
            NormalizedMcpServer {
                name: "existing".to_string(),
                transport: McpTransport::Stdio {
                    command: "printf".to_string(),
                    args: vec!["ok".to_string()],
                },
                env: BTreeMap::new(),
            },
            NormalizedMcpServer {
                name: "filesystem".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
                },
                env: BTreeMap::new(),
            },
        ];

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
                name: "mine".to_string(),
                transport: McpTransport::Stdio {
                    command: "my-server".to_string(),
                    args: Vec::new(),
                },
                env: BTreeMap::new(),
            },
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
    fn test_vibe_string_command_is_shell_split_like_vibe() {
        // Vibe runs `shlex.split(self.command)` on a string command
        // (vibe/core/config/models.py `argv`).
        let toml = r#"[[mcp_servers]]
name = "fs"
transport = "stdio"
command = "npx -y 'my server'"
args = ["."]
"#;
        assert_eq!(
            parse_vibe_mcp_toml(toml).unwrap()[0].transport,
            McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "my server".to_string(), ".".to_string()],
            }
        );
    }

    #[test]
    fn test_shell_split_matches_python_shlex() {
        // Expected values are what `shlex.split` returns for the same input.
        let split = |s: &str| shell_split(s);
        let words = |w: &[&str]| Some(w.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(split("npx -y  fs"), words(&["npx", "-y", "fs"]));
        assert_eq!(split(r#"run "a b" c"#), words(&["run", "a b", "c"]));
        assert_eq!(split(r#""say \"hi\"""#), words(&[r#"say "hi""#]));
        assert_eq!(split(r#""a\nb""#), words(&[r"a\nb"]));
        assert_eq!(split(r"a\ b"), words(&["a b"]));
        assert_eq!(split("'it''s'"), words(&["its"]));
        assert_eq!(split("x ''"), words(&["x", ""]));
        assert_eq!(split("   "), words(&[]));
        // Unlike a shell, `shlex` escapes only `\` and `"` inside double
        // quotes, and keeps an escaped newline.
        assert_eq!(split(r#""a\$b""#), words(&[r"a\$b"]));
        assert_eq!(split(r#""a\`b""#), words(&[r"a\`b"]));
        assert_eq!(split("a\\\nb"), words(&["a\nb"]));
        assert_eq!(split("unterminated 'quote"), None);
        assert_eq!(split("trailing\\"), None);
    }

    #[test]
    fn test_vibe_command_with_a_space_is_written_as_a_list() {
        // A string would be split by Vibe: a path holding a space must stay
        // one word.
        let servers = vec![NormalizedMcpServer {
            name: "local".to_string(),
            transport: McpTransport::Stdio {
                command: "/Applications/My Tools/server".to_string(),
                args: vec!["--stdio".to_string()],
            },
            env: BTreeMap::new(),
        }];
        let toml = merge_vibe_mcp_toml("", &servers).unwrap();
        assert!(
            toml.contains(r#"command = ["/Applications/My Tools/server"]"#),
            "{toml}"
        );
        assert_eq!(parse_vibe_mcp_toml(&toml).unwrap(), servers);
        // A plain command stays a string.
        assert!(merge_vibe_mcp_toml("", &vibe_servers())
            .unwrap()
            .contains("command = \"npx\""));
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

[mcp_servers.api.oauth]
client_id = "conforme"
callback_port = 5555
"#;
        let parsed = parse_codex_mcp_toml(content).unwrap();
        assert_eq!(parsed.len(), 2);
        // The OAuth client settings stay Codex's: the merge keeps them.
        let merged = merge_codex_mcp_toml(content, &parsed).unwrap();
        assert!(merged.contains("client_id = \"conforme\""), "{merged}");

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
    fn test_merge_codex_mcp_toml_drops_bearer_token_on_http() {
        // Codex rejects `bearer_token` on an HTTP server too
        // (`throw_if_set("streamable_http", "bearer_token", ..)`,
        // codex-rs/config/src/mcp_types.rs): left in place it would make
        // Codex refuse the file; the token is `bearer_token_env_var` now.
        let existing =
            "[mcp_servers.api]\nurl = \"https://old.example.com/mcp\"\nbearer_token = \"literal\"\n";
        let server = NormalizedMcpServer {
            name: "api".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: BTreeMap::from([(
                    "Authorization".to_string(),
                    "Bearer ${API_TOKEN}".to_string(),
                )]),
            },
            env: BTreeMap::new(),
        };

        let toml = merge_codex_mcp_toml(existing, &[server]).unwrap();

        assert!(!toml.contains("bearer_token ="), "{toml}");
        assert!(
            toml.contains("bearer_token_env_var = \"API_TOKEN\""),
            "{toml}"
        );
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
        let servers = ZOOCODE_SHAPE.build(&servers);
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
    fn test_parse_mcp_json_reads_sse() {
        // `type: "sse"` (Claude, Gemini) or `transport: "sse"` (Devin) with a URL is
        // an SSE server; every other remote entry stays an HTTP transport.
        let parsed = parse_mcp_json(
            r#"{"mcpServers": {
                "claude": {"type": "sse", "url": "u"},
                "devin": {"transport": "sse", "url": "u"},
                "gemini": {"type": "sse", "url": "u"},
                "http": {"type": "http", "url": "u"},
                "bare": {"url": "u"}
            }}"#,
        )
        .unwrap();
        assert_eq!(parsed.len(), 5);
        for server in &parsed {
            match (server.name.as_str(), &server.transport) {
                ("claude" | "devin" | "gemini", McpTransport::Sse { url, .. }) => {
                    assert_eq!(url, "u")
                }
                ("http" | "bare", McpTransport::Http { url, .. }) => assert_eq!(url, "u"),
                (name, other) => panic!("unexpected transport for {name}: {other:?}"),
            }
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
        let parsed = OPENCODE_SHAPE.read(&mcp);
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
        let mcp = OPENCODE_SHAPE.build(&servers);
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
        let mcp = OPENCODE_SHAPE.build(&servers);
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
        let obj = ZED_SHAPE.build(&servers);
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
        let obj = GEMINI_SHAPE.build(&servers);
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
        let obj = GEMINI_SHAPE.build(&servers);
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
        let obj = OPENCODE_SHAPE.build(&servers);
        let parsed = OPENCODE_SHAPE.read(&serde_json::Value::Object(obj));
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
            ("claude", CLAUDE_SHAPE.build(&servers)),
            ("cursor", CURSOR_SHAPE.build(&servers)),
            ("copilot", COPILOT_SHAPE.build(&servers)),
            ("zoocode", ZOOCODE_SHAPE.build(&servers)),
            ("zed", ZED_SHAPE.build(&servers)),
            ("devin", DEVIN_SHAPE.build(&servers)),
        ] {
            assert!(map["remote"].get("env").is_none(), "{tool}: {map:?}");
        }
        let opencode = OPENCODE_SHAPE.build(&servers);
        assert!(opencode["remote"].get("environment").is_none());
        // Kiro documents `env` on remote servers.
        assert_eq!(KIRO_SHAPE.build(&servers)["remote"]["env"]["X"], "1");
    }

    #[test]
    fn test_cursor_remote_entry_has_no_type() {
        let map = CURSOR_SHAPE.build(&[remote_with_env("https://e.x/mcp")]);
        assert!(map["remote"].get("type").is_none(), "{map:?}");
        assert_eq!(map["remote"]["url"], "https://e.x/mcp");
    }

    #[test]
    fn test_env_refs_are_written_in_each_tool_syntax() {
        let header = |map: &serde_json::Map<String, serde_json::Value>| {
            map["remote"]["headers"]["Authorization"].clone()
        };
        let servers = vec![remote_with_env("${API_URL:-https://default}/mcp")];
        assert_eq!(header(&CLAUDE_SHAPE.build(&servers)), "Bearer ${TOKEN}");
        assert_eq!(
            CLAUDE_SHAPE.build(&servers)["remote"]["url"],
            "${API_URL:-https://default}/mcp"
        );
        for map in [
            CURSOR_SHAPE.build(&servers),
            COPILOT_SHAPE.build(&servers),
            ZOOCODE_SHAPE.build(&servers),
            DEVIN_SHAPE.build(&servers),
        ] {
            assert_eq!(header(&map), "Bearer ${env:TOKEN}");
            assert_eq!(map["remote"]["url"], "${env:API_URL}/mcp");
        }
        let kiro = KIRO_SHAPE.build(&servers);
        assert_eq!(kiro["remote"]["url"], "${API_URL}/mcp");
        let opencode = OPENCODE_SHAPE.build(&servers);
        assert_eq!(
            opencode["remote"]["headers"]["Authorization"],
            "Bearer {env:TOKEN}"
        );
        // Zed expands nothing: the text is copied as is.
        assert_eq!(header(&ZED_SHAPE.build(&servers)), "Bearer ${TOKEN}");
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
        let merge = |existing: serde_json::Value, generated, owned: Vec<&str>| {
            serde_json::Value::Object(crate::json_settings::merge_server_entries(
                Some(&existing),
                generated,
                &owned,
            ))
        };
        let server = vec![remote_with_env("https://e.x/mcp")];
        // A hand-set `"enabled": false` would keep Zed from starting the
        // server while `check` reports everything in sync.
        let zed = merge(
            serde_json::json!({"remote": {"enabled": false, "timeout": 5}}),
            ZED_SHAPE.build(&server),
            ZED_SHAPE.owned_keys(),
        );
        assert!(zed["remote"].get("enabled").is_none());
        assert_eq!(zed["remote"]["timeout"], 5);
        // `gemini mcp add -t sse` writes `type` next to `url`; it must not
        // survive beside the `httpUrl` conforme writes.
        let gemini = merge(
            serde_json::json!({"remote": {"type": "sse", "url": "https://old", "trust": true}}),
            GEMINI_SHAPE.build(&server),
            GEMINI_SHAPE.owned_keys(),
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

    #[test]
    fn test_sse_entry_shape_per_tool() {
        use serde_json::json;
        let server = NormalizedMcpServer {
            name: "legacy".to_string(),
            transport: McpTransport::Sse {
                url: "https://x.dev/sse".to_string(),
                headers: BTreeMap::from([("X-Org".to_string(), "acme".to_string())]),
            },
            env: BTreeMap::new(),
        };
        let build = |shape: &ServerShape| {
            let obj = shape.build(std::slice::from_ref(&server));
            obj.get("legacy").cloned().expect("legacy entry")
        };
        let with_headers =
            json!({"type": "sse", "url": "https://x.dev/sse", "headers": {"X-Org": "acme"}});
        for shape in [&CLAUDE_SHAPE, &CURSOR_SHAPE, &COPILOT_SHAPE, &ZOOCODE_SHAPE] {
            assert_eq!(build(shape), with_headers);
        }
        let gemini = build(&GEMINI_SHAPE);
        assert_eq!(gemini["type"], "sse");
        assert_eq!(gemini["url"], "https://x.dev/sse");
        assert!(gemini.get("httpUrl").is_none());
        assert_eq!(
            build(&DEVIN_SHAPE),
            json!({"transport": "sse", "url": "https://x.dev/sse", "headers": {"X-Org": "acme"}})
        );
        assert_eq!(
            build(&KIRO_SHAPE),
            json!({"url": "https://x.dev/sse", "headers": {"X-Org": "acme"}})
        );
        let http = NormalizedMcpServer {
            transport: McpTransport::Http {
                url: "https://x.dev/sse".to_string(),
                headers: BTreeMap::from([("X-Org".to_string(), "acme".to_string())]),
            },
            ..server.clone()
        };
        let zed_http = ZED_SHAPE.build(std::slice::from_ref(&http));
        assert_eq!(build(&ZED_SHAPE), zed_http["legacy"]);
    }

    #[test]
    fn test_antigravity_mcp_shape() {
        use serde_json::json;
        let servers = [
            NormalizedMcpServer {
                name: "fs".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/fs".to_string()],
                },
                env: BTreeMap::from([("ROOT".to_string(), "/tmp".to_string())]),
            },
            NormalizedMcpServer {
                name: "api".to_string(),
                transport: McpTransport::Http {
                    url: "https://x.dev/mcp".to_string(),
                    headers: BTreeMap::from([("X-Org".to_string(), "acme".to_string())]),
                },
                env: BTreeMap::new(),
            },
            NormalizedMcpServer {
                name: "legacy".to_string(),
                transport: McpTransport::Sse {
                    url: "https://x.dev/sse".to_string(),
                    headers: BTreeMap::from([("X-Org".to_string(), "acme".to_string())]),
                },
                env: BTreeMap::new(),
            },
        ];
        let obj = ANTIGRAVITY_SHAPE.build(&servers);
        assert_eq!(
            obj["fs"],
            json!({"command": "npx", "args": ["-y", "@mcp/fs"], "env": {"ROOT": "/tmp"}})
        );
        assert_eq!(
            obj["api"],
            json!({"serverUrl": "https://x.dev/mcp", "headers": {"X-Org": "acme"}})
        );
        assert_eq!(
            obj["legacy"],
            json!({"serverUrl": "https://x.dev/sse", "headers": {"X-Org": "acme"}})
        );
        for entry in obj.values() {
            assert!(entry.get("type").is_none());
            assert!(entry.get("url").is_none());
        }
    }
}
