use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::config::{McpTransport, NormalizedMcpServer};

/// Generate a `.mcp.json` file (Claude Code format) from normalized MCP servers.
/// This is the common format: { "mcpServers": { "name": { ... } } }
pub fn generate_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
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

    let root = serde_json::json!({ "mcpServers": mcp_servers });
    serde_json::to_string_pretty(&root).context("failed to serialize MCP config")
}

/// Generate Copilot VS Code MCP format (uses `servers` key, not `mcpServers`).
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
            McpTransport::Http { url, .. } => {
                entry.insert(
                    "type".to_string(),
                    serde_json::Value::String("http".to_string()),
                );
                entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
            }
        }

        mcp_servers.insert(server.name.clone(), serde_json::Value::Object(entry));
    }

    let root = serde_json::json!({ "servers": mcp_servers });
    serde_json::to_string_pretty(&root).context("failed to serialize Copilot MCP config")
}

/// Parse a `.mcp.json` file (Claude Code / standard format).
#[allow(dead_code)]
pub fn parse_mcp_json(content: &str) -> Result<Vec<NormalizedMcpServer>> {
    let root: serde_json::Value =
        serde_json::from_str(content).context("failed to parse MCP JSON")?;

    let servers_key = if root.get("mcpServers").is_some() {
        "mcpServers"
    } else if root.get("servers").is_some() {
        "servers"
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

        let transport_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("stdio");

        let transport = if transport_type == "http" {
            let url = obj
                .get("url")
                .or_else(|| obj.get("httpUrl"))
                .or_else(|| obj.get("serverUrl"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
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

/// Get the MCP config path for a given tool.
#[allow(dead_code)]
pub fn mcp_path_for_tool(project_root: &Path, tool_id: &str) -> Option<PathBuf> {
    match tool_id {
        "claude" => Some(project_root.join(".mcp.json")),
        "cursor" => Some(project_root.join(".cursor").join("mcp.json")),
        "copilot" => Some(project_root.join(".vscode").join("mcp.json")),
        "roocode" => Some(project_root.join(".roo").join("mcp.json")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
