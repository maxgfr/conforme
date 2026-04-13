use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::NormalizedConfig;

/// Amp (Sourcegraph) adapter.
/// Reads AGENTS.md natively as primary, falls back to AGENT.md or CLAUDE.md.
/// Global config at ~/.config/amp/AGENTS.md.
/// Settings at .amp/settings.json.
/// Skills in .agents/skills/ (shared format with Codex).
/// MCP in .amp/settings.json.
pub struct AmpAdapter;

impl AiToolAdapter for AmpAdapter {
    fn name(&self) -> &str {
        "Amp"
    }

    fn id(&self) -> &str {
        "amp"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".amp").is_dir()
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
        vec![project_root.join(".agents").join("skills")]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // Amp reads AGENTS.md natively
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
        // Amp reads AGENTS.md natively — no need to re-generate it.
        // But we generate skills and MCP config.
        let mut files = Vec::new();

        // Generate skills as .agents/skills/<name>/SKILL.md (shared Codex format)
        if !config.skills.is_empty() {
            files.extend(crate::skills::generate_codex_skills(
                project_root,
                &config.skills,
            )?);
        }

        // Generate MCP config as .amp/settings.json (amp.mcpServers format)
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_amp_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".amp").join("settings.json"),
                format!("{}\n", mcp_json),
            ));
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpTransport, NormalizedConfig, NormalizedMcpServer, NormalizedSkill};
    use std::path::Path;

    fn make_adapter() -> AmpAdapter {
        AmpAdapter
    }

    #[test]
    fn test_generate_no_files_without_skills_or_mcp() {
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
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy the app".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec![],
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
        assert!(files[0].1.contains("description: Deploy the app"));
    }

    #[test]
    fn test_generate_with_mcp() {
        let adapter = make_adapter();
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
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.amp/settings.json"));
        assert!(files[0].1.contains("amp.mcpServers"));
        assert!(files[0].1.contains("\"fs\""));
    }
}
