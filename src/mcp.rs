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
                entry.remove("url");
                entry.remove("http_headers");
                entry.remove("env_http_headers");
                entry.remove("bearer_token_env_var");
                entry.remove("auth");
                entry.insert("command", value(command.clone()));

                let mut toml_args = Array::new();
                for arg in args {
                    toml_args.push(arg);
                }
                entry.insert("args", value(toml_args));

                if server.env.is_empty() {
                    entry.remove("env");
                } else {
                    let mut env = Table::new();
                    for (name, env_value) in &server.env {
                        env[name.as_str()] = value(env_value.clone());
                    }
                    let env = if entry_inline {
                        Item::Value(Value::InlineTable(env.into_inline_table()))
                    } else {
                        Item::Table(env)
                    };
                    entry.insert("env", env);
                }
            }
            McpTransport::Http { url, headers } => {
                if !server.env.is_empty() {
                    bail!(
                        "Codex HTTP MCP server `{}` cannot represent literal environment variables",
                        server.name
                    );
                }
                entry.remove("command");
                entry.remove("args");
                entry.remove("env");
                entry.remove("cwd");
                entry.remove("env_vars");
                entry.insert("url", value(url.clone()));

                if headers.is_empty() {
                    entry.remove("http_headers");
                } else {
                    let mut http_headers = Table::new();
                    for (name, header_value) in headers {
                        http_headers[name.as_str()] = value(header_value.clone());
                    }
                    let http_headers = if entry_inline {
                        Item::Value(Value::InlineTable(http_headers.into_inline_table()))
                    } else {
                        Item::Table(http_headers)
                    };
                    entry.insert("http_headers", http_headers);
                }
            }
        }
    }

    Ok(document.to_string())
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
            validate_codex_mcp_fields(entry, &["url", "http_headers", "enabled"], name)?;
            let headers = toml_string_map(entry, "http_headers", name)?;
            (McpTransport::Http { url, headers }, BTreeMap::new())
        } else if let Some(command) = command {
            validate_codex_mcp_fields(entry, &["command", "args", "env", "enabled"], name)?;
            let args = toml_string_array(entry, "args", name)?;
            let env = toml_string_map(entry, "env", name)?;
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

fn validate_codex_mcp_fields(
    entry: &toml::value::Table,
    allowed: &[&str],
    server_name: &str,
) -> Result<()> {
    for field in entry.keys() {
        if !allowed.contains(&field.as_str()) {
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

/// Generate a `.mcp.json` file (Claude Code format) from normalized MCP servers.
/// This is the common format: { "mcpServers": { "name": { ... } } }
pub fn generate_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    build_mcpservers_json(servers, "http")
}

/// Generate Roo Code's `.roo/mcp.json`.
/// Identical to the standard `mcpServers` format, except HTTP servers use
/// `type: "streamable-http"`. Roo Code does not recognize a bare `"http"`
/// transport — it only accepts `streamable-http` (modern) or `sse` (legacy).
pub fn generate_roocode_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    build_mcpservers_json(servers, "streamable-http")
}

/// Generate Continue.dev's MCP config (`.continue/mcpServers/mcp.json`).
/// Identical to the standard `mcpServers` format, except HTTP servers use
/// `type: "streamable-http"`. Continue only recognizes `stdio`, `sse`, and
/// `streamable-http` transport values — a bare `"http"` is not accepted, so
/// remote servers must be emitted as `streamable-http`.
pub fn generate_continue_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    build_mcpservers_json(servers, "streamable-http")
}

/// Shared builder for the standard `{ "mcpServers": { "name": { ... } } }` format.
/// `http_type` is the value written for the `type` field of HTTP servers
/// (`"http"` for most tools, `"streamable-http"` for Roo Code).
fn build_mcpservers_json(servers: &[NormalizedMcpServer], http_type: &str) -> Result<String> {
    if servers.is_empty() {
        return Ok(String::new());
    }

    let mut mcp_servers = serde_json::Map::new();

    for server in servers {
        let mut entry = serde_json::Map::new();

        match &server.transport {
            McpTransport::Stdio { command, args } => {
                entry.insert(
                    "type".to_string(),
                    serde_json::Value::String("stdio".to_string()),
                );
                entry.insert(
                    "command".to_string(),
                    serde_json::Value::String(command.clone()),
                );
                let json_args: Vec<serde_json::Value> = args
                    .iter()
                    .map(|a| serde_json::Value::String(a.clone()))
                    .collect();
                entry.insert("args".to_string(), serde_json::Value::Array(json_args));
            }
            McpTransport::Http { url, headers } => {
                entry.insert(
                    "type".to_string(),
                    serde_json::Value::String(http_type.to_string()),
                );
                entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
                if !headers.is_empty() {
                    let h: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    entry.insert("headers".to_string(), serde_json::Value::Object(h));
                }
            }
        }

        if !server.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = server
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            entry.insert("env".to_string(), serde_json::Value::Object(env_obj));
        }

        mcp_servers.insert(server.name.clone(), serde_json::Value::Object(entry));
    }

    let root = serde_json::json!({ "mcpServers": mcp_servers });
    serde_json::to_string_pretty(&root).context("failed to serialize MCP config")
}

/// Generate Copilot VS Code MCP format (uses `servers` key, not `mcpServers`).
/// Supports `env` for stdio and `headers` for HTTP transports.
pub fn generate_copilot_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    if servers.is_empty() {
        return Ok(String::new());
    }

    let mut mcp_servers = serde_json::Map::new();

    for server in servers {
        let mut entry = serde_json::Map::new();

        match &server.transport {
            McpTransport::Stdio { command, args } => {
                entry.insert(
                    "type".to_string(),
                    serde_json::Value::String("stdio".to_string()),
                );
                entry.insert(
                    "command".to_string(),
                    serde_json::Value::String(command.clone()),
                );
                let json_args: Vec<serde_json::Value> = args
                    .iter()
                    .map(|a| serde_json::Value::String(a.clone()))
                    .collect();
                entry.insert("args".to_string(), serde_json::Value::Array(json_args));
            }
            McpTransport::Http { url, headers } => {
                entry.insert(
                    "type".to_string(),
                    serde_json::Value::String("http".to_string()),
                );
                entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
                if !headers.is_empty() {
                    let h: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    entry.insert("headers".to_string(), serde_json::Value::Object(h));
                }
            }
        }

        if !server.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = server
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            entry.insert("env".to_string(), serde_json::Value::Object(env_obj));
        }

        mcp_servers.insert(server.name.clone(), serde_json::Value::Object(entry));
    }

    let root = serde_json::json!({ "servers": mcp_servers });
    serde_json::to_string_pretty(&root).context("failed to serialize Copilot MCP config")
}

/// Generate Windsurf MCP format (used inside the `mcp` object of opencode.json or standalone).
/// Windsurf infers transport from shape: stdio uses `command`/`args`, HTTP uses `serverUrl`.
/// No `type` field is emitted (Windsurf does not document one).
pub fn generate_windsurf_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    if servers.is_empty() {
        return Ok(String::new());
    }

    let mut mcp_servers = serde_json::Map::new();

    for server in servers {
        let mut entry = serde_json::Map::new();

        match &server.transport {
            McpTransport::Stdio { command, args } => {
                entry.insert(
                    "command".to_string(),
                    serde_json::Value::String(command.clone()),
                );
                let json_args: Vec<serde_json::Value> = args
                    .iter()
                    .map(|a| serde_json::Value::String(a.clone()))
                    .collect();
                entry.insert("args".to_string(), serde_json::Value::Array(json_args));
            }
            McpTransport::Http { url, headers } => {
                entry.insert(
                    "serverUrl".to_string(),
                    serde_json::Value::String(url.clone()),
                );
                if !headers.is_empty() {
                    let h: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    entry.insert("headers".to_string(), serde_json::Value::Object(h));
                }
            }
        }

        if !server.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = server
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            entry.insert("env".to_string(), serde_json::Value::Object(env_obj));
        }

        mcp_servers.insert(server.name.clone(), serde_json::Value::Object(entry));
    }

    let root = serde_json::json!({ "mcpServers": mcp_servers });
    serde_json::to_string_pretty(&root).context("failed to serialize Windsurf MCP config")
}

/// Build the OpenCode `mcp` object (not a full file — `opencode.json` is merged by the adapter).
/// OpenCode format: stdio uses `command: [cmd, ...args]` as a single array;
/// env var key is `environment` (not `env`); remote servers use `url`.
pub fn build_opencode_mcp_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    let mut mcp = serde_json::Map::new();

    for server in servers {
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
            }
            McpTransport::Http { url, headers } => {
                entry.insert(
                    "type".to_string(),
                    serde_json::Value::String("remote".to_string()),
                );
                entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
                if !headers.is_empty() {
                    let h: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    entry.insert("headers".to_string(), serde_json::Value::Object(h));
                }
            }
        }

        if !server.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = server
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            entry.insert(
                "environment".to_string(),
                serde_json::Value::Object(env_obj),
            );
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

/// Build the Zed `context_servers` object (not a full file — `.zed/settings.json`
/// is merged by the adapter so user-authored settings such as theme and
/// keybindings are preserved). Flat shape: stdio uses `command`/`args`/`env`,
/// remote uses `url`/`headers`; no `type` field.
pub fn build_zed_context_servers_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    let mut context_servers = serde_json::Map::new();

    for server in servers {
        let mut entry = serde_json::Map::new();

        match &server.transport {
            McpTransport::Stdio { command, args } => {
                entry.insert(
                    "command".to_string(),
                    serde_json::Value::String(command.clone()),
                );
                let json_args: Vec<serde_json::Value> = args
                    .iter()
                    .map(|a| serde_json::Value::String(a.clone()))
                    .collect();
                entry.insert("args".to_string(), serde_json::Value::Array(json_args));
            }
            McpTransport::Http { url, headers } => {
                entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
                if !headers.is_empty() {
                    let h: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    entry.insert("headers".to_string(), serde_json::Value::Object(h));
                }
            }
        }

        if !server.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = server
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            entry.insert("env".to_string(), serde_json::Value::Object(env_obj));
        }

        context_servers.insert(server.name.clone(), serde_json::Value::Object(entry));
    }

    context_servers
}

/// Build the Amp `amp.mcpServers` object (not a full file — `.amp/settings.json`
/// is merged by the adapter so user-authored workspace settings are preserved).
/// Amp infers the transport from the entry's shape: stdio uses `command`/`args`,
/// remote uses `url` (+ optional `headers`). No `type` field is emitted.
pub fn build_amp_mcp_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    let mut mcp_servers = serde_json::Map::new();

    for server in servers {
        let mut entry = serde_json::Map::new();

        match &server.transport {
            McpTransport::Stdio { command, args } => {
                entry.insert(
                    "command".to_string(),
                    serde_json::Value::String(command.clone()),
                );
                let json_args: Vec<serde_json::Value> = args
                    .iter()
                    .map(|a| serde_json::Value::String(a.clone()))
                    .collect();
                entry.insert("args".to_string(), serde_json::Value::Array(json_args));
            }
            McpTransport::Http { url, headers } => {
                entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
                if !headers.is_empty() {
                    let h: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    entry.insert("headers".to_string(), serde_json::Value::Object(h));
                }
            }
        }

        if !server.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = server
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            entry.insert("env".to_string(), serde_json::Value::Object(env_obj));
        }

        mcp_servers.insert(server.name.clone(), serde_json::Value::Object(entry));
    }

    mcp_servers
}

/// Build the Gemini CLI `mcpServers` object (not a full file — `.gemini/settings.json`
/// is merged by the adapter so user-authored settings are preserved).
/// Gemini does NOT use a `type` field and uses `httpUrl` for HTTP servers.
pub fn build_gemini_mcp_object(
    servers: &[NormalizedMcpServer],
) -> serde_json::Map<String, serde_json::Value> {
    let mut mcp_servers = serde_json::Map::new();

    for server in servers {
        let mut entry = serde_json::Map::new();

        match &server.transport {
            McpTransport::Stdio { command, args } => {
                entry.insert(
                    "command".to_string(),
                    serde_json::Value::String(command.clone()),
                );
                let json_args: Vec<serde_json::Value> = args
                    .iter()
                    .map(|a| serde_json::Value::String(a.clone()))
                    .collect();
                entry.insert("args".to_string(), serde_json::Value::Array(json_args));
            }
            McpTransport::Http { url, headers } => {
                // Gemini uses "httpUrl" instead of "url"
                entry.insert(
                    "httpUrl".to_string(),
                    serde_json::Value::String(url.clone()),
                );
                if !headers.is_empty() {
                    let h: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    entry.insert("headers".to_string(), serde_json::Value::Object(h));
                }
            }
        }

        if !server.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = server
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            entry.insert("env".to_string(), serde_json::Value::Object(env_obj));
        }

        mcp_servers.insert(server.name.clone(), serde_json::Value::Object(entry));
    }

    mcp_servers
}

/// Build the OpenCode `agent` object for `opencode.json` (merged by the adapter).
pub fn build_opencode_agent_object(
    agents: &[crate::config::NormalizedAgent],
) -> serde_json::Map<String, serde_json::Value> {
    let mut agent_map = serde_json::Map::new();

    for agent in agents {
        let mut entry = serde_json::Map::new();
        if !agent.description.is_empty() {
            entry.insert(
                "description".to_string(),
                serde_json::Value::String(agent.description.clone()),
            );
        }
        entry.insert(
            "mode".to_string(),
            serde_json::Value::String("subagent".to_string()),
        );
        if let Some(model) = &agent.model {
            entry.insert(
                "model".to_string(),
                serde_json::Value::String(model.clone()),
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

/// Generate Amazon Q agent JSON files.
/// Each agent is a separate JSON file: `.amazonq/cli-agents/<name>.json`
pub fn generate_amazonq_agents_json(
    agents: &[crate::config::NormalizedAgent],
) -> Result<Vec<(String, String)>> {
    let mut files = Vec::new();

    for agent in agents {
        let mut entry = serde_json::Map::new();

        if !agent.description.is_empty() {
            entry.insert(
                "description".to_string(),
                serde_json::Value::String(agent.description.clone()),
            );
        }
        if let Some(model) = &agent.model {
            entry.insert(
                "model".to_string(),
                serde_json::Value::String(model.clone()),
            );
        }
        if !agent.tools.is_empty() {
            let json_tools: Vec<serde_json::Value> = agent
                .tools
                .iter()
                .map(|t| serde_json::Value::String(t.clone()))
                .collect();
            entry.insert("tools".to_string(), serde_json::Value::Array(json_tools));
        }
        if !agent.content.is_empty() {
            entry.insert(
                "prompt".to_string(),
                serde_json::Value::String(agent.content.clone()),
            );
        }
        // Give the generated agent access to the rules conforme also writes to
        // `.amazonq/rules/`, and let it pick up MCP servers from the sibling
        // `.amazonq/mcp.json` (both otherwise invisible to a bare agent file).
        entry.insert(
            "resources".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String(
                "file://.amazonq/rules/**/*.md".to_string(),
            )]),
        );
        entry.insert(
            "useLegacyMcpJson".to_string(),
            serde_json::Value::Bool(true),
        );

        let filename = format!("{}.json", crate::config::sanitize_name(&agent.name));
        let json = serde_json::to_string_pretty(&entry)
            .context("failed to serialize Amazon Q agent config")?;
        files.push((filename, json));
    }

    Ok(files)
}

/// Parse an MCP config file into normalized servers. Handles every key conforme
/// emits — `mcpServers` (standard), `servers` (Copilot/VS Code),
/// `context_servers` (Zed) and `amp.mcpServers` (Amp) — and infers the transport
/// from the entry's shape when there is no explicit `type` field (Gemini
/// `httpUrl`, Windsurf `serverUrl`, Zed/Amp remote `url`).
pub fn parse_mcp_json(content: &str) -> Result<Vec<NormalizedMcpServer>> {
    let root: serde_json::Value =
        serde_json::from_str(content).context("failed to parse MCP JSON")?;

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
env_http_headers = { Authorization = "TOKEN" }
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
        assert!(!stdio_result.contains("auth"));
        assert!(!stdio_result.contains("bearer_token_env_var"));
        assert!(!stdio_result.contains("env_http_headers"));
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

    #[test]
    fn test_parse_codex_mcp_toml_rejects_unsupported_auth() {
        let content = r#"[mcp_servers.private]
url = "https://example.com/mcp"
bearer_token_env_var = "MCP_TOKEN"
"#;

        let error = parse_codex_mcp_toml(content).unwrap_err();

        assert!(error.to_string().contains("cannot safely migrate"));
        assert!(error.to_string().contains("bearer_token_env_var"));
    }

    #[test]
    fn test_parse_codex_mcp_toml_rejects_unrepresentable_stdio_options() {
        for field in ["cwd = \"tools\"", "env_vars = [\"TOKEN\"]"] {
            let content = format!("[mcp_servers.private]\ncommand = \"node\"\n{field}\n");
            let error = parse_codex_mcp_toml(&content).unwrap_err();

            assert!(error.to_string().contains("cannot safely migrate"));
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
    fn test_merge_codex_mcp_toml_rejects_http_literal_env() {
        let server = NormalizedMcpServer {
            name: "api".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::from([("TOKEN".to_string(), "secret".to_string())]),
        };

        let error = merge_codex_mcp_toml("", &[server]).unwrap_err();

        assert!(error
            .to_string()
            .contains("cannot represent literal environment variables"));
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
    fn test_generate_roocode_mcp_json_http_uses_streamable_http() {
        let servers = vec![NormalizedMcpServer {
            name: "context7".to_string(),
            transport: McpTransport::Http {
                url: "https://mcp.context7.com/mcp".to_string(),
                headers: BTreeMap::from([("x-api-key".to_string(), "secret".to_string())]),
            },
            env: BTreeMap::new(),
        }];
        let result = generate_roocode_mcp_json(&servers).unwrap();
        // Roo Code requires `streamable-http`, never a bare `http` type value.
        assert!(result.contains("\"type\": \"streamable-http\""));
        assert!(!result.contains("\"type\": \"http\""));
        assert!(result.contains("mcpServers"));
        assert!(result.contains("https://mcp.context7.com/mcp"));
        assert!(result.contains("x-api-key"));
    }

    #[test]
    fn test_generate_roocode_mcp_json_stdio_matches_standard() {
        // For stdio servers Roo Code uses the same shape as the standard format.
        let servers = vec![NormalizedMcpServer {
            name: "fs".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/fs".to_string()],
            },
            env: BTreeMap::new(),
        }];
        assert_eq!(
            generate_roocode_mcp_json(&servers).unwrap(),
            generate_mcp_json(&servers).unwrap()
        );
    }

    #[test]
    fn test_generate_continue_mcp_json_http_uses_streamable_http() {
        // Continue.dev rejects a bare `http` transport — HTTP servers must use
        // `streamable-http` (or `sse`).
        let servers = vec![NormalizedMcpServer {
            name: "context7".to_string(),
            transport: McpTransport::Http {
                url: "https://mcp.context7.com/mcp".to_string(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::new(),
        }];
        let result = generate_continue_mcp_json(&servers).unwrap();
        assert!(result.contains("\"type\": \"streamable-http\""));
        assert!(!result.contains("\"type\": \"http\""));
        assert!(result.contains("mcpServers"));
        assert!(result.contains("https://mcp.context7.com/mcp"));
    }

    #[test]
    fn test_generate_continue_mcp_json_stdio_matches_standard() {
        // For stdio servers Continue uses the same shape as the standard format.
        let servers = vec![NormalizedMcpServer {
            name: "fs".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/fs".to_string()],
            },
            env: BTreeMap::new(),
        }];
        assert_eq!(
            generate_continue_mcp_json(&servers).unwrap(),
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
    fn test_parse_mcp_json_streamable_http_roundtrip() {
        // A Roo Code config with `streamable-http` must parse back to an HTTP transport.
        let servers = vec![NormalizedMcpServer {
            name: "ctx".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::new(),
        }];
        let json = generate_roocode_mcp_json(&servers).unwrap();
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
    fn test_generate_windsurf_mcp_stdio() {
        let servers = vec![NormalizedMcpServer {
            name: "fs".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/fs".to_string()],
            },
            env: BTreeMap::new(),
        }];
        let result = generate_windsurf_mcp_json(&servers).unwrap();
        assert!(result.contains("\"mcpServers\""));
        assert!(result.contains("\"command\": \"npx\""));
        // Windsurf does NOT use "type" field
        assert!(!result.contains("\"type\""));
    }

    #[test]
    fn test_generate_windsurf_mcp_http() {
        let servers = vec![NormalizedMcpServer {
            name: "api".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::new(),
        }];
        let result = generate_windsurf_mcp_json(&servers).unwrap();
        // Windsurf uses "serverUrl" not "url"
        assert!(result.contains("\"serverUrl\": \"https://example.com/mcp\""));
        assert!(!result.contains("\"\"url\""));
        assert!(!result.contains("\"type\""));
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
    fn test_generate_amazonq_agents() {
        let agents = vec![crate::config::NormalizedAgent {
            name: "reviewer".to_string(),
            description: "Code review".to_string(),
            content: "Review code.".to_string(),
            model: Some("claude-sonnet".to_string()),
            tools: vec!["codebase".to_string()],
            ..Default::default()
        }];
        let result = generate_amazonq_agents_json(&agents).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "reviewer.json");
        assert!(result[0].1.contains("\"description\": \"Code review\""));
        assert!(result[0].1.contains("\"model\": \"claude-sonnet\""));
        assert!(result[0].1.contains("\"prompt\": \"Review code.\""));
        assert!(result[0].1.contains("\"codebase\""));
        // Generated agents load the synced rules and legacy MCP config.
        assert!(result[0].1.contains("file://.amazonq/rules/**/*.md"));
        assert!(result[0].1.contains("\"useLegacyMcpJson\": true"));
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
            model: Some("gpt-4o".to_string()),
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
        assert_eq!(entry.get("model").unwrap().as_str().unwrap(), "gpt-4o");
        assert_eq!(
            entry.get("prompt").unwrap().as_str().unwrap(),
            "Review code."
        );
    }
}
