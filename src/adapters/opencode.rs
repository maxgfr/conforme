use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
use crate::config::NormalizedConfig;

/// OpenCode adapter.
/// OpenCode reads AGENTS.md as primary, falls back to CLAUDE.md, then CONTEXT.md.
/// It also scans skills from .opencode/skills/, .claude/skills/, .agents/skills/.
/// MCP servers and agent definitions live inside `opencode.json` under
/// the `mcp` and `agent` keys respectively; per-project markdown agents
/// additionally live at `.opencode/agents/<name>.md`.
pub struct OpenCodeAdapter;

/// OpenCode reads `AGENTS.md`, else `CLAUDE.md`, else the deprecated
/// `CONTEXT.md`.
const INSTRUCTION_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "CONTEXT.md"];

/// The project config files OpenCode merges, lowest precedence first.
const CONFIG_FILES: &[&str] = &[
    "opencode.json",
    "opencode.jsonc",
    ".opencode/opencode.json",
    ".opencode/opencode.jsonc",
];

/// The config file conforme merges into: the first that exists, else
/// `.opencode/opencode.json`. Kilo Code also loads the root `opencode.json`
/// and refuses a project file holding `{env:VAR}`, which it never sees in
/// `.opencode/`.
fn config_path(project_root: &Path) -> PathBuf {
    CONFIG_FILES
        .iter()
        .map(|name| project_root.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| project_root.join(".opencode").join("opencode.json"))
}

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

    /// `opencode.json` is the user's whole OpenCode configuration; conforme
    /// only merges the `mcp` and `agent` keys into it, so `remove`/`migrate`
    /// must never delete the file wholesale.
    fn reads_agents_md(&self, _project_root: &Path) -> bool {
        true
    }

    fn source_files(&self, project_root: &Path) -> Vec<PathBuf> {
        crate::adapters::first_existing_file(project_root, INSTRUCTION_FILES)
    }

    fn is_shared_file(&self, path: &Path) -> bool {
        path.file_name()
            .is_some_and(|name| name == "opencode.json" || name == "opencode.jsonc")
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        // The agents and skills directories are managed (skill folders are
        // never swept, only the stale bundled files of the ones conforme
        // writes). The top-level `.opencode/` also holds user-owned files
        // (`package.json` for plugins, commands, tools, …) and must never be
        // swept for orphans.
        vec![
            // `.opencode/agents/build.md` tunes OpenCode's own Build agent:
            // conforme never writes it, so it is the user's.
            ManagedDir::files_except(
                project_root.join(".opencode").join("agents"),
                ".md",
                |path| {
                    path.file_stem().is_some_and(|stem| {
                        crate::mcp::is_opencode_builtin_agent(&stem.to_string_lossy())
                    })
                },
            ),
            ManagedDir::subdirs(project_root.join(".opencode").join("skills")),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // Read skills back from `.opencode/skills/**/SKILL.md`, then the
        // singular `.opencode/skill/` OpenCode also accepts.
        let mut skills = Vec::new();
        for dir in ["skills", "skill"] {
            for skill in crate::skills::read_skills_recursive(
                &project_root.join(".opencode").join(dir),
                crate::skills::UNBOUNDED_SKILL_DEPTH,
            )? {
                if !skills
                    .iter()
                    .any(|s: &crate::config::NormalizedSkill| s.name == skill.name)
                {
                    skills.push(skill);
                }
            }
        }

        // Read agents back from `.opencode/agents/` and the singular
        // `.opencode/agent/`, then the `agent` key in `opencode.json`:
        // OpenCode loads all three, the markdown files winning on a clash.
        let mut agents = Vec::new();
        for dir in ["agents", "agent"] {
            for agent in crate::skills::read_agents_from_dir(
                &project_root.join(".opencode").join(dir),
                true,
            )? {
                // `.opencode/agents/build.md` overrides OpenCode's Build agent:
                // not an agent another tool could load.
                if !crate::mcp::is_opencode_builtin_agent(&agent.name)
                    && !agents
                        .iter()
                        .any(|a: &crate::config::NormalizedAgent| a.name == agent.name)
                {
                    agents.push(agent);
                }
            }
        }

        // MCP servers live inside `opencode.json` under the `mcp` key, in
        // OpenCode's own shape (`type: local/remote`, `command` array,
        // `environment`) — not the standard `mcpServers` layout.
        // OpenCode parses the file as JSONC, so comments are accepted here too.
        // Later files override earlier ones, server by server.
        let mut mcp_servers: Vec<crate::config::NormalizedMcpServer> = Vec::new();
        for name in CONFIG_FILES {
            let Some(root) = crate::json_settings::load(&project_root.join(name))? else {
                continue;
            };
            if let Some(mcp) = root.get("mcp") {
                for server in crate::mcp::canonicalize_env_refs(
                    crate::mcp::parse_opencode_mcp_object(mcp),
                    crate::mcp::EnvRefStyle::OpenCode,
                ) {
                    mcp_servers.retain(|s| s.name != server.name);
                    mcp_servers.push(server);
                }
            }
            if let Some(agent) = root.get("agent") {
                for agent in crate::mcp::parse_opencode_agent_object(agent) {
                    if !agents.iter().any(|a| a.name == agent.name) {
                        agents.push(agent);
                    }
                }
            }
        }

        crate::markdown::read_native_agents_md(
            project_root,
            INSTRUCTION_FILES,
            NormalizedConfig {
                instructions: String::new(),
                rules: Vec::new(),
                skills,
                agents,
                mcp_servers,
            },
        )
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        // OpenCode reads AGENTS.md natively — no need to re-generate it.
        let mut files = Vec::new();

        // An agent named like a built-in one (`build`, `plan`, …) would
        // override OpenCode's own agent and demote it to a subagent.
        let agents: Vec<_> = config
            .agents
            .iter()
            .filter(|a| !crate::mcp::is_opencode_builtin_agent(&a.name))
            .cloned()
            .collect();
        let config = &NormalizedConfig {
            agents,
            ..config.clone()
        };

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

        // Merge MCP + agent objects into the project's opencode.json (see
        // `config_path`). OpenCode reads MCP under the `mcp` key (not from a
        // standalone .opencode/mcp.json). We read any existing file to
        // preserve user-authored keys (and JSONC comments), then replace
        // only our managed keys.
        // A key the source has nothing for is left alone, like every other
        // MCP target: it may hold what the user keeps there by hand.
        let config_path = config_path(project_root);
        // With nothing to write there, a file conforme cannot parse is only
        // left alone, as before.
        let existing = match crate::json_settings::load(&config_path) {
            Err(_) if config.mcp_servers.is_empty() && config.agents.is_empty() => None,
            loaded => loaded?,
        };
        let stale = stale_agent_entries(
            project_root,
            existing.as_ref().and_then(|f| f.get("agent")),
            &config.agents,
        );
        if !config.mcp_servers.is_empty() || !config.agents.is_empty() || !stale.is_empty() {
            let mut set = Vec::new();
            if !config.mcp_servers.is_empty() {
                let mcp_obj = crate::json_settings::merge_server_entries(
                    existing.as_ref().and_then(|f| f.get("mcp")),
                    crate::mcp::build_opencode_mcp_object(&config.mcp_servers),
                    crate::mcp::OPENCODE_OWNED_SERVER_KEYS,
                );
                set.push(("mcp", serde_json::Value::Object(mcp_obj)));
            }
            if !config.agents.is_empty() || !stale.is_empty() {
                // `agent` also holds the user's own entries (overrides of the
                // built-in `build`/`plan` agents, per-agent `permission`, …),
                // which survive.
                let mut agent_obj = crate::mcp::merge_opencode_agents(
                    existing.as_ref().and_then(|f| f.get("agent")),
                    crate::mcp::build_opencode_agent_object(&config.agents),
                );
                for name in &stale {
                    agent_obj.remove(name);
                }
                set.push(("agent", serde_json::Value::Object(agent_obj)));
            }

            let json = crate::json_settings::render(
                existing.as_ref(),
                &set,
                &[(
                    "$schema",
                    serde_json::Value::String("https://opencode.ai/config.json".to_string()),
                )],
            )?;
            files.push((config_path, json));
        }

        Ok(files)
    }
}

/// The `agent` entries of opencode.json an earlier sync wrote for an agent
/// that has left the source. conforme writes every agent twice, as
/// `.opencode/agents/<name>.md` and as `agent.<name>`: an entry whose file is
/// still there (orphan cleanup removes it in the same sync) and that holds
/// only the keys conforme writes is conforme's. Left behind, it would keep the
/// agent loaded. An entry without that file, or one the user extended
/// (`permission`, `temperature`, …), is the user's.
fn stale_agent_entries(
    project_root: &Path,
    existing: Option<&serde_json::Value>,
    agents: &[crate::config::NormalizedAgent],
) -> Vec<String> {
    let Some(entries) = existing.and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    let generated: Vec<String> = agents
        .iter()
        .map(|a| crate::config::sanitize_name(&a.name))
        .collect();
    entries
        .iter()
        .filter(|(name, entry)| {
            !generated.contains(name)
                && !crate::mcp::is_opencode_builtin_agent(name)
                && entry.as_object().is_some_and(|fields| {
                    fields
                        .keys()
                        .all(|key| crate::mcp::OPENCODE_OWNED_AGENT_KEYS.contains(&key.as_str()))
                })
                && project_root
                    .join(".opencode")
                    .join("agents")
                    .join(format!("{name}.md"))
                    .is_file()
        })
        .map(|(name, _)| name.clone())
        .collect()
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
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 2); // SKILL.md + conforme marker
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
        // A new file goes to `.opencode/`, where Kilo Code never reads it.
        assert_eq!(files[0].0, tmp.path().join(".opencode/opencode.json"));
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
                ..Default::default()
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
