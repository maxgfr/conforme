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
            skills: true,
            agents: false,
            mcp: true,
        }
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<PathBuf> {
        // Zed reads skills from the shared `.agents/skills/` location (same as
        // Codex/Amp), so track it for orphan cleanup.
        vec![project_root.join(".agents").join("skills")]
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

        // Read skills (.agents/skills/) and MCP servers (.zed/settings.json) back.
        let skills =
            crate::skills::read_skills_from_dir(&project_root.join(".agents").join("skills"))?;
        let mut mcp_servers = Vec::new();
        let settings_path = project_root.join(".zed").join("settings.json");
        if settings_path.exists() {
            let settings = std::fs::read_to_string(&settings_path)?;
            mcp_servers = crate::mcp::parse_mcp_json(&settings)?;
        }

        Ok(NormalizedConfig {
            instructions,
            rules: Vec::new(),
            skills,
            mcp_servers,
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

        // Generate skills as .agents/skills/<name>/SKILL.md (shared SKILL.md format).
        if !config.skills.is_empty() {
            files.extend(crate::skills::generate_codex_skills(
                project_root,
                &config.skills,
            )?);
        }

        // Merge MCP config into .zed/settings.json (context_servers format).
        // `.zed/settings.json` is the user's entire Zed configuration (theme,
        // keybindings, editor settings, …), so we read any existing file and
        // replace only the managed `context_servers` key rather than clobbering it.
        if !config.mcp_servers.is_empty() {
            let config_path = project_root.join(".zed").join("settings.json");
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

            let context_servers = crate::mcp::build_zed_context_servers_object(&config.mcp_servers);
            root_map.insert(
                "context_servers".to_string(),
                serde_json::Value::Object(context_servers),
            );

            let json = serde_json::to_string_pretty(&serde_json::Value::Object(root_map))
                .context("failed to serialize .zed/settings.json")?;
            files.push((config_path, format!("{}\n", json)));
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

    #[test]
    fn test_generate_preserves_existing_zed_settings() {
        use crate::config::{McpTransport, NormalizedMcpServer};
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".zed")).unwrap();
        std::fs::write(
            tmp.path().join(".zed").join("settings.json"),
            r#"{"theme":"One Dark","vim_mode":true}"#,
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
        let settings = files
            .iter()
            .find(|(p, _)| p.ends_with("settings.json"))
            .unwrap();
        // User-authored Zed settings must be preserved
        assert!(settings.1.contains("\"theme\""));
        assert!(settings.1.contains("One Dark"));
        assert!(settings.1.contains("\"vim_mode\""));
        // And the managed context_servers key is written
        assert!(settings.1.contains("\"context_servers\""));
        assert!(settings.1.contains("\"fs\""));
        assert!(!settings.1.contains("mcpServers"));
    }
}
