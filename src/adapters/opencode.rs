use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::NormalizedConfig;

/// OpenCode adapter.
/// OpenCode reads AGENTS.md as primary, falls back to CLAUDE.md.
/// It also scans skills from .opencode/skills/, .claude/skills/, .agents/skills/.
/// MCP servers and agent definitions live inside `opencode.json` under
/// the `mcp` and `agent` keys respectively; per-project markdown agents
/// additionally live at `.opencode/agents/<name>.md`.
pub struct OpenCodeAdapter;

impl AiToolAdapter for OpenCodeAdapter {
    fn name(&self) -> &str {
        "OpenCode"
    }

    fn id(&self) -> &str {
        "opencode"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("opencode.json").exists() || project_root.join(".opencode").is_dir()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: false,
            skills: true,
            agents: true,
            mcp: true,
        }
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<PathBuf> {
        // Top-level `.opencode/` is tracked so legacy orphans like
        // `.opencode/mcp.json` and `.opencode/agents.json` (paths used by
        // earlier conforme versions) are cleaned when their config is now
        // merged into opencode.json at the project root.
        vec![
            project_root.join(".opencode"),
            project_root.join(".opencode").join("agents"),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // OpenCode reads AGENTS.md natively
        let agents_md = project_root.join("AGENTS.md");
        let instructions = if agents_md.exists() {
            std::fs::read_to_string(&agents_md)?.trim().to_string()
        } else {
            String::new()
        };
        Ok(NormalizedConfig {
            instructions,
            rules: Vec::new(),
            ..Default::default()
        })
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        // OpenCode reads AGENTS.md natively — no need to re-generate it.
        let mut files = Vec::new();

        // Generate skills as .opencode/skills/<name>/SKILL.md
        if !config.skills.is_empty() {
            files.extend(crate::skills::generate_opencode_skills(
                project_root,
                &config.skills,
            )?);
        }

        // Generate per-project agent markdown files in .opencode/agents/<name>.md
        if !config.agents.is_empty() {
            files.extend(crate::skills::generate_opencode_agents_md(
                project_root,
                &config.agents,
            )?);
        }

        // Merge MCP + agent objects into opencode.json at the project root.
        // OpenCode reads MCP from opencode.json under the `mcp` key (not from a
        // standalone .opencode/mcp.json). We read any existing opencode.json
        // to preserve user-authored keys, then replace only our managed keys.
        if !config.mcp_servers.is_empty() || !config.agents.is_empty() {
            let config_path = project_root.join("opencode.json");
            let existing = if config_path.exists() {
                let content = std::fs::read_to_string(&config_path)
                    .with_context(|| format!("failed to read {}", config_path.display()))?;
                serde_json::from_str::<serde_json::Value>(&content)
                    .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()))
            } else {
                serde_json::Value::Object(serde_json::Map::new())
            };

            let mut root_map = match existing {
                serde_json::Value::Object(m) => m,
                _ => serde_json::Map::new(),
            };
            root_map.entry("$schema".to_string()).or_insert_with(|| {
                serde_json::Value::String("https://opencode.ai/config.json".to_string())
            });

            if !config.mcp_servers.is_empty() {
                let mcp_obj = crate::mcp::build_opencode_mcp_object(&config.mcp_servers);
                root_map.insert("mcp".to_string(), serde_json::Value::Object(mcp_obj));
            }

            if !config.agents.is_empty() {
                let agent_obj = crate::mcp::build_opencode_agent_object(&config.agents);
                root_map.insert("agent".to_string(), serde_json::Value::Object(agent_obj));
            }

            let json = serde_json::to_string_pretty(&serde_json::Value::Object(root_map))
                .context("failed to serialize opencode.json")?;
            files.push((config_path, format!("{}\n", json)));
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NormalizedConfig;
    use std::path::Path;

    fn make_adapter() -> OpenCodeAdapter {
        OpenCodeAdapter
    }

    #[test]
    fn test_generate_no_files_without_mcp_or_agents() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "General instructions.".to_string(),
            rules: vec![],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_generate_with_skills() {
        use crate::config::NormalizedSkill;
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy the app".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec!["Bash".to_string()],
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0]
            .0
            .to_string_lossy()
            .contains(".opencode/skills/deploy/SKILL.md"));
        assert!(files[0].1.contains("name: deploy"));
        assert!(files[0].1.contains("description: Deploy the app"));
        // OpenCode skills do not recognize `allowed-tools`
        assert!(!files[0].1.contains("allowed-tools"));
    }

    #[test]
    fn test_generate_with_mcp() {
        use crate::config::{McpTransport, NormalizedMcpServer};
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            mcp_servers: vec![NormalizedMcpServer {
                name: "fs".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/fs".to_string()],
                },
                env: std::collections::BTreeMap::new(),
            }],
            ..Default::default()
        };
        let files = adapter.generate(tmp.path(), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, tmp.path().join("opencode.json"));
        assert!(files[0].1.contains("\"$schema\""));
        assert!(files[0].1.contains("\"mcp\""));
        assert!(files[0].1.contains("\"type\": \"local\""));
        assert!(files[0].1.contains("\"fs\""));
        // command should be a single array, not separate command + args
        assert!(files[0].1.contains("\"command\": [\n"));
        assert!(!files[0].1.contains("\"args\""));
    }

    #[test]
    fn test_generate_with_agents() {
        use crate::config::NormalizedAgent;
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            agents: vec![NormalizedAgent {
                name: "reviewer".to_string(),
                description: "Code review".to_string(),
                content: "Review code.".to_string(),
                model: Some("gpt-4o".to_string()),
                tools: vec![],
            }],
            ..Default::default()
        };
        let files = adapter.generate(tmp.path(), &config).unwrap();
        // Two files: markdown agent + opencode.json with "agent" key
        assert_eq!(files.len(), 2);
        let md_file = files
            .iter()
            .find(|(p, _)| p.to_string_lossy().contains(".opencode/agents/reviewer.md"))
            .expect("missing markdown agent file");
        assert!(md_file.1.contains("description: Code review"));
        assert!(md_file.1.contains("mode: subagent"));

        let json_file = files
            .iter()
            .find(|(p, _)| p.ends_with("opencode.json"))
            .expect("missing opencode.json");
        assert!(json_file.1.contains("\"agent\""));
        assert!(json_file.1.contains("\"reviewer\""));
        assert!(json_file.1.contains("\"mode\": \"subagent\""));
    }

    #[test]
    fn test_generate_preserves_existing_opencode_json() {
        use crate::config::{McpTransport, NormalizedMcpServer};
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("opencode.json"),
            r#"{"theme":"dark","model":"custom"}"#,
        )
        .unwrap();

        let config = NormalizedConfig {
            mcp_servers: vec![NormalizedMcpServer {
                name: "fs".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec![],
                },
                env: std::collections::BTreeMap::new(),
            }],
            ..Default::default()
        };
        let files = adapter.generate(tmp.path(), &config).unwrap();
        let json_file = &files[0].1;
        // User-authored keys must be preserved
        assert!(json_file.contains("\"theme\""));
        assert!(json_file.contains("\"dark\""));
        assert!(json_file.contains("\"model\""));
        // And the mcp key should be written
        assert!(json_file.contains("\"mcp\""));
    }
}
