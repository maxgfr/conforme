use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir, WriteReport};
use crate::config::NormalizedConfig;

/// Mistral Vibe adapter.
///
/// Vibe reads `AGENTS.md` natively (from the working directory up to the
/// trusted root) and has no per-rule files. Skills live in
/// `.vibe/skills/<name>/SKILL.md` (then the shared `.agents/skills`),
/// subagents in `.vibe/agents/<name>.toml`, and MCP servers in the
/// `[[mcp_servers]]` array of the project `.vibe/config.toml`, a file that
/// also holds the user's own settings. Project files load only in a folder
/// the user trusted.
pub struct VibeAdapter;

/// Vibe reads `AGENTS.md` and no other instruction file.
const INSTRUCTION_FILES: &[&str] = &["AGENTS.md"];

/// Skill names Vibe's built-in skills occupy: a project skill of that name is
/// skipped.
const RESERVED_SKILLS: &[&str] = &["vibe", "skill-creator"];

/// Vibe's built-in agents (`BuiltinAgentName`): a custom file of that name
/// replaces the built-in one, so a synced subagent there would remove a mode
/// (and `accept-edits`, the default start agent, would stop Vibe starting).
const BUILTIN_AGENTS: &[&str] = &[
    "ask",
    "plan",
    "accept-edits",
    "smart-approve",
    "auto-approve",
    "explore",
    "lean",
];

fn is_builtin_agent(name: &str) -> bool {
    BUILTIN_AGENTS.contains(&crate::config::sanitize_name(name).as_str())
}

fn config_path(project_root: &Path) -> PathBuf {
    project_root.join(".vibe").join("config.toml")
}

/// The skills in `.vibe/skills`; the shared `.agents/skills` root is read
/// only when there are none (as for DeepSeek), so a Codex or Zed copy
/// there is not taken for the source's own.
fn own_skills(project_root: &Path) -> Result<Vec<crate::config::NormalizedSkill>> {
    crate::skills::read_skills_from_dir(&project_root.join(".vibe").join("skills"))
}

impl AiToolAdapter for VibeAdapter {
    fn name(&self) -> &str {
        "Mistral Vibe"
    }

    fn id(&self) -> &str {
        "vibe"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".vibe").is_dir()
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
        let mut files = crate::adapters::first_existing_file(project_root, INSTRUCTION_FILES);
        let shared = project_root.join(".agents").join("skills");
        if shared.is_dir() && own_skills(project_root).is_ok_and(|s| s.is_empty()) {
            files.push(shared);
        }
        files
    }

    /// `.vibe/config.toml` holds the user's models, providers, tools and
    /// theme; conforme only merges `[[mcp_servers]]` into it.
    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".vibe/config.toml"))
    }

    fn warnings(&self, _project_root: &Path, config: &NormalizedConfig) -> Vec<String> {
        let mut warnings = Vec::new();
        for skill in &config.skills {
            let name = crate::config::sanitize_name(&skill.name);
            if RESERVED_SKILLS.contains(&name.as_str()) {
                warnings.push(format!(
                    "skill {name} is skipped: Vibe reserves that name for a built-in skill"
                ));
            }
        }
        for agent in &config.agents {
            if is_builtin_agent(&agent.name) {
                warnings.push(format!(
                    "agent {} is not written: a file of that name would replace Vibe's built-in agent",
                    crate::config::sanitize_name(&agent.name)
                ));
            }
        }
        for server in &config.mcp_servers {
            // A stdio server gets only the variables its `env` sets, verbatim;
            // only a bearer token is read from a variable (`api_key_env`).
            let unresolved = match &server.transport {
                crate::config::McpTransport::Stdio { command, args } => std::iter::once(command)
                    .chain(args)
                    .chain(server.env.values())
                    .any(|s| s.contains("${")),
                crate::config::McpTransport::Http { url, headers } => {
                    url.contains("${")
                        || headers.iter().any(|(key, value)| {
                            value.contains("${")
                                && !(key.eq_ignore_ascii_case("authorization")
                                    && crate::mcp::bearer_env_var(value).is_some())
                        })
                }
            };
            if unresolved {
                warnings.push(format!(
                    "MCP server {}: Vibe resolves no ${{VAR}} there and passes it as written; \
                     declare the server with its real value under another name in \
                     ~/.vibe/config.toml (the project entry replaces one of the same name)",
                    server.name
                ));
            }
        }
        warnings
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![
            ManagedDir::subdirs(project_root.join(".vibe").join("skills")),
            // An agent file that is not a subagent is a mode the user defined,
            // and one named like a built-in agent overrides it on purpose.
            ManagedDir::files_except(project_root.join(".vibe").join("agents"), ".toml", |path| {
                !crate::skills::is_vibe_subagent_file(path)
                    || path
                        .file_stem()
                        .is_some_and(|stem| is_builtin_agent(&stem.to_string_lossy()))
            }),
        ]
    }

    fn write(&self, project_root: &Path, config: &NormalizedConfig) -> Result<WriteReport> {
        let mut report = WriteReport {
            files_written: Vec::new(),
            files_unchanged: Vec::new(),
        };
        for (path, content) in self.generate(project_root, config)? {
            if self.is_shared_file(&path) {
                crate::adapters::write_if_changed_atomic(&path, &content, &mut report)?;
            } else {
                crate::adapters::write_if_changed(&path, &content, &mut report)?;
            }
        }
        Ok(report)
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut skills = own_skills(project_root)?;
        if skills.is_empty() {
            skills =
                crate::skills::read_skills_from_dir(&project_root.join(".agents").join("skills"))?;
        }
        let path = config_path(project_root);
        let mcp_servers = if path.exists() {
            crate::mcp::parse_vibe_mcp_toml(&std::fs::read_to_string(&path)?)?
        } else {
            Vec::new()
        };
        crate::markdown::read_native_agents_md(
            project_root,
            INSTRUCTION_FILES,
            NormalizedConfig {
                skills,
                // A built-in override is not an agent another tool could load.
                agents: crate::skills::read_vibe_agents(project_root)?
                    .into_iter()
                    .filter(|a| !is_builtin_agent(&a.name))
                    .collect(),
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
        // Vibe reads AGENTS.md itself: no instruction file is written.
        let mut files = crate::skills::generate_vibe_skills(project_root, &config.skills)?;
        let agents: Vec<_> = config
            .agents
            .iter()
            .filter(|a| !is_builtin_agent(&a.name))
            .cloned()
            .collect();
        files.extend(crate::skills::generate_vibe_agents(project_root, &agents)?);
        // A source with no MCP server leaves the user's config untouched.
        if !config.mcp_servers.is_empty() {
            let path = config_path(project_root);
            let existing = if path.exists() {
                std::fs::read_to_string(&path)?
            } else {
                String::new()
            };
            files.push((
                path,
                crate::mcp::merge_vibe_mcp_toml(&existing, &config.mcp_servers)?,
            ));
        }
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{NormalizedAgent, NormalizedSkill};

    #[test]
    fn test_generate_empty_config() {
        let files = VibeAdapter
            .generate(Path::new("/tmp/test"), &NormalizedConfig::default())
            .unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_warnings_name_what_vibe_will_not_load() {
        use crate::config::{McpTransport, NormalizedMcpServer};
        use std::collections::BTreeMap;
        let config = NormalizedConfig {
            skills: vec![NormalizedSkill {
                name: "skill-creator".to_string(),
                ..Default::default()
            }],
            mcp_servers: vec![
                NormalizedMcpServer {
                    name: "files".to_string(),
                    transport: McpTransport::Stdio {
                        command: "npx".to_string(),
                        args: vec![],
                    },
                    env: BTreeMap::from([("TOKEN".to_string(), "${TOKEN}".to_string())]),
                },
                NormalizedMcpServer {
                    name: "remote".to_string(),
                    transport: McpTransport::Http {
                        url: "https://example.com/mcp".to_string(),
                        headers: BTreeMap::from([(
                            "Authorization".to_string(),
                            "Bearer ${API_TOKEN}".to_string(),
                        )]),
                    },
                    env: BTreeMap::new(),
                },
            ],
            ..Default::default()
        };
        let warnings = VibeAdapter.warnings(Path::new("/tmp/test"), &config);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].starts_with("skill skill-creator is skipped"));
        assert!(warnings[1].starts_with("MCP server files:"));
    }

    #[test]
    fn test_generate_skill_and_subagent() {
        let config = NormalizedConfig {
            skills: vec![NormalizedSkill {
                name: "Deploy App".to_string(),
                description: "Deploy".to_string(),
                content: "Run.".to_string(),
                manual_invocation: true,
                ..Default::default()
            }],
            agents: vec![NormalizedAgent {
                name: "reviewer".to_string(),
                description: "Review".to_string(),
                content: "Review the diff.".to_string(),
                model: Some("sonnet".to_string()),
                tools: vec!["Read".to_string(), "Bash".to_string(), "Edit".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = VibeAdapter
            .generate(Path::new("/tmp/test"), &config)
            .unwrap();

        let skill = files
            .iter()
            .find(|(p, _)| p.ends_with(".vibe/skills/deploy-app/SKILL.md"))
            .unwrap();
        assert!(skill.1.contains("name: deploy-app"), "{}", skill.1);
        assert!(
            skill.1.contains("disable-model-invocation: true"),
            "{}",
            skill.1
        );

        let agent = files
            .iter()
            .find(|(p, _)| p.ends_with(".vibe/agents/reviewer.toml"))
            .unwrap();
        let value: toml::Value = agent.1.parse().unwrap();
        assert_eq!(value["agent_type"].as_str(), Some("subagent"));
        assert_eq!(value["description"].as_str(), Some("Review"));
        assert_eq!(value["instructions"].as_str(), Some("Review the diff."));
        assert_eq!(
            value["enabled_tools"],
            toml::Value::Array(vec!["read_file".into(), "bash".into(), "edit".into()])
        );
        assert!(
            value.get("model").is_none(),
            "a Claude alias is no Vibe model"
        );
    }
}
