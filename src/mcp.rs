use anyhow::{Context, Result};
use std::collections::BTreeMap;

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

/// Generate OpenCode MCP format: { "mcp": { "name": { "type": "local", "command": ... } } }
pub fn generate_opencode_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    if servers.is_empty() {
        return Ok(String::new());
    }

    let mut mcp = serde_json::Map::new();

    for server in servers {
        let mut entry = serde_json::Map::new();

        match &server.transport {
            McpTransport::Stdio { command, args } => {
                entry.insert(
                    "type".to_string(),
                    serde_json::Value::String("local".to_string()),
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
                    serde_json::Value::String("remote".to_string()),
                );
                entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
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

        mcp.insert(server.name.clone(), serde_json::Value::Object(entry));
    }

    let root = serde_json::json!({ "mcp": mcp });
    serde_json::to_string_pretty(&root).context("failed to serialize OpenCode MCP config")
}

/// Generate Zed context_servers format: { "context_servers": { "name": { "command": ..., "args": [...] } } }
pub fn generate_zed_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
    if servers.is_empty() {
        return Ok(String::new());
    }

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

    let root = serde_json::json!({ "context_servers": context_servers });
    serde_json::to_string_pretty(&root).context("failed to serialize Zed MCP config")
}

/// Generate Amp MCP format: { "amp.mcpServers": { "name": { "command": ..., "args": [...] } } }
pub fn generate_amp_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
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

    let root = serde_json::json!({ "amp.mcpServers": mcp_servers });
    serde_json::to_string_pretty(&root).context("failed to serialize Amp MCP config")
}

/// Generate Gemini CLI MCP format in `.gemini/settings.json`.
/// Gemini uses `mcpServers` key but does NOT use `type` field, and uses `httpUrl` for HTTP servers.
pub fn generate_gemini_mcp_json(servers: &[NormalizedMcpServer]) -> Result<String> {
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

    let root = serde_json::json!({ "mcpServers": mcp_servers });
    serde_json::to_string_pretty(&root).context("failed to serialize Gemini MCP config")
}

/// Generate OpenCode agents format for opencode.json: { "agent": { "name": { ... } } }
pub fn generate_opencode_agents_json(agents: &[crate::config::NormalizedAgent]) -> Result<String> {
    if agents.is_empty() {
        return Ok(String::new());
    }

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

    let root = serde_json::json!({ "agent": agent_map });
    serde_json::to_string_pretty(&root).context("failed to serialize OpenCode agents config")
}

/// Generate Amazon Q agent JSON files.
/// Each agent is a separate JSON file: `.amazonq/agents/<name>.json`
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

        let filename = format!("{}.json", crate::config::sanitize_name(&agent.name));
        let json = serde_json::to_string_pretty(&entry)
            .context("failed to serialize Amazon Q agent config")?;
        files.push((filename, json));
    }

    Ok(files)
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

        let transport = if transport_type == "http" || transport_type == "sse" {
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

    #[test]
    fn test_generate_opencode_mcp() {
        let servers = vec![NormalizedMcpServer {
            name: "filesystem".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@mcp/fs".to_string()],
            },
            env: BTreeMap::new(),
        }];
        let result = generate_opencode_mcp_json(&servers).unwrap();
        assert!(result.contains("\"mcp\""));
        assert!(result.contains("\"type\": \"local\""));
        assert!(result.contains("filesystem"));
        assert!(result.contains("npx"));
        assert!(!result.contains("mcpServers"));
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
        let result = generate_zed_mcp_json(&servers).unwrap();
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
        let result = generate_gemini_mcp_json(&servers).unwrap();
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
        let result = generate_gemini_mcp_json(&servers).unwrap();
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
        }];
        let result = generate_amazonq_agents_json(&agents).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "reviewer.json");
        assert!(result[0].1.contains("\"description\": \"Code review\""));
        assert!(result[0].1.contains("\"model\": \"claude-sonnet\""));
        assert!(result[0].1.contains("\"prompt\": \"Review code.\""));
        assert!(result[0].1.contains("\"codebase\""));
    }

    #[test]
    fn test_generate_opencode_agents() {
        let agents = vec![crate::config::NormalizedAgent {
            name: "reviewer".to_string(),
            description: "Code review".to_string(),
            content: "Review code.".to_string(),
            model: Some("gpt-4o".to_string()),
            tools: vec![],
        }];
        let result = generate_opencode_agents_json(&agents).unwrap();
        assert!(result.contains("\"agent\""));
        assert!(result.contains("\"reviewer\""));
        assert!(result.contains("\"description\": \"Code review\""));
        assert!(result.contains("\"mode\": \"subagent\""));
        assert!(result.contains("\"model\": \"gpt-4o\""));
        assert!(result.contains("\"prompt\": \"Review code.\""));
    }
}
