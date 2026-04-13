use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::NormalizedConfig;

/// OpenCode adapter.
/// OpenCode reads AGENTS.md as primary, falls back to CLAUDE.md.
/// It also scans skills from .opencode/skills/, .claude/skills/, .agents/skills/.
/// Since AGENTS.md is our source of truth and OpenCode reads it natively,
/// this adapter is mostly a pass-through.
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
        // But we generate skills, MCP and agents config.
        let mut files = Vec::new();

        // Generate skills as .opencode/skills/<name>/SKILL.md
        if !config.skills.is_empty() {
            files.extend(crate::skills::generate_opencode_skills(
                project_root,
                &config.skills,
            )?);
        }

        // Generate MCP config as opencode.json with "mcp" key
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_opencode_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".opencode").join("mcp.json"),
                format!("{}\n", mcp_json),
            ));
        }

        // Generate agents config as .opencode/agents.json with "agent" key
        if !config.agents.is_empty() {
            let agents_json = crate::mcp::generate_opencode_agents_json(&config.agents)?;
            files.push((
                project_root.join(".opencode").join("agents.json"),
                format!("{}\n", agents_json),
            ));
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
        assert!(files[0].1.contains("allowed-tools: Bash"));
    }

    #[test]
    fn test_generate_with_mcp() {
        use crate::config::{McpTransport, NormalizedMcpServer};
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
        assert_eq!(files[0].0, Path::new("/tmp/test/.opencode/mcp.json"));
        assert!(files[0].1.contains("\"mcp\""));
        assert!(files[0].1.contains("\"type\": \"local\""));
        assert!(files[0].1.contains("\"fs\""));
    }

    #[test]
    fn test_generate_with_agents() {
        use crate::config::NormalizedAgent;
        let adapter = make_adapter();
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
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.opencode/agents.json"));
        assert!(files[0].1.contains("\"agent\""));
        assert!(files[0].1.contains("\"reviewer\""));
        assert!(files[0].1.contains("\"mode\": \"subagent\""));
    }
}
