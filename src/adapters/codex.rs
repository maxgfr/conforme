use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir, WriteReport};
use crate::config::NormalizedConfig;

/// OpenAI Codex CLI adapter.
/// Codex reads AGENTS.md natively as its primary instruction file.
/// It also supports AGENTS.override.md and project-scoped `.codex/config.toml`
/// settings, including MCP servers.
pub struct CodexAdapter;

/// Codex reads the shared `AGENTS.md` (never the personal override).
const INSTRUCTION_FILES: &[&str] = &["AGENTS.md"];

/// Whether a `.codex/agents/*.toml` file overrides a built-in role. Codex
/// names a discovered agent by its `name` key, not its file name, so either
/// one naming a built-in role makes the file the user's override.
fn is_builtin_agent_override(path: &Path) -> bool {
    let stem_is_builtin = path
        .file_stem()
        .is_some_and(|stem| crate::skills::is_codex_builtin_agent(&stem.to_string_lossy()));
    stem_is_builtin
        || std::fs::read_to_string(path)
            .ok()
            .and_then(|content| content.parse::<toml::Table>().ok())
            .and_then(|table| {
                table
                    .get("name")
                    .and_then(toml::Value::as_str)
                    .map(|name| crate::skills::is_codex_builtin_agent(name.trim()))
            })
            .unwrap_or(false)
}

/// Whether conforme could have written this agent file: it sits directly in
/// `.codex/agents/`, holds only the keys conforme writes, and its file stem
/// is the sanitized form of its `name` key. Codex finds an agent by its
/// `name` anywhere under the folder, so any other file is the user's own.
fn is_conforme_written_agent(path: &Path) -> bool {
    let in_agents_dir = path.parent().is_some_and(|parent| {
        parent.file_name().is_some_and(|name| name == "agents")
            && parent
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == ".codex")
    });
    in_agents_dir
        && crate::skills::is_conforme_codex_agent_file(path)
        && std::fs::read_to_string(path)
            .ok()
            .and_then(|content| content.parse::<toml::Table>().ok())
            .and_then(|table| {
                table
                    .get("name")
                    .and_then(toml::Value::as_str)
                    .map(crate::config::sanitize_name)
            })
            .zip(path.file_stem())
            .is_some_and(|(name, stem)| stem == name.as_str())
}

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
            agents: true,
            mcp: true,
        }
    }

    fn reads_agents_md(&self, _project_root: &Path) -> bool {
        true
    }

    fn source_files(&self, project_root: &Path) -> Vec<PathBuf> {
        crate::adapters::first_existing_file(project_root, INSTRUCTION_FILES)
    }

    /// Skills live in the `.agents/skills/` root Codex shares with Zed;
    /// conforme only writes `<name>/SKILL.md` folders there. Agents are
    /// `.codex/agents/<name>.toml`; a file conforme could not have written
    /// (see [`is_conforme_written_agent`]), or one named like a built-in
    /// role, is the user's own.
    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![
            ManagedDir::subdirs(project_root.join(".agents").join("skills")),
            ManagedDir::files_except(
                project_root.join(".codex").join("agents"),
                ".toml",
                |path| !is_conforme_written_agent(path) || is_builtin_agent_override(path),
            ),
        ]
    }

    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".codex/config.toml"))
    }

    fn warnings(&self, _project_root: &Path, config: &NormalizedConfig) -> Vec<String> {
        let builtin_agents = config
            .agents
            .iter()
            .filter(|agent| crate::skills::is_codex_builtin_agent(&agent.name))
            .map(|agent| {
                format!(
                    "agent {} is not written: a file of that name would replace Codex's built-in agent",
                    agent.name
                )
            });
        config
            .mcp_servers
            .iter()
            .filter(|server| crate::mcp::codex_keeps_literal(server))
            .map(|server| {
                format!(
                    "MCP server {}: Codex expands no ${{VAR}} and forwards only `NAME=${{NAME}}`, \
                     `Authorization: Bearer ${{VAR}}` and a header that is exactly `${{VAR}}`; \
                     any other reference is sent as written. Use one of those forms, or put the \
                     real value in ~/.codex/config.toml under another server name",
                    server.name
                )
            })
            .chain(
                config
                    .mcp_servers
                    .iter()
                    .filter(|server| {
                        matches!(server.transport, crate::config::McpTransport::Sse { .. })
                    })
                    .map(|server| {
                        format!(
                            "MCP server {}: Codex has no SSE transport, so it is written as \
                             streamable HTTP and connects only if the server speaks that too",
                            server.name
                        )
                    }),
            )
            .chain(builtin_agents)
            .collect()
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
        // Read skills back from the shared `.agents/skills/` location so a Codex
        // project round-trips as a source. Codex searches it 6 levels deep.
        let skills =
            crate::skills::read_skills_recursive(&project_root.join(".agents").join("skills"), 6)?;
        let codex_config = project_root.join(".codex").join("config.toml");
        let mcp_servers = if codex_config.exists() {
            let content = std::fs::read_to_string(&codex_config)?;
            crate::mcp::parse_codex_mcp_toml(&content)?
        } else {
            Vec::new()
        };

        // Codex reads `AGENTS.md`; a personal `AGENTS.override.md` is
        // deliberately not used as the source of every other tool.
        crate::markdown::read_native_agents_md(
            project_root,
            INSTRUCTION_FILES,
            NormalizedConfig {
                instructions: String::new(),
                rules: Vec::new(),
                skills,
                agents: crate::skills::read_codex_agents(project_root)?,
                mcp_servers,
            },
        )
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
        files.extend(crate::skills::generate_codex_agents(
            project_root,
            &config.agents,
        )?);

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
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 2); // SKILL.md + conforme marker
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

    #[test]
    fn test_builtin_override_named_by_its_name_key_is_never_swept() {
        // Codex names a discovered agent by its `name` key, not its file name,
        // so `my-explorer.toml` with `name = "explorer"` overrides the built-in
        // `explorer` role: conforme never writes it, so it is no orphan.
        let adapter = make_adapter();
        let dir = tempfile::TempDir::new().unwrap();
        let agents = dir.path().join(".codex").join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(
            agents.join("my-explorer.toml"),
            "name = \"explorer\"\ndescription = \"Mine\"\ndeveloper_instructions = \"Explore.\"\n",
        )
        .unwrap();
        let config = NormalizedConfig {
            instructions: "Hi.".to_string(),
            ..Default::default()
        };
        let orphans = crate::adapters::find_orphans(
            &adapter.managed_directories(dir.path()),
            &adapter.generate(dir.path(), &config).unwrap(),
        )
        .unwrap();
        assert!(orphans.is_empty(), "{orphans:?}");
    }
}
