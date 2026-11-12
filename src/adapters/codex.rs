use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
use crate::config::NormalizedConfig;

/// OpenAI Codex CLI adapter.
/// Codex reads AGENTS.md natively as its primary instruction file.
/// It also supports AGENTS.override.md and project-scoped `.codex/config.toml`
/// settings, including MCP servers.
pub struct CodexAdapter;

/// Codex reads the shared `AGENTS.md` (never the personal override).
const INSTRUCTION_FILES: &[&str] = &["AGENTS.md"];

/// The repository marketplace Codex discovers at the repository root
/// ("`$REPO_ROOT/.agents/plugins/marketplace.json`",
/// https://developers.openai.com/plugins/build/plugins; `MARKETPLACE_MANIFEST_RELATIVE_PATHS`
/// in `codex-rs/core-plugins/src/marketplace.rs`).
const MARKETPLACE: &str = ".agents/plugins/marketplace.json";

/// The name a marketplace conforme creates gets; an existing one keeps its own.
const DEFAULT_MARKETPLACE: &str = "conforme";

fn marketplace_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".agents")
        .join("plugins")
        .join("marketplace.json")
}

fn config_path(project_root: &Path) -> PathBuf {
    project_root.join(".codex").join("config.toml")
}

/// The plugin a marketplace entry installs from `.codex/plugins/<name>`
/// (`{"source": "local", "path": "./.codex/plugins/<name>"}`, or the path as a
/// plain string), if that is where it points.
fn bundle_entry_name(entry: &serde_json::Value) -> Option<String> {
    let path = match entry.get("source")? {
        serde_json::Value::String(path) => path.as_str(),
        serde_json::Value::Object(source)
            if source.get("source").and_then(serde_json::Value::as_str) == Some("local") =>
        {
            source.get("path")?.as_str()?
        }
        _ => return None,
    };
    let name = path.strip_prefix("./")?.strip_prefix(".codex/plugins/")?;
    let name = name.trim_end_matches('/');
    (!name.is_empty() && !name.contains('/')).then(|| name.to_string())
}

/// Whether the bundle `.codex/plugins/<name>` is conforme's to list and
/// enable: one it wrote (marked), or one that is gone. A bundle written by
/// hand, and its entries, are the user's.
fn owns_bundle(project_root: &Path, name: &str) -> bool {
    !crate::plugins::is_hand_written_bundle(
        &crate::plugins::CODEX_PLUGINS.root(project_root).join(name),
    )
}

/// The repository marketplace's name: its own, else the one conforme gives a
/// new one.
fn marketplace_name(project_root: &Path) -> String {
    crate::config::read_text(&marketplace_path(project_root))
        .ok()
        .and_then(|text| crate::json_settings::parse_jsonc(&text).ok())
        .and_then(|doc| {
            doc.get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| DEFAULT_MARKETPLACE.to_string())
}

/// The repository marketplace after a sync.
struct MergedMarketplace {
    /// The file to write; `None` when it is left untouched (no plugin, and no
    /// entry of conforme's to remove).
    file: Option<(PathBuf, String)>,
    /// The plugins it lists from conforme's bundles, by folder name.
    listed: Vec<String>,
    /// The plugins whose entries were conforme's before this sync: their
    /// `[plugins]` tables are conforme's too.
    owned: Vec<String>,
}

/// The repository marketplace listing conforme's bundles of `plugins` next to
/// the user's own entries (see [`MergedMarketplace`]). An entry the user
/// already gave a plugin's name wins over conforme's.
fn merged_marketplace(
    project_root: &Path,
    plugins: &[crate::config::NormalizedPlugin],
) -> Result<MergedMarketplace> {
    use serde_json::{json, Value};
    let path = marketplace_path(project_root);
    let existing = if path.is_file() {
        Some(crate::config::read_text(&path)?)
    } else {
        None
    };
    let mut doc = match &existing {
        Some(text) => crate::json_settings::parse_jsonc(text).map_err(|e| {
            anyhow::anyhow!(
                "{} does not parse, nothing was changed: {e}",
                path.display()
            )
        })?,
        None => json!({}),
    };
    let Some(object) = doc.as_object_mut() else {
        anyhow::bail!(
            "{} is not a JSON object, nothing was changed",
            path.display()
        );
    };
    let entries = object
        .get("plugins")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let ours =
        |entry: &Value| bundle_entry_name(entry).is_some_and(|n| owns_bundle(project_root, &n));
    // By folder name, as Codex reads the bundles back: the same plugins list
    // the same way whatever the source's order.
    let mut wanted: Vec<&crate::config::NormalizedPlugin> = plugins
        .iter()
        .filter(|p| owns_bundle(project_root, &crate::config::sanitize_name(&p.name)))
        .collect();
    wanted.sort_by_key(|p| crate::config::sanitize_name(&p.name));
    let owned: Vec<String> = entries
        .iter()
        .filter(|e| ours(e))
        .filter_map(bundle_entry_name)
        .collect();
    if wanted.is_empty() && owned.is_empty() {
        return Ok(MergedMarketplace {
            file: None,
            listed: Vec::new(),
            owned,
        });
    }
    let mut kept: Vec<Value> = entries.into_iter().filter(|e| !ours(e)).collect();
    let mut listed = Vec::new();
    for plugin in wanted {
        let name = crate::config::sanitize_name(&plugin.name);
        let taken = kept
            .iter()
            .any(|e| e.get("name").and_then(Value::as_str) == Some(name.as_str()));
        if taken {
            continue;
        }
        kept.push(json!({
            "name": name,
            "source": { "source": "local", "path": format!("./.codex/plugins/{name}") },
            "policy": { "installation": "AVAILABLE", "authentication": "ON_INSTALL" },
            "category": "Productivity",
        }));
        listed.push(name);
    }
    object
        .entry("name")
        .or_insert_with(|| Value::String(DEFAULT_MARKETPLACE.to_string()));
    object
        .entry("interface")
        .or_insert_with(|| json!({ "displayName": "Project plugins" }));
    object.insert("plugins".to_string(), Value::Array(kept));
    // An unchanged marketplace keeps the user's own layout.
    let unchanged = existing
        .as_deref()
        .and_then(|text| crate::json_settings::parse_jsonc(text).ok())
        .is_some_and(|old| old == doc);
    let content = match existing {
        Some(text) if unchanged => text,
        _ => format!("{}\n", serde_json::to_string_pretty(&doc)?),
    };
    Ok(MergedMarketplace {
        file: Some((path, content)),
        listed,
        owned,
    })
}

/// `config_text` with a `[plugins."<name>@<marketplace>"]` table enabling or
/// disabling each plugin of `listed`, and without the tables of the plugins
/// conforme listed before (`owned`) that are no longer listed. Other keys of
/// those tables, and every other table, are kept.
fn merge_codex_plugins_toml(
    config_text: &str,
    marketplace: &str,
    listed: &[(String, bool)],
    owned: &[String],
) -> Result<String> {
    let mut document: toml_edit::DocumentMut = config_text.parse().map_err(|e| {
        anyhow::anyhow!("failed to parse .codex/config.toml, nothing was changed: {e}")
    })?;
    let suffix = format!("@{marketplace}");
    let stale: Vec<String> = document
        .get("plugins")
        .and_then(|item| item.as_table_like())
        .map(|table| {
            table
                .iter()
                .map(|(key, _)| key.to_string())
                .filter(|key| {
                    key.strip_suffix(&suffix).is_some_and(|name| {
                        owned.iter().any(|o| o == name) && !listed.iter().any(|(n, _)| n == name)
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if stale.is_empty() && listed.is_empty() {
        return Ok(config_text.to_string());
    }
    if document.get("plugins").is_none() {
        let mut table = toml_edit::Table::new();
        table.set_implicit(true);
        document.insert("plugins", toml_edit::Item::Table(table));
    }
    let Some(table) = document
        .get_mut("plugins")
        .and_then(|item| item.as_table_like_mut())
    else {
        anyhow::bail!(".codex/config.toml: `plugins` is not a table, nothing was changed");
    };
    for key in &stale {
        table.remove(key);
    }
    for (name, enabled) in listed {
        let key = format!("{name}{suffix}");
        if table.get(&key).is_none() {
            table.insert(&key, toml_edit::Item::Table(toml_edit::Table::new()));
        }
        if let Some(entry) = table.get_mut(&key).and_then(|i| i.as_table_like_mut()) {
            if entry.get("enabled").and_then(|v| v.as_bool()) != Some(*enabled) {
                entry.insert("enabled", toml_edit::value(*enabled));
            }
        }
    }
    let empty = table.is_empty();
    if empty {
        document.remove("plugins");
    }
    Ok(document.to_string())
}

/// The project's `[plugins."<name>@<marketplace>"] enabled` values.
fn plugin_switches(
    project_root: &Path,
    marketplace: &str,
) -> std::collections::BTreeMap<String, bool> {
    let suffix = format!("@{marketplace}");
    crate::config::read_text(&config_path(project_root))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|table| {
            table
                .get("plugins")
                .and_then(toml::Value::as_table)
                .cloned()
        })
        .map(|plugins| {
            plugins
                .into_iter()
                .filter_map(|(key, value)| {
                    let name = key.strip_suffix(&suffix)?.to_string();
                    let on = value.get("enabled").and_then(toml::Value::as_bool)?;
                    Some((name, on))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Whether a `.codex/agents/*.toml` file overrides a built-in role. Codex
/// names a discovered agent by its `name` key, not its file name, so either
/// one naming a built-in role makes the file the user's override.
fn is_builtin_agent_override(path: &Path) -> bool {
    let stem_is_builtin = path
        .file_stem()
        .is_some_and(|stem| crate::skills::is_codex_builtin_agent(&stem.to_string_lossy()));
    stem_is_builtin
        || crate::config::read_text(path)
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
        && crate::config::read_text(path)
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

/// The directories Codex looks up in its `projects` trust table for this
/// project: the directory itself (as given and canonical) and the root of
/// its main git repository, a worktree resolving to the main checkout
/// (`codex-rs/config/src/loader/mod.rs` `decision_for_dir`,
/// `git-utils/src/trust.rs`).
fn codex_trust_keys(project_root: &Path) -> Vec<PathBuf> {
    let canonical = canonical_path(project_root);
    let mut keys = vec![project_root.to_path_buf(), canonical.clone()];
    if let Some(repo) = canonical.ancestors().find(|dir| dir.join(".git").exists()) {
        let git = repo.join(".git");
        let main = std::fs::read_to_string(&git)
            .ok()
            .and_then(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("gitdir:"))
                    .map(|dir| repo.join(dir.trim()))
            })
            .and_then(|gitdir| {
                let gitdir = canonical_path(&gitdir);
                let worktrees = gitdir.parent()?;
                (worktrees.file_name()? == "worktrees")
                    .then(|| worktrees.parent()?.parent().map(Path::to_path_buf))
                    .flatten()
            });
        keys.push(main.unwrap_or_else(|| repo.to_path_buf()));
    }
    keys
}

/// The canonical form of a path as Codex writes it in its trust table:
/// without the `\\?\` verbatim prefix `canonicalize` adds on Windows.
fn canonical_path(path: &Path) -> PathBuf {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    strip_verbatim_prefix(&canonical.to_string_lossy())
        .map(PathBuf::from)
        .unwrap_or(canonical)
}

/// `C:\dir` for `\\?\C:\dir`; `None` for any other path (a verbatim UNC path
/// keeps its prefix).
fn strip_verbatim_prefix(path: &str) -> Option<&str> {
    path.strip_prefix(r"\\?\")
        .filter(|rest| !rest.starts_with(r"UNC\"))
}

/// The `[projects."<dir>"]` table header that trusts the project, its key
/// escaped as a TOML string so a Windows path pastes as written.
fn codex_trust_table(project_root: &Path) -> String {
    let dir = codex_trust_keys(project_root)
        .pop()
        .unwrap_or_else(|| project_root.to_path_buf());
    format!(
        "[projects.{}]",
        toml::Value::String(dir.display().to_string())
    )
}

/// Whether Codex trusts the project: `[projects."<dir>"] trust_level =
/// "trusted"` in `$CODEX_HOME/config.toml` (default `~/.codex`) for one of
/// [`codex_trust_keys`]. Without a home directory to look in, assume it does.
fn codex_trusts(project_root: &Path) -> bool {
    let codex_home = std::env::var_os("CODEX_HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".codex"))
        });
    let Some(codex_home) = codex_home else {
        return true;
    };
    let projects = crate::config::read_text(&codex_home.join("config.toml"))
        .ok()
        .and_then(|content| content.parse::<toml::Table>().ok())
        .and_then(|mut table| match table.remove("projects") {
            Some(toml::Value::Table(projects)) => Some(projects),
            _ => None,
        })
        .unwrap_or_default();
    // Codex compares keys case-insensitively on Windows.
    let same = |a: &str, b: &str| {
        if cfg!(windows) {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };
    codex_trust_keys(project_root).iter().any(|dir| {
        let dir = dir.to_string_lossy();
        projects.iter().any(|(key, entry)| {
            same(key, &dir)
                && entry.get("trust_level").and_then(toml::Value::as_str) == Some("trusted")
        })
    })
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
            ManagedDir::subdirs(project_root.join(".agents").join("skills")).recursive(),
            ManagedDir::files_except(
                project_root.join(".codex").join("agents"),
                ".toml",
                |path| !is_conforme_written_agent(path) || is_builtin_agent_override(path),
            )
            .recursive(),
            ManagedDir::bundles(project_root.join(".codex").join("plugins")),
        ]
    }

    /// `.codex/config.toml` holds the user's settings and the repository
    /// marketplace lists their own plugins too: both are merged.
    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".codex/config.toml")) || path.ends_with(Path::new(MARKETPLACE))
    }

    fn tool_warnings(&self, project_root: &Path, config: &NormalizedConfig) -> Vec<String> {
        // Codex loads project `.codex/` layers (config.toml, so the MCP
        // servers and plugin switches, and agents/) only in a trusted
        // project; skills are not gated.
        let gated = !config.mcp_servers.is_empty()
            || !config.plugins.is_empty()
            || !config.plugin_refs.is_empty()
            || config
                .agents
                .iter()
                .any(|agent| !crate::skills::is_codex_builtin_agent(&agent.name));
        let untrusted = (gated && !codex_trusts(project_root)).then(|| {
            format!(
                "Codex loads .codex/config.toml (MCP servers, plugins) and .codex/agents/ only in \
                 a trusted project, and {} is not trusted: accept Codex's trust prompt, or add \
                 `{}` with `trust_level = \"trusted\"` to ~/.codex/config.toml",
                project_root.display(),
                codex_trust_table(project_root)
            )
        });
        let mut bundles =
            crate::plugins::bundle_warnings(project_root, config, &crate::plugins::CODEX_PLUGINS);
        // Codex installs an enabled marketplace plugin only from a marketplace
        // the user added: it clones none a project names.
        let mut markets: Vec<&str> = config
            .plugin_refs
            .iter()
            .map(|r| r.marketplace.as_str())
            .filter(|m| *m != marketplace_name(project_root))
            .collect();
        markets.sort_unstable();
        markets.dedup();
        for market in markets {
            let add = config
                .marketplaces
                .iter()
                .find(|m| m.name == market)
                .and_then(|m| crate::marketplaces::codex_add_argument(&m.source));
            bundles.push(match add {
                Some(argument) => format!(
                    "marketplace {market}: Codex installs its plugins once each user adds it, \
                     with `codex plugin marketplace add {argument}`"
                ),
                None => format!(
                    "marketplace {market}: Codex installs its plugins once each user adds it \
                     (`codex plugin marketplace add <github repo, git URL or path>`)"
                ),
            });
        }
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
            .chain(
                config
                    .mcp_servers
                    .iter()
                    .filter(|server| !crate::mcp::is_codex_server_name(&server.name))
                    .map(|server| {
                        format!(
                            "MCP server {}: Codex refuses a server name outside \
                             `^[a-zA-Z0-9_:@/.-]+$` and does not start it; rename it in the source",
                            server.name
                        )
                    }),
            )
            .chain(untrusted)
            .chain(bundles)
            .collect()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // Read skills back from the shared `.agents/skills/` location so a Codex
        // project round-trips as a source. Codex searches it 6 levels deep.
        let skills =
            crate::skills::read_skills_recursive(&project_root.join(".agents").join("skills"), 6)?;
        let codex_config = project_root.join(".codex").join("config.toml");
        let mcp_servers = if codex_config.exists() {
            let content = crate::config::read_text(&codex_config)?;
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
                plugins: {
                    let switches = plugin_switches(project_root, &marketplace_name(project_root));
                    crate::plugins::read_plugins(
                        project_root,
                        &crate::plugins::CODEX_PLUGINS,
                        &|name| switches.get(&crate::config::sanitize_name(name)).copied(),
                    )?
                },
                // Codex keeps the marketplaces themselves per user.
                plugin_refs: crate::config::read_text(&config_path(project_root))
                    .map(|text| {
                        crate::marketplaces::read_codex_refs(&text, &marketplace_name(project_root))
                    })
                    .unwrap_or_default(),
                marketplaces: Vec::new(),
            },
        )
    }

    fn holds_plugin_refs(&self) -> bool {
        true
    }

    fn generate_flat(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        // Codex reads AGENTS.md natively — no need to re-generate it
        // since AGENTS.md is already our source of truth.
        // Generate skills as .agents/skills/<name>/SKILL.md (Codex format).
        let mut files = crate::skills::generate_skills(
            project_root,
            &config.skills,
            &crate::skills::CODEX_SKILLS,
        )?;
        files.extend(crate::skills::generate_codex_agents(
            project_root,
            &config.agents,
        )?);

        // Plugin bundles in .codex/plugins/<plugin>/, listed in the repository
        // marketplace and enabled in .codex/config.toml.
        files.extend(crate::plugins::generate_plugins(
            project_root,
            &config.plugins,
            &crate::plugins::CODEX_PLUGINS,
        )?);
        let MergedMarketplace {
            file: marketplace,
            listed,
            owned,
        } = merged_marketplace(project_root, &config.plugins)?;
        let switches: Vec<(String, bool)> = listed
            .iter()
            .map(|name| {
                let disabled = config
                    .plugins
                    .iter()
                    .any(|p| crate::config::sanitize_name(&p.name) == *name && p.disabled);
                (name.clone(), !disabled)
            })
            .collect();
        let plugin_tables = marketplace.is_some();
        files.extend(marketplace);

        // Merge MCP servers (and the plugin switches) into the project-scoped
        // config. The merge preserves unrelated Codex settings and
        // server-specific options not represented by conforme's normalized
        // model.
        if !config.mcp_servers.is_empty() || plugin_tables || !config.plugin_refs.is_empty() {
            let path = config_path(project_root);
            let existing = if path.exists() {
                crate::config::read_text(&path)?
            } else {
                String::new()
            };
            let mut content = existing.clone();
            if !config.mcp_servers.is_empty() {
                content = crate::mcp::merge_codex_mcp_toml(&content, &config.mcp_servers)?;
            }
            if plugin_tables {
                content = merge_codex_plugins_toml(
                    &content,
                    &marketplace_name(project_root),
                    &switches,
                    &owned,
                )?;
            }
            content = crate::marketplaces::merge_codex_refs_toml(
                &content,
                &config.plugin_refs,
                &marketplace_name(project_root),
            )?;
            if path.exists() || !content.trim().is_empty() {
                files.push((path, content));
            }
        }

        Ok(files)
    }

    fn plugin_shape(&self) -> Option<&'static crate::plugins::PluginShape> {
        Some(&crate::plugins::CODEX_PLUGINS)
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

    #[test]
    fn test_codex_trust_table_is_valid_toml_for_any_path() {
        // A Windows path holds `\`, which a TOML basic string escapes; the
        // advice must paste into ~/.codex/config.toml as written.
        assert_eq!(
            strip_verbatim_prefix(r"\\?\C:\Users\me"),
            Some(r"C:\Users\me")
        );
        assert_eq!(strip_verbatim_prefix(r"\\?\UNC\server\share"), None);
        assert_eq!(strip_verbatim_prefix("/home/me"), None);

        let tmp = tempfile::tempdir().unwrap();
        // Windows forbids `"` in a file name; its `\` separators are escaped.
        let root = tmp
            .path()
            .join(if cfg!(windows) { "repo" } else { "my \"repo\"" });
        std::fs::create_dir_all(&root).unwrap();
        let table = codex_trust_table(&root);
        let parsed: toml::Table = format!("{table}\ntrust_level = \"trusted\"\n")
            .parse()
            .unwrap();
        let (key, _) = parsed["projects"]
            .as_table()
            .unwrap()
            .iter()
            .next()
            .unwrap();
        assert_eq!(Path::new(key), canonical_path(&root));
    }

    #[test]
    fn test_codex_trust_keys_resolve_a_worktree_to_its_main_checkout() {
        // Codex checks trust on "the root of the main repository", a
        // worktree included (codex-rs/git-utils/src/trust.rs).
        let tmp = tempfile::tempdir().unwrap();
        // Codex's form of the path: no `\\?\` prefix on Windows.
        let main = canonical_path(tmp.path()).join("repo");
        let gitdir = main.join(".git").join("worktrees").join("wt");
        std::fs::create_dir_all(&gitdir).unwrap();
        let worktree = main.join("wt");
        std::fs::create_dir_all(worktree.join("sub")).unwrap();
        std::fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", gitdir.display()),
        )
        .unwrap();

        let keys = codex_trust_keys(&worktree.join("sub"));
        assert_eq!(keys.last(), Some(&main), "{keys:?}");
        assert_eq!(codex_trust_keys(&main).last(), Some(&main));
    }

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
