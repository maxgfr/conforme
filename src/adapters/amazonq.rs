use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};

/// Amazon Q Developer adapter.
/// Rules in .amazonq/rules/*.md — plain Markdown files.
/// No YAML frontmatter documented for rules.
pub struct AmazonQAdapter;

impl AiToolAdapter for AmazonQAdapter {
    fn name(&self) -> &str {
        "Amazon Q"
    }

    fn id(&self) -> &str {
        "amazonq"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".amazonq").is_dir()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: false,
            skills: false,
            agents: true,
            mcp: true,
        }
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<PathBuf> {
        vec![
            project_root.join(".amazonq").join("rules"),
            project_root.join(".amazonq").join("cli-agents"),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let rules_dir = project_root.join(".amazonq").join("rules");
        if rules_dir.is_dir() {
            for entry in std::fs::read_dir(&rules_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    let content = std::fs::read_to_string(&path)
                        .with_context(|| format!("failed to read {}", path.display()))?;
                    let name = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    if name == "general" {
                        instructions = content.trim().to_string();
                    } else {
                        rules.push(NormalizedRule {
                            name,
                            content: content.trim().to_string(),
                            activation: ActivationMode::Always,
                        });
                    }
                }
            }
        }

        Ok(NormalizedConfig {
            instructions,
            rules,
            ..Default::default()
        })
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        let rules_dir = project_root.join(".amazonq").join("rules");
        let mut files = Vec::new();

        if !config.instructions.is_empty() {
            files.push((
                rules_dir.join("general.md"),
                format!("{}\n", config.instructions),
            ));
        }

        for rule in &config.rules {
            let filename = format!("{}.md", sanitize_name(&rule.name));
            files.push((rules_dir.join(filename), format!("{}\n", rule.content)));
        }

        // Generate agents as .amazonq/cli-agents/<name>.json
        if !config.agents.is_empty() {
            let agent_files = crate::mcp::generate_amazonq_agents_json(&config.agents)?;
            let agents_dir = project_root.join(".amazonq").join("cli-agents");
            for (filename, content) in agent_files {
                files.push((agents_dir.join(filename), format!("{}\n", content)));
            }
        }

        // Generate MCP config as .amazonq/mcp.json
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".amazonq").join("mcp.json"),
                format!("{}\n", mcp_json),
            ));
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpTransport, NormalizedConfig, NormalizedMcpServer};
    use std::path::Path;

    fn make_adapter() -> AmazonQAdapter {
        AmazonQAdapter
    }

    #[test]
    fn test_generate_instructions_only() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "General instructions.".to_string(),
            rules: vec![],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.amazonq/rules/general.md"));
        assert_eq!(files[0].1, "General instructions.\n");
    }

    #[test]
    fn test_generate_with_rules() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Main.".to_string(),
            rules: vec![
                NormalizedRule {
                    name: "TypeScript".to_string(),
                    content: "Use strict mode.".to_string(),
                    activation: ActivationMode::Always,
                },
                NormalizedRule {
                    name: "Security".to_string(),
                    content: "No eval.".to_string(),
                    activation: ActivationMode::Always,
                },
            ],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].0, Path::new("/tmp/test/.amazonq/rules/general.md"));
        assert_eq!(
            files[1].0,
            Path::new("/tmp/test/.amazonq/rules/typescript.md")
        );
        assert_eq!(
            files[2].0,
            Path::new("/tmp/test/.amazonq/rules/security.md")
        );
        assert!(files[1].1.contains("Use strict mode."));
        assert!(files[2].1.contains("No eval."));
    }

    #[test]
    fn test_generate_with_agents() {
        use crate::config::NormalizedAgent;
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![],
            agents: vec![NormalizedAgent {
                name: "reviewer".to_string(),
                description: "Code review".to_string(),
                content: "Review code.".to_string(),
                model: Some("claude-sonnet".to_string()),
                tools: vec!["codebase".to_string()],
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        let agent_file = files
            .iter()
            .find(|(p, _)| p.to_string_lossy().contains(".amazonq/cli-agents/"))
            .unwrap();
        assert!(agent_file.0.ends_with("reviewer.json"));
        assert!(agent_file.1.contains("\"description\": \"Code review\""));
        assert!(agent_file.1.contains("\"model\": \"claude-sonnet\""));
        assert!(agent_file.1.contains("\"prompt\": \"Review code.\""));
        assert!(agent_file.1.contains("\"codebase\""));
    }

    #[test]
    fn test_generate_with_mcp() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: String::new(),
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
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.amazonq/mcp.json"));
        assert!(files[0].1.contains("mcpServers"));
        assert!(files[0].1.contains("\"fs\""));
    }

    #[test]
    fn test_generate_empty_config() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert!(files.is_empty());
    }
}
