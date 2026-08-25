use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, WriteReport};
use crate::config::NormalizedConfig;

/// OpenAI Codex CLI adapter.
/// Codex reads AGENTS.md natively as its primary instruction file.
/// It also supports AGENTS.override.md and project-scoped `.codex/config.toml`
/// settings, including MCP servers.
pub struct CodexAdapter;

impl AiToolAdapter for CodexAdapter {
    fn name(&self) -> &str {
        "Codex CLI"
    }

    fn id(&self) -> &str {
        "codex"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".codex").is_dir()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: false,
            skills: true,
            agents: false,
            mcp: true,
        }
    }

    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".codex/config.toml"))
    }

    fn write(&self, project_root: &Path, config: &NormalizedConfig) -> Result<WriteReport> {
        let generated = self.generate(project_root, config)?;
        let mut report = WriteReport {
            files_written: Vec::new(),
            files_unchanged: Vec::new(),
        };
        for (path, content) in generated {
            if self.is_shared_file(&path) {
                crate::adapters::write_if_changed_atomic(&path, &content, &mut report)?;
            } else {
                crate::adapters::write_if_changed(&path, &content, &mut report)?;
            }
        }
        Ok(report)
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // Codex reads AGENTS.md directly — same as our source of truth
        let agents_md = project_root.join("AGENTS.md");
        let instructions = if agents_md.exists() {
            std::fs::read_to_string(&agents_md)?.trim().to_string()
        } else {
            String::new()
        };
        // Read skills back from the shared `.agents/skills/` location so a Codex
        // project round-trips as a source.
        let skills =
            crate::skills::read_skills_from_dir(&project_root.join(".agents").join("skills"))?;
        let codex_config = project_root.join(".codex").join("config.toml");
        let mcp_servers = if codex_config.exists() {
            let content = std::fs::read_to_string(&codex_config)?;
            crate::mcp::parse_codex_mcp_toml(&content)?
        } else {
            Vec::new()
        };

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
        // Codex reads AGENTS.md natively — no need to re-generate it
        // since AGENTS.md is already our source of truth.
        // Generate skills as .agents/skills/<name>/SKILL.md (Codex format).
        let mut files = crate::skills::generate_codex_skills(project_root, &config.skills)?;

        // Merge MCP servers into the project-scoped config. The merge preserves
        // unrelated Codex settings and server-specific options not represented
        // by conforme's normalized model.
        if !config.mcp_servers.is_empty() {
            let path = project_root.join(".codex").join("config.toml");
            let existing = if path.exists() {
                std::fs::read_to_string(&path)?
            } else {
                String::new()
            };
            let content = crate::mcp::merge_codex_mcp_toml(&existing, &config.mcp_servers)?;
            files.push((path, content));
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ActivationMode, McpTransport, NormalizedConfig, NormalizedMcpServer, NormalizedRule,
        NormalizedSkill,
    };
    use std::collections::BTreeMap;
    use std::path::Path;

    fn make_adapter() -> CodexAdapter {
        CodexAdapter
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
        // Codex reads AGENTS.md natively, no files generated without skills
        assert!(files.is_empty());
    }

    #[test]
    fn test_generate_with_rules() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Top-level.".to_string(),
            rules: vec![NormalizedRule {
                name: "TypeScript".to_string(),
                content: "Use strict mode.".to_string(),
                activation: ActivationMode::Always,
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        // No files generated — Codex reads AGENTS.md directly
        assert!(files.is_empty());
    }

    #[test]
    fn test_generate_with_skills() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Main.".to_string(),
            rules: vec![],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec!["Bash".to_string()],
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].0,
            Path::new("/tmp/test/.agents/skills/deploy/SKILL.md")
        );
        assert!(files[0].1.contains("name: deploy"));
        assert!(files[0].1.contains("description: Deploy"));
        assert!(files[0].1.contains("Run deploy."));
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

    #[test]
    fn test_generate_with_mcp() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            mcp_servers: vec![NormalizedMcpServer {
                name: "filesystem".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
                },
                env: BTreeMap::new(),
            }],
            ..Default::default()
        };

        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.codex/config.toml"));
        assert!(files[0].1.contains("[mcp_servers.filesystem]"));
        assert!(files[0].1.contains("command = \"npx\""));
    }
}
