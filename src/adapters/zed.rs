use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::NormalizedConfig;

/// Zed AI adapter.
/// Uses .rules as primary file.
/// Fallback priority: .rules → .cursorrules → .windsurfrules → .clinerules → .github/copilot-instructions.md → AGENT.md → AGENTS.md → CLAUDE.md → GEMINI.md
/// Single file, no per-rule frontmatter.
pub struct ZedAdapter;

impl AiToolAdapter for ZedAdapter {
    fn name(&self) -> &str {
        "Zed AI"
    }

    fn id(&self) -> &str {
        "zed"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".rules").exists()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: false,
            skills: false,
            agents: false,
            mcp: true,
        }
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let rules_file = project_root.join(".rules");
        let instructions = if rules_file.exists() {
            std::fs::read_to_string(&rules_file)
                .with_context(|| format!("failed to read {}", rules_file.display()))?
                .trim()
                .to_string()
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
        // Zed uses a single .rules file — no frontmatter, no activation modes.
        let mut content = config.instructions.clone();

        for rule in &config.rules {
            content.push_str("\n\n## ");
            content.push_str(&rule.name);
            content.push_str("\n\n");
            content.push_str(&rule.content);
        }

        let mut files = vec![(project_root.join(".rules"), format!("{}\n", content.trim()))];

        // Generate MCP config as .zed/settings.json (context_servers format)
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_zed_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".zed").join("settings.json"),
                format!("{}\n", mcp_json),
            ));
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ActivationMode, NormalizedConfig, NormalizedRule};
    use std::path::Path;

    fn make_adapter() -> ZedAdapter {
        ZedAdapter
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
        assert_eq!(files[0].0, Path::new("/tmp/test/.rules"));
        assert_eq!(files[0].1, "General instructions.\n");
    }

    #[test]
    fn test_generate_with_rules() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Top-level.".to_string(),
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
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.rules"));
        let content = &files[0].1;
        assert!(content.contains("Top-level."));
        assert!(content.contains("## TypeScript"));
        assert!(content.contains("Use strict mode."));
        assert!(content.contains("## Security"));
        assert!(content.contains("No eval."));
    }

    #[test]
    fn test_generate_with_mcp() {
        use crate::config::{McpTransport, NormalizedMcpServer};
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Hello.".to_string(),
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
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, Path::new("/tmp/test/.rules"));
        assert_eq!(files[1].0, Path::new("/tmp/test/.zed/settings.json"));
        assert!(files[1].1.contains("context_servers"));
        assert!(files[1].1.contains("\"fs\""));
        assert!(!files[1].1.contains("mcpServers"));
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
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.rules"));
        assert_eq!(files[0].1, "\n");
    }
}
