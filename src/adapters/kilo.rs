use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
use crate::config::{ActivationMode, NormalizedConfig, NormalizedRule};

/// Kilo Code adapter (the Kilo CLI and the VS Code extension built on it).
///
/// Kilo is an OpenCode fork: it reads `AGENTS.md` (else `CLAUDE.md`, else
/// `CONTEXT.md`) plus the plain Markdown files of `.kilo/rules/`, skills from
/// `.kilo/skills/<name>/SKILL.md`, subagents from `.kilo/agents/<name>.md`,
/// and MCP servers from the `mcp` key of `kilo.jsonc`, in OpenCode's shape.
/// The legacy `.kilocode/` locations are still read.
pub struct KiloAdapter;

/// Kilo reads the first of these that exists.
const INSTRUCTION_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "CONTEXT.md"];

/// The config files Kilo merges, lowest precedence first.
const CONFIG_FILES: &[&str] = &[
    "kilo.json",
    "kilo.jsonc",
    ".kilocode/kilo.json",
    ".kilocode/kilo.jsonc",
    ".kilo/kilo.json",
    ".kilo/kilo.jsonc",
];

/// Every project config file Kilo loads, lowest precedence first: `kilo`
/// then `opencode` at the root (config/config.ts), then each of
/// `ALL_CONFIG_FILES` in `.kilocode/` and `.kilo/`. conforme writes only the
/// `kilo` ones (`CONFIG_FILES`); the `opencode` ones are read too.
const LOADED_CONFIG_FILES: &[&str] = &[
    "kilo.json",
    "kilo.jsonc",
    "opencode.json",
    "opencode.jsonc",
    ".kilocode/kilo.jsonc",
    ".kilocode/kilo.json",
    ".kilocode/opencode.jsonc",
    ".kilocode/opencode.json",
    ".kilo/kilo.jsonc",
    ".kilo/kilo.json",
    ".kilo/opencode.jsonc",
    ".kilo/opencode.json",
];

/// The rule directories Kilo reads (top-level `*.md` files only).
const RULE_DIRS: &[&str] = &[".kilocode/rules", ".kilo/rules"];

/// The config file conforme merges into: the highest-precedence one that
/// exists, else `.kilo/kilo.jsonc`, where Kilo saves its own settings.
fn config_path(project_root: &Path) -> PathBuf {
    CONFIG_FILES
        .iter()
        .rev()
        .map(|name| project_root.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| project_root.join(".kilo").join("kilo.jsonc"))
}

fn rule_files(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for dir in RULE_DIRS {
        let dir = project_root.join(dir);
        if !dir.is_dir() {
            continue;
        }
        let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
            .with_context(|| format!("failed to read {}", dir.display()))?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.is_file() && path.extension().is_some_and(|e| e == "md"))
            .collect();
        found.sort();
        files.extend(found);
    }
    Ok(files)
}

/// An agent file Kilo loads as a built-in agent's override, not an agent.
fn is_builtin_agent_file(path: &Path) -> bool {
    path.file_stem()
        .is_some_and(|stem| crate::mcp::is_kilo_builtin_agent(&stem.to_string_lossy()))
}

impl AiToolAdapter for KiloAdapter {
    fn name(&self) -> &str {
        "Kilo Code"
    }

    fn id(&self) -> &str {
        "kilo"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".kilo").is_dir()
            || project_root.join(".kilocode").is_dir()
            || project_root.join("kilo.json").is_file()
            || project_root.join("kilo.jsonc").is_file()
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
        files.extend(
            RULE_DIRS
                .iter()
                .map(|dir| project_root.join(dir))
                .filter(|dir| dir.is_dir()),
        );
        files
    }

    /// `kilo.jsonc` holds the user's whole Kilo configuration; conforme only
    /// merges the `mcp` key into it.
    fn is_shared_file(&self, path: &Path) -> bool {
        path.file_name()
            .is_some_and(|name| name == "kilo.json" || name == "kilo.jsonc")
    }

    fn warnings(&self, project_root: &Path, config: &NormalizedConfig) -> Vec<String> {
        let mut warnings = Vec::new();
        // Kilo also loads the root `opencode.json`, and refuses the whole
        // file when it holds the `{env:VAR}` references OpenCode resolves.
        for name in ["opencode.json", "opencode.jsonc"] {
            let path = project_root.join(name);
            if std::fs::read_to_string(&path).is_ok_and(|text| text.contains("{env:")) {
                warnings.push(format!(
                    "{name} holds {{env:VAR}} references, and Kilo refuses that file in a project; \
                     move OpenCode's settings to .opencode/opencode.json, which OpenCode reads and Kilo does not"
                ));
            }
        }
        for server in &config.mcp_servers {
            let mut strings: Vec<&String> = match &server.transport {
                crate::config::McpTransport::Stdio { command, args } => {
                    std::iter::once(command).chain(args).collect()
                }
                crate::config::McpTransport::Http { url, headers }
                | crate::config::McpTransport::Sse { url, headers } => {
                    std::iter::once(url).chain(headers.values()).collect()
                }
            };
            strings.extend(
                server
                    .env
                    .iter()
                    .filter(|(key, value)| **value != format!("${{{key}}}"))
                    .map(|(_, value)| value),
            );
            if strings.iter().any(|s| s.contains("${")) {
                warnings.push(format!(
                    "MCP server {}: Kilo resolves no variable in a project config, so ${{VAR}} is \
                     sent as written; declare the server under another name in \
                     ~/.config/kilo/kilo.jsonc, where {{env:VAR}} works (the project entry \
                     overrides one of the same name)",
                    server.name
                ));
            }
        }
        warnings
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![
            ManagedDir::subdirs(project_root.join(".kilo").join("skills")),
            // `.kilo/agents/code.md` tunes Kilo's own Code agent: the user's.
            ManagedDir::files_except(
                project_root.join(".kilo").join("agents"),
                ".md",
                is_builtin_agent_file,
            ),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut skills: Vec<crate::config::NormalizedSkill> = Vec::new();
        for base in [".kilo", ".kilocode"] {
            for dir in ["skills", "skill"] {
                for skill in crate::skills::read_skills_recursive(
                    &project_root.join(base).join(dir),
                    crate::skills::UNBOUNDED_SKILL_DEPTH,
                )? {
                    if !skills.iter().any(|s| s.name == skill.name) {
                        skills.push(skill);
                    }
                }
            }
        }

        let mut agents: Vec<crate::config::NormalizedAgent> = Vec::new();
        for base in [".kilo", ".kilocode"] {
            for dir in ["agents", "agent"] {
                for agent in
                    crate::skills::read_agents_from_dir(&project_root.join(base).join(dir), true)?
                {
                    if !crate::mcp::is_kilo_builtin_agent(&agent.name)
                        && !agents.iter().any(|a| a.name == agent.name)
                    {
                        agents.push(agent);
                    }
                }
            }
        }

        // Later config files override earlier ones, server by server.
        let mut mcp_servers: Vec<crate::config::NormalizedMcpServer> = Vec::new();
        for name in LOADED_CONFIG_FILES {
            let Some(root) = crate::json_settings::load(&project_root.join(name))? else {
                continue;
            };
            if let Some(mcp) = root.get("mcp") {
                for server in crate::mcp::parse_opencode_mcp_object(mcp) {
                    mcp_servers.retain(|s| s.name != server.name);
                    mcp_servers.push(server);
                }
            }
            if let Some(agent) = root.get("agent") {
                for agent in crate::mcp::parse_opencode_agent_object(agent) {
                    if !crate::mcp::is_kilo_builtin_agent(&agent.name)
                        && !agents.iter().any(|a| a.name == agent.name)
                    {
                        agents.push(agent);
                    }
                }
            }
        }

        let mut config = crate::markdown::read_native_agents_md(
            project_root,
            INSTRUCTION_FILES,
            NormalizedConfig {
                skills,
                agents,
                mcp_servers,
                ..Default::default()
            },
        )?;
        // Kilo loads every rule file in full, alongside the instructions.
        for path in rule_files(project_root)? {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            config.rules.retain(|r| r.name != name);
            config.rules.push(NormalizedRule {
                name,
                content: content.trim().to_string(),
                activation: ActivationMode::Always,
            });
        }
        Ok(config)
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        // Kilo reads AGENTS.md itself: no instruction file is written.
        let mut files = crate::skills::generate_kilo_skills(project_root, &config.skills)?;

        // An agent named like a built-in one (`code`, `ask`, …) would
        // override Kilo's own agent.
        let agents: Vec<_> = config
            .agents
            .iter()
            .filter(|a| !crate::mcp::is_kilo_builtin_agent(&a.name))
            .cloned()
            .collect();
        files.extend(crate::skills::generate_kilo_agents_md(
            project_root,
            &agents,
        )?);

        files.extend(crate::json_settings::server_settings_file(
            &config_path(project_root),
            "mcp",
            crate::mcp::build_kilo_mcp_object(&config.mcp_servers),
            crate::mcp::OPENCODE_OWNED_SERVER_KEYS,
            &[(
                "$schema",
                serde_json::Value::String("https://app.kilo.ai/config.json".to_string()),
            )],
        )?);
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpTransport, NormalizedAgent, NormalizedMcpServer};
    use std::collections::BTreeMap;

    #[test]
    fn test_generate_empty_config() {
        let files = KiloAdapter
            .generate(Path::new("/tmp/test"), &NormalizedConfig::default())
            .unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_mcp_has_no_env_references() {
        let tmp = tempfile::tempdir().unwrap();
        let config = NormalizedConfig {
            mcp_servers: vec![NormalizedMcpServer {
                name: "files".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "server".to_string()],
                },
                env: BTreeMap::from([
                    ("TOKEN".to_string(), "${TOKEN}".to_string()),
                    ("MODE".to_string(), "ci".to_string()),
                ]),
            }],
            ..Default::default()
        };
        let files = KiloAdapter.generate(tmp.path(), &config).unwrap();
        let (path, json) = &files[0];
        assert_eq!(*path, tmp.path().join(".kilo").join("kilo.jsonc"));
        assert!(!json.contains("{env:"), "{json}");
        assert!(!json.contains("TOKEN"), "an inherited variable: {json}");
        assert!(json.contains("\"MODE\": \"ci\""), "{json}");
        assert!(json.contains("\"type\": \"local\""), "{json}");
    }

    #[test]
    fn test_builtin_agent_names_are_not_written() {
        let config = NormalizedConfig {
            agents: vec![
                NormalizedAgent {
                    name: "code".to_string(),
                    description: "Mine".to_string(),
                    content: "Code.".to_string(),
                    ..Default::default()
                },
                NormalizedAgent {
                    name: "reviewer".to_string(),
                    description: "Review".to_string(),
                    content: "Review.".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let files = KiloAdapter
            .generate(Path::new("/tmp/test"), &config)
            .unwrap();
        let paths: Vec<_> = files.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(
            paths,
            vec![PathBuf::from("/tmp/test/.kilo/agents/reviewer.md")]
        );
        assert!(files[0].1.contains("mode: subagent"));
    }
}
