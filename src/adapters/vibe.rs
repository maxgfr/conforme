use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
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

/// Vibe's built-in agents (`BuiltinAgentName`): a custom file of that name
/// replaces the built-in one, so a synced subagent there would remove a mode
/// (and `accept-edits`, the default start agent, would stop Vibe starting).
/// Vibe 3 has no built-in `plan` agent any more ("Vibe has no built-in `plan`
/// agent", https://docs.mistral.ai/vibe/code/cli/agents).
const BUILTIN_AGENTS: &[&str] = &[
    "ask",
    "accept-edits",
    "smart-approve",
    "auto-approve",
    "explore",
    "lean",
];

/// An agent file Vibe refuses to load: "The name `main` is reserved, so don't
/// name an agent file `main.toml`" (https://docs.mistral.ai/vibe/code/cli/agents;
/// `RESERVED_AGENT_NAME`, matched case-insensitively, since Vibe 3.0.3).
const RESERVED_AGENT: &str = "main";

/// Whether conforme leaves an agent of that name out of `.vibe/agents/`: a
/// built-in agent's name or the reserved `main`.
fn is_builtin_agent(name: &str) -> bool {
    let name = crate::config::sanitize_name(name);
    name == RESERVED_AGENT || BUILTIN_AGENTS.contains(&name.as_str())
}

/// The `enabled_plugins` and `disabled_plugins` lists of a Vibe
/// `config.toml` (Vibe's `vibe/core/plugins/_enablement.py`:
/// https://github.com/mistralai/mistral-vibe/blob/main/vibe/core/plugins/_enablement.py).
fn plugin_switches(config_text: &str) -> (Vec<String>, Vec<String>) {
    let table = config_text.parse::<toml::Table>().unwrap_or_default();
    let list = |key: &str| -> Vec<String> {
        table
            .get(key)
            .and_then(toml::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    (list("enabled_plugins"), list("disabled_plugins"))
}

/// `config_text` with `disabled_plugins` naming exactly the disabled ones of
/// `plugins`: a plugin conforme writes is added or removed there, any other
/// name the list holds is the user's and stays. An empty list is removed.
fn merge_disabled_plugins(
    config_text: &str,
    plugins: &[crate::config::NormalizedPlugin],
) -> Result<String> {
    let mut document: toml_edit::DocumentMut = config_text.parse().map_err(|e| {
        anyhow::anyhow!("failed to parse .vibe/config.toml, nothing was changed: {e}")
    })?;
    let ours: Vec<String> = plugins
        .iter()
        .map(|p| crate::config::sanitize_name(&p.name))
        .collect();
    let mut names: Vec<String> = document
        .get("disabled_plugins")
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .filter(|name| !ours.contains(name))
                .collect()
        })
        .unwrap_or_default();
    names.extend(
        plugins
            .iter()
            .filter(|p| p.disabled)
            .map(|p| crate::config::sanitize_name(&p.name)),
    );
    let unchanged = document
        .get("disabled_plugins")
        .and_then(|item| item.as_array())
        .is_some_and(|items| {
            items.len() == names.len()
                && items.iter().zip(&names).all(|(a, b)| a.as_str() == Some(b))
        });
    if unchanged {
        return Ok(config_text.to_string());
    }
    if names.is_empty() {
        document.remove("disabled_plugins");
    } else {
        let mut array = toml_edit::Array::new();
        for name in names {
            array.push(name);
        }
        document["disabled_plugins"] = toml_edit::value(array);
    }
    Ok(document.to_string())
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
        path.ends_with(Path::new(".vibe/hooks.toml"))
            || path.ends_with(Path::new(".vibe/config.toml"))
    }

    fn tool_warnings(&self, project_root: &Path, config: &NormalizedConfig) -> Vec<String> {
        let mut warnings =
            crate::plugins::bundle_warnings(project_root, config, &crate::plugins::VIBE_PLUGINS);
        if !config.hooks.is_empty() {
            if let Ok((_, lost)) = crate::hooks::merge_vibe_project_toml("", &config.hooks) {
                warnings.extend(lost);
            }
            warnings.push(crate::hooks::payload_warning(
                "project",
                "Vibe",
                "a hook blocks only by printing a `deny` decision (a non-zero exit is a warning \
                 unless `strict`)",
            ));
        }
        for plugin in &config.plugins {
            if plugin.disabled {
                warnings.push(format!(
                    "plugin {} is switched off by `disabled_plugins` in .vibe/config.toml, which \
                     Vibe reads since 3.1.2: an older Vibe loads it",
                    plugin.name
                ));
            }
            for server in &plugin.mcp_servers {
                if matches!(server.transport, crate::config::McpTransport::Sse { .. }) {
                    warnings.push(format!(
                        "plugin {}: MCP server {}: Vibe plugins have no SSE transport, so it is \
                         written as streamable HTTP and connects only if the server speaks that too",
                        plugin.name, server.name
                    ));
                }
                let reaches_out = crate::mcp::server_strings(server)
                    .iter()
                    .any(|(_, v)| v.contains("${") && !crate::plugins::uses_plugin_root(v));
                if reaches_out {
                    warnings.push(format!(
                        "plugin {}: MCP server {}: a Vibe plugin expands only ${{PLUGIN_ROOT}} \
                         and ${{PLUGIN_DATA}}, so any other ${{VAR}} is passed as written",
                        plugin.name, server.name
                    ));
                }
            }
        }
        for agent in &config.agents {
            let name = crate::config::sanitize_name(&agent.name);
            if name == RESERVED_AGENT {
                warnings.push(format!(
                    "agent {name} is not written: Vibe reserves that name and refuses an agent file `{name}.toml`"
                ));
            } else if is_builtin_agent(&name) {
                warnings.push(format!(
                    "agent {name} is not written: a file of that name would replace Vibe's built-in agent"
                ));
            }
        }
        for server in &config.mcp_servers {
            // A stdio server gets only the variables its `env` sets, verbatim;
            // only one header is read from a variable (`api_key_env`).
            let api_key = match &server.transport {
                crate::config::McpTransport::Stdio { .. } => None,
                crate::config::McpTransport::Http { headers, .. }
                | crate::config::McpTransport::Sse { headers, .. } => {
                    crate::mcp::vibe_api_key(headers)
                }
            };
            let unresolved = crate::mcp::server_strings(server)
                .iter()
                .any(|(key, value)| {
                    value.contains("${")
                        && !(key.is_some()
                            && api_key.as_ref().is_some_and(|k| Some(k.header) == *key))
                });
            if unresolved {
                warnings.push(format!(
                    "MCP server {}: Vibe resolves no ${{VAR}} there and passes it as written; \
                     declare the server with its real value under another name in \
                     ~/.vibe/config.toml (the project entry replaces one of the same name)",
                    server.name
                ));
            }
            if matches!(server.transport, crate::config::McpTransport::Sse { .. }) {
                warnings.push(format!(
                    "MCP server {}: Mistral Vibe has no SSE transport, so it is written as \
                     streamable HTTP and connects only if the server speaks that too",
                    server.name
                ));
            }
        }
        warnings
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![
            ManagedDir::subdirs(project_root.join(".vibe").join("skills")),
            ManagedDir::bundles(project_root.join(".vibe").join("plugins")),
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

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut skills = own_skills(project_root)?;
        if skills.is_empty() {
            skills =
                crate::skills::read_skills_from_dir(&project_root.join(".agents").join("skills"))?;
        }
        let path = config_path(project_root);
        let config_text = if path.exists() {
            crate::config::read_text(&path)?
        } else {
            String::new()
        };
        let mcp_servers = if path.exists() {
            crate::mcp::parse_vibe_mcp_toml(&config_text)?
        } else {
            Vec::new()
        };
        let (enabled, disabled) = plugin_switches(&config_text);
        let plugins = crate::plugins::read_plugins(
            project_root,
            &crate::plugins::VIBE_PLUGINS,
            // "a disable beats an enable".
            &|name| {
                if disabled.iter().any(|n| n == name) {
                    Some(false)
                } else if enabled.iter().any(|n| n == name) {
                    Some(true)
                } else {
                    None
                }
            },
        )?;
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
                plugins,
                hooks: crate::config::read_text(&hooks_path(project_root))
                    .ok()
                    .and_then(|text| crate::hooks::read_vibe_project(&text).ok())
                    .unwrap_or_default(),
                ..Default::default()
            },
        )
    }

    fn holds_hooks(&self) -> Result<(), &'static str> {
        Ok(())
    }

    fn plugin_shape(&self) -> Option<&'static crate::plugins::PluginShape> {
        Some(&crate::plugins::VIBE_PLUGINS)
    }

    fn generate_flat(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        // Vibe reads AGENTS.md itself: no instruction file is written.
        let mut files = crate::skills::generate_skills(
            project_root,
            &config.skills,
            &crate::skills::VIBE_SKILLS,
        )?;
        let agents: Vec<_> = config
            .agents
            .iter()
            .filter(|a| !is_builtin_agent(&a.name))
            .cloned()
            .collect();
        files.extend(crate::skills::generate_vibe_agents(project_root, &agents)?);
        files.extend(crate::plugins::generate_plugins(
            project_root,
            &config.plugins,
            &crate::plugins::VIBE_PLUGINS,
        )?);
        // A source with no MCP server and no plugin leaves the user's config
        // untouched.
        if !config.mcp_servers.is_empty() || !config.plugins.is_empty() {
            let path = config_path(project_root);
            let mut text = if path.exists() {
                crate::config::read_text(&path)?
            } else {
                String::new()
            };
            if !config.mcp_servers.is_empty() {
                text = crate::mcp::merge_vibe_mcp_toml(&text, &config.mcp_servers)?;
            }
            if !config.plugins.is_empty() {
                text = merge_disabled_plugins(&text, &config.plugins)?;
            }
            if path.exists() || !text.trim().is_empty() {
                files.push((path, text));
            }
        }
        // Project hooks (`hooks = true`) in .vibe/hooks.toml, named
        // `conforme-…`; the user's own hooks there stay.
        if !config.hooks.is_empty() {
            let path = hooks_path(project_root);
            let existing = if path.exists() {
                crate::config::read_text(&path)?
            } else {
                String::new()
            };
            let (text, _) = crate::hooks::merge_vibe_project_toml(&existing, &config.hooks)?;
            if path.exists() || !text.trim().is_empty() {
                files.push((path, text));
            }
        }
        Ok(files)
    }
}

/// Vibe's project hooks file (https://docs.mistral.ai/vibe/code/cli/hooks),
/// loaded in a trusted folder.
fn hooks_path(project_root: &Path) -> PathBuf {
    project_root.join(".vibe").join("hooks.toml")
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
            // Since Vibe 3.0.4 a project skill of a built-in skill's name wins
            // ("a local skill wins"): nothing to warn about.
            skills: vec![NormalizedSkill {
                name: "skill-creator".to_string(),
                ..Default::default()
            }],
            agents: vec![
                NormalizedAgent {
                    name: "main".to_string(),
                    ..Default::default()
                },
                // Vibe 3 has no built-in `plan` agent: one is written.
                NormalizedAgent {
                    name: "plan".to_string(),
                    ..Default::default()
                },
            ],
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
        assert!(warnings[0].starts_with("agent main is not written: Vibe reserves"));
        assert!(warnings[1].starts_with("MCP server files:"));
        let files = VibeAdapter
            .generate(Path::new("/tmp/test"), &config)
            .unwrap();
        let agents: Vec<_> = files
            .iter()
            .filter(|(path, _)| path.starts_with("/tmp/test/.vibe/agents"))
            .map(|(path, _)| path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(agents, vec!["plan.toml"]);
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
