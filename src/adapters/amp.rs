use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
use crate::config::NormalizedConfig;

/// Amp adapter.
/// Reads AGENTS.md natively as primary, falls back to AGENT.md or CLAUDE.md.
/// Global config at ~/.config/amp/AGENTS.md.
/// Settings at .amp/settings.json (or .amp/settings.jsonc).
/// Skills in .agents/skills/ (shared format with Codex).
/// MCP in the workspace settings file, under `amp.mcpServers`.
pub struct AmpAdapter;

/// Amp's workspace settings file: `.amp/settings.json`, or the JSONC spelling
/// `.amp/settings.jsonc` when only that one exists, so conforme merges into
/// the file Amp actually reads instead of creating a second one beside it.
fn settings_path(project_root: &Path) -> PathBuf {
    let dir = project_root.join(".amp");
    let json = dir.join("settings.json");
    let jsonc = dir.join("settings.jsonc");
    if !json.exists() && jsonc.exists() {
        jsonc
    } else {
        json
    }
}

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

    /// `.amp/settings.json` is Amp's whole workspace settings blob; conforme
    /// only merges `amp.mcpServers` into it, so `remove`/`migrate` must
    /// never delete the file wholesale.
    fn reads_agents_md(&self) -> bool {
        true
    }

    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".amp/settings.json"))
            || path.ends_with(Path::new(".amp/settings.jsonc"))
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![ManagedDir::subdirs(
            project_root.join(".agents").join("skills"),
        )]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // Read skills (.agents/skills/) and MCP (.amp/settings.json, keyed under
        // `amp.mcpServers`) back so an Amp project round-trips as a source.
        // Amp searches each skills root up to five directories deep.
        let skills =
            crate::skills::read_skills_recursive(&project_root.join(".agents").join("skills"), 5)?;
        let mut mcp_servers = Vec::new();
        let settings_path = settings_path(project_root);
        if settings_path.exists() {
            let settings = std::fs::read_to_string(&settings_path)?;
            mcp_servers = crate::mcp::parse_mcp_json(&settings)?;
        }

        crate::markdown::read_native_agents_md(
            project_root,
            &["AGENTS.md", "AGENT.md", "CLAUDE.md"],
            NormalizedConfig {
                instructions: String::new(),
                rules: Vec::new(),
                skills,
                mcp_servers,
                ..Default::default()
            },
        )
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

        // Merge MCP config into .amp/settings.json under the `amp.mcpServers` key.
        // That file is Amp's whole workspace settings blob, so we read any
        // existing file and replace only the managed key rather than clobbering
        // user-authored settings.
        files.extend(crate::json_settings::server_settings_file(
            &settings_path(project_root),
            "amp.mcpServers",
            crate::mcp::build_amp_mcp_object(&config.mcp_servers),
            crate::mcp::AMP_OWNED_SERVER_KEYS,
            &[],
        )?);

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
                ..Default::default()
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
    fn test_read_falls_back_to_agent_md_then_claude_md() {
        let adapter = make_adapter();

        // AGENT.md (singular) is used when AGENTS.md is absent.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("AGENT.md"), "From AGENT.md").unwrap();
        assert_eq!(
            adapter.read(tmp.path()).unwrap().instructions,
            "From AGENT.md"
        );

        // CLAUDE.md is used when neither AGENTS.md nor AGENT.md exist.
        let tmp2 = tempfile::tempdir().unwrap();
        std::fs::write(tmp2.path().join("CLAUDE.md"), "From CLAUDE.md").unwrap();
        assert_eq!(
            adapter.read(tmp2.path()).unwrap().instructions,
            "From CLAUDE.md"
        );

        // AGENTS.md takes priority over the fallbacks.
        let tmp3 = tempfile::tempdir().unwrap();
        std::fs::write(tmp3.path().join("AGENTS.md"), "Primary").unwrap();
        std::fs::write(tmp3.path().join("AGENT.md"), "Fallback").unwrap();
        assert_eq!(adapter.read(tmp3.path()).unwrap().instructions, "Primary");
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
