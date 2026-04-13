use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::NormalizedConfig;

/// Gemini CLI adapter.
/// Uses GEMINI.md discovered hierarchically.
/// Supports @path/to/file.md imports. No per-rule files — single GEMINI.md.
pub struct GeminiAdapter;

impl AiToolAdapter for GeminiAdapter {
    fn name(&self) -> &str {
        "Gemini CLI"
    }

    fn id(&self) -> &str {
        "gemini"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("GEMINI.md").exists() || project_root.join(".gemini").is_dir()
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
        vec![
            project_root.join(".gemini").join("agents"),
            project_root.join(".gemini").join("skills"),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let gemini_md = project_root.join("GEMINI.md");
        let instructions = if gemini_md.exists() {
            std::fs::read_to_string(&gemini_md)
                .with_context(|| format!("failed to read {}", gemini_md.display()))?
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
        // Gemini uses a single GEMINI.md — no per-rule frontmatter, no activation modes.
        // All content is merged into one file.
        let mut content = config.instructions.clone();

        for rule in &config.rules {
            content.push_str("\n\n## ");
            content.push_str(&rule.name);
            content.push_str("\n\n");
            content.push_str(&rule.content);
        }

        let mut files = Vec::new();

        // Only generate GEMINI.md if there's actual content
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            files.push((project_root.join("GEMINI.md"), format!("{}\n", trimmed)));
        }

        // Generate skills as .gemini/skills/<name>/SKILL.md
        if !config.skills.is_empty() {
            files.extend(crate::skills::generate_gemini_skills(
                project_root,
                &config.skills,
            )?);
        }

        // Generate subagents as .gemini/agents/<name>.md
        if !config.agents.is_empty() {
            files.extend(crate::skills::generate_gemini_agents(
                project_root,
                &config.agents,
            )?);
        }

        // Generate MCP config as .gemini/settings.json (Gemini-specific format: no type field, httpUrl for HTTP)
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_gemini_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".gemini").join("settings.json"),
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

    fn make_adapter() -> GeminiAdapter {
        GeminiAdapter
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
        assert_eq!(files[0].0, Path::new("/tmp/test/GEMINI.md"));
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
        assert_eq!(files[0].0, Path::new("/tmp/test/GEMINI.md"));
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
        assert_eq!(files[0].0, Path::new("/tmp/test/GEMINI.md"));
        assert_eq!(files[1].0, Path::new("/tmp/test/.gemini/settings.json"));
        assert!(files[1].1.contains("mcpServers"));
        assert!(files[1].1.contains("\"fs\""));
        // Gemini MCP should NOT have "type" field
        assert!(!files[1].1.contains("\"type\""));
    }

    #[test]
    fn test_generate_with_skills() {
        use crate::config::NormalizedSkill;
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Hello.".to_string(),
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
        assert_eq!(files.len(), 2);
        let skill_file = files
            .iter()
            .find(|(p, _)| p.to_string_lossy().contains(".gemini/skills/"))
            .unwrap();
        assert!(skill_file.0.ends_with("SKILL.md"));
        assert!(skill_file.1.contains("name: deploy"));
        assert!(skill_file.1.contains("description: Deploy the app"));
        // Gemini skills should NOT include allowed-tools
        assert!(!skill_file.1.contains("allowed-tools"));
    }

    #[test]
    fn test_generate_with_agents() {
        use crate::config::NormalizedAgent;
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Hello.".to_string(),
            rules: vec![],
            agents: vec![NormalizedAgent {
                name: "reviewer".to_string(),
                description: "Code review".to_string(),
                content: "Review code.".to_string(),
                model: Some("gemini-3-flash".to_string()),
                tools: vec!["read_file".to_string()],
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 2);
        let agent_file = files
            .iter()
            .find(|(p, _)| p.to_string_lossy().contains(".gemini/agents/"))
            .unwrap();
        assert!(agent_file.0.ends_with("reviewer.md"));
        assert!(agent_file.1.contains("kind: local"));
        assert!(agent_file.1.contains("description: Code review"));
        assert!(agent_file.1.contains("model: gemini-3-flash"));
        assert!(agent_file.1.contains("- read_file"));
        assert!(agent_file.1.contains("Review code."));
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
        // Empty config should generate no files
        assert!(files.is_empty());
    }
}
