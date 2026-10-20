use anyhow::Result;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
use crate::config::NormalizedConfig;

/// OpenCode 2 adapter.
/// OpenCode 2 reads AGENTS.md (and no CLAUDE.md fallback), skills from
/// .opencode/skills/ (also .claude/skills/, .agents/skills/) and agents from
/// .opencode/agents/<name>.md. MCP servers live inside `opencode.json` under
/// `mcp.servers`.
pub struct OpenCodeAdapter;

/// OpenCode 2 "currently only discovers AGENTS.md"
/// (https://opencode.ai/v2/docs/migrate-v1/#instruction-files).
const INSTRUCTION_FILES: &[&str] = &["AGENTS.md"];

/// The project config files OpenCode merges, lowest precedence first.
const CONFIG_FILES: &[&str] = &[
    "opencode.json",
    "opencode.jsonc",
    ".opencode/opencode.json",
    ".opencode/opencode.jsonc",
];

/// The config file conforme merges into: `.opencode/opencode.jsonc` when it
/// exists, else `.opencode/opencode.json`. Never the root `opencode.json(c)`:
/// Kilo Code loads that one with OpenCode 1's schema, where `mcp.servers`
/// would be a server named `servers`, and refuses a project file holding
/// `{env:VAR}`; it never reads `.opencode/`.
fn config_path(project_root: &Path) -> PathBuf {
    let dir = project_root.join(".opencode");
    let jsonc = dir.join("opencode.jsonc");
    if jsonc.is_file() {
        jsonc
    } else {
        dir.join("opencode.json")
    }
}

impl AiToolAdapter for OpenCodeAdapter {
    /// What its variable syntax cannot carry (see [`crate::mcp::env_ref_losses`]),
    /// and the SSE servers OpenCode 2 cannot reach.
    fn warnings(&self, _project_root: &Path, config: &NormalizedConfig) -> Vec<String> {
        let mut warnings = crate::mcp::env_ref_losses(
            &config.mcp_servers,
            crate::mcp::EnvRefStyle::OpenCode,
            false,
            "OpenCode",
        );
        for server in &config.mcp_servers {
            if matches!(server.transport, crate::config::McpTransport::Sse { .. }) {
                warnings.push(format!(
                    "MCP server {}: OpenCode 2 connects remote servers over streamable HTTP only, \
                     so this SSE server connects only if it also speaks streamable HTTP at that \
                     URL (or run it through a stdio bridge such as mcp-remote)",
                    server.name
                ));
            }
        }
        warnings
    }

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

    fn reads_agents_md(&self, _project_root: &Path) -> bool {
        true
    }

    fn source_files(&self, project_root: &Path) -> Vec<PathBuf> {
        crate::adapters::first_existing_file(project_root, INSTRUCTION_FILES)
    }

    /// `opencode.json` is the user's whole OpenCode configuration; conforme
    /// only merges `mcp.servers` into it (and takes its own OpenCode 1
    /// entries out), so `remove`/`migrate` must never delete the file
    /// wholesale.
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
        // `.opencode/agent/`, then the `agents` key in `opencode.json` (and
        // OpenCode 1's `agent`, which OpenCode 2 still loads): the markdown
        // files win on a clash.
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

        // MCP servers live inside `opencode.json` under `mcp.servers`, in
        // OpenCode's own shape (`type: local/remote`, `command` array,
        // `environment`) — not the standard `mcpServers` layout. OpenCode 2
        // still loads OpenCode 1's `mcp.<name>` entries, a native one of the
        // same name winning. OpenCode parses the file as JSONC, so comments
        // are accepted here too. Later files override earlier ones, server by
        // server.
        let mut mcp_servers: Vec<crate::config::NormalizedMcpServer> = Vec::new();
        for name in CONFIG_FILES {
            let Some(root) = crate::json_settings::load(&project_root.join(name))? else {
                continue;
            };
            if let Some(mcp) = root.get("mcp") {
                let v1 = crate::mcp::OPENCODE_V1_SHAPE.read(mcp);
                let v2 = mcp
                    .get("servers")
                    .map(|servers| crate::mcp::OPENCODE_SHAPE.read(servers))
                    .unwrap_or_default();
                for server in v1.into_iter().chain(v2) {
                    mcp_servers.retain(|s| s.name != server.name);
                    mcp_servers.push(server);
                }
            }
            for key in ["agents", "agent"] {
                if let Some(agent) = root.get(key) {
                    for agent in crate::mcp::parse_opencode_agent_object(agent) {
                        if !agents.iter().any(|a| a.name == agent.name) {
                            agents.push(agent);
                        }
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
            files.extend(crate::skills::generate_skills(
                project_root,
                &config.skills,
                &crate::skills::OPENCODE_SKILLS,
            )?);
        }

        // Agents are written as .opencode/agents/<name>.md only: OpenCode 2's
        // preferred location, whose frontmatter is already its native shape.
        if !config.agents.is_empty() {
            files.extend(crate::skills::generate_opencode_agents_md(
                project_root,
                &config.agents,
            )?);
        }

        files.extend(config_files(project_root, config)?);
        Ok(files)
    }
}

/// The OpenCode config files a sync rewrites: the servers merged into
/// `mcp.servers` of `.opencode/opencode.json(c)` (see [`config_path`]), and
/// what earlier conforme versions wrote in OpenCode 1's shape taken out of
/// every project config file — the root `opencode.json` included, which they
/// merged into when it existed.
///
/// What conforme takes out is what it owned before: the expressible
/// `mcp.<name>` servers of the file it merged into (moved to `mcp.servers`,
/// where the merge keeps a disabled one and the per-server keys conforme does
/// not own, converted to OpenCode 2's names), and, in `agent.<name>` entries,
/// the keys it wrote for an agent it now writes as markdown only. Like every
/// other MCP target, a source with no server leaves the servers alone.
fn config_files(project_root: &Path, config: &NormalizedConfig) -> Result<Vec<(PathBuf, String)>> {
    // Every existing config file, lowest precedence first, then the target:
    // OpenCode 1 servers move up to it in OpenCode's own order.
    let target = config_path(project_root);
    let mut paths: Vec<PathBuf> = CONFIG_FILES
        .iter()
        .map(|name| project_root.join(name))
        .filter(|path| path.is_file() && *path != target)
        .collect();
    paths.push(target.clone());

    let mut files = Vec::new();
    let mut legacy_servers = Map::new();
    for path in paths {
        let is_target = path == target;
        let writes_servers = is_target && !config.mcp_servers.is_empty();
        // With nothing to write or take out, a file conforme cannot parse
        // is only left alone.
        let existing = match crate::json_settings::load(&path) {
            Err(_) if config.mcp_servers.is_empty() && config.agents.is_empty() => continue,
            loaded => loaded?,
        };
        if existing.is_none() && !writes_servers {
            continue;
        }
        let mut set: Vec<(&str, Value)> = Vec::new();
        let mut remove: Vec<&str> = Vec::new();

        let mut mcp = existing
            .as_ref()
            .and_then(|f| f.get("mcp"))
            .and_then(Value::as_object)
            .cloned();
        if !config.mcp_servers.is_empty() {
            if let Some(mcp) = mcp.as_mut() {
                let v1: Vec<String> = mcp
                    .iter()
                    .filter(|(name, entry)| {
                        name.as_str() != "servers"
                            && crate::json_settings::is_expressible_server(entry)
                    })
                    .map(|(name, _)| name.clone())
                    .collect();
                for name in v1 {
                    if let Some(entry) = mcp.remove(&name) {
                        legacy_servers.insert(name, crate::mcp::opencode_v1_server_to_v2(&entry));
                    }
                }
            }
        }
        if writes_servers {
            let mcp = mcp.get_or_insert_with(Map::new);
            // A native entry wins over an OpenCode 1 one of the same name.
            let mut servers = mcp
                .get("servers")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for (name, entry) in std::mem::take(&mut legacy_servers) {
                servers.entry(name).or_insert(entry);
            }
            let servers = crate::json_settings::merge_server_entries(
                Some(&Value::Object(servers)),
                crate::mcp::OPENCODE_SHAPE.build(&config.mcp_servers),
                &crate::mcp::OPENCODE_SHAPE.owned_keys(),
            );
            mcp.insert("servers".into(), Value::Object(servers));
        }
        if let Some(mcp) = mcp {
            if existing.as_ref().and_then(|f| f.get("mcp")) != Some(&Value::Object(mcp.clone())) {
                if mcp.is_empty() {
                    remove.push("mcp");
                } else {
                    set.push(("mcp", Value::Object(mcp)));
                }
            }
        }

        if let Some(agent) = without_conforme_agents(
            project_root,
            existing.as_ref().and_then(|f| f.get("agent")),
            &config.agents,
        ) {
            if agent.is_empty() {
                remove.push("agent");
            } else {
                set.push(("agent", Value::Object(agent)));
            }
        }

        if set.is_empty() && remove.is_empty() && !writes_servers {
            continue;
        }
        let defaults: &[(&str, Value)] = if writes_servers {
            &[(
                "$schema",
                Value::String("https://opencode.ai/config.json".to_string()),
            )]
        } else {
            &[]
        };
        let json =
            crate::json_settings::render_removing(existing.as_ref(), &set, defaults, &remove)?;
        files.push((path, json));
    }
    Ok(files)
}

/// The OpenCode 1 `agent` map with what earlier conforme versions wrote there
/// taken out, or `None` when there is nothing to take out. They wrote every
/// agent twice, as `.opencode/agents/<name>.md` and as `agent.<name>`; the
/// markdown file is now the only copy. In the entry of an agent the source
/// defines, conforme's keys go (`description`, `mode`, `prompt`, and `model`
/// when the source has one OpenCode can use; otherwise it may be the user's),
/// the user's own (`permission`, `temperature`, …) stay and still tune the
/// agent, and an entry left empty goes. An entry for an agent that has left
/// the source goes with its file (orphan cleanup removes it in the same
/// sync), provided it still has that file and holds only conforme's keys; an
/// entry without the file, or one the user extended, is the user's.
fn without_conforme_agents(
    project_root: &Path,
    existing: Option<&Value>,
    agents: &[crate::config::NormalizedAgent],
) -> Option<Map<String, Value>> {
    let entries = existing.and_then(Value::as_object)?;
    let mut kept = entries.clone();
    for agent in agents {
        let name = crate::config::sanitize_name(&agent.name);
        let Some(Value::Object(entry)) = kept.get_mut(&name) else {
            continue;
        };
        for key in crate::mcp::OPENCODE_OWNED_AGENT_KEYS {
            if *key != "model" || crate::skills::opencode_model(agent.model.as_deref()).is_some() {
                entry.remove(*key);
            }
        }
        if entry.is_empty() {
            kept.remove(&name);
        }
    }
    let generated: Vec<String> = agents
        .iter()
        .map(|a| crate::config::sanitize_name(&a.name))
        .collect();
    kept.retain(|name, entry| {
        generated.contains(name)
            || crate::mcp::is_opencode_builtin_agent(name)
            || !entry.as_object().is_some_and(|fields| {
                fields
                    .keys()
                    .all(|key| crate::mcp::OPENCODE_OWNED_AGENT_KEYS.contains(&key.as_str()))
            })
            || !project_root
                .join(".opencode")
                .join("agents")
                .join(format!("{name}.md"))
                .is_file()
    });
    (kept != *entries).then_some(kept)
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
            .replace('\\', "/")
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
        let json: serde_json::Value = serde_json::from_str(&files[0].1).unwrap();
        assert_eq!(json["$schema"], "https://opencode.ai/config.json");
        // OpenCode 2's native shape: servers under `mcp.servers`.
        let fs = &json["mcp"]["servers"]["fs"];
        assert_eq!(fs["type"], "local");
        // command should be a single array, not separate command + args
        assert_eq!(fs["command"], serde_json::json!(["npx", "-y", "@mcp/fs"]));
        assert!(fs.get("args").is_none());
        assert!(json["mcp"].get("fs").is_none());
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
        // The markdown agent only: no `agent`/`agents` entry in opencode.json.
        assert_eq!(files.len(), 1);
        assert!(files[0]
            .0
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with(".opencode/agents/reviewer.md"));
        assert!(files[0].1.contains("description: Code review"));
        assert!(files[0].1.contains("mode: subagent"));
    }

    fn fs_server() -> crate::config::NormalizedMcpServer {
        crate::config::NormalizedMcpServer {
            name: "fs".to_string(),
            transport: crate::config::McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec![],
            },
            env: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn test_generate_preserves_existing_opencode_json() {
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".opencode")).unwrap();
        std::fs::write(
            tmp.path().join(".opencode/opencode.jsonc"),
            "{\n  // mine\n  \"theme\": \"dark\",\n  \"model\": \"custom\",\n  \"mcp\": {\"timeout\": {\"catalog\": 1000}}\n}\n",
        )
        .unwrap();

        let config = NormalizedConfig {
            mcp_servers: vec![fs_server()],
            ..Default::default()
        };
        let files = adapter.generate(tmp.path(), &config).unwrap();
        assert_eq!(files.len(), 1);
        // The existing `.jsonc` wins over a new `.json`.
        assert_eq!(files[0].0, tmp.path().join(".opencode/opencode.jsonc"));
        let text = &files[0].1;
        // User-authored keys and comments must be preserved
        assert!(text.contains("// mine"));
        let json = crate::json_settings::parse_jsonc(text).unwrap();
        assert_eq!(json["theme"], "dark");
        assert_eq!(json["model"], "custom");
        assert_eq!(json["mcp"]["timeout"]["catalog"], 1000);
        assert_eq!(json["mcp"]["servers"]["fs"]["type"], "local");
    }

    #[test]
    fn test_root_opencode_json_is_never_the_target() {
        // Kilo Code loads the root `opencode.json` with OpenCode 1's schema:
        // `mcp.servers` there would be a server named `servers`.
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        let root = r#"{"theme":"dark"}"#;
        std::fs::write(tmp.path().join("opencode.json"), root).unwrap();
        let config = NormalizedConfig {
            mcp_servers: vec![fs_server()],
            ..Default::default()
        };
        let files = adapter.generate(tmp.path(), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, tmp.path().join(".opencode/opencode.json"));
    }

    #[test]
    fn test_v1_entries_written_by_earlier_versions_move_to_v2() {
        // An earlier conforme merged into the root `opencode.json` in
        // OpenCode 1's shape. Its servers move to `.opencode/opencode.json`
        // under `mcp.servers` (a disabled one the source lacks is the user's
        // and survives, converted), its agent keys go, and the user's keys
        // stay where they are.
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".opencode/agents")).unwrap();
        std::fs::write(tmp.path().join(".opencode/agents/old.md"), "x").unwrap();
        std::fs::write(
            tmp.path().join("opencode.json"),
            r#"{
  "$schema": "https://opencode.ai/config.json",
  "theme": "dark",
  "mcp": {
    "fs": {"type": "local", "command": ["old"], "timeout": 5000},
    "gone": {"type": "local", "command": ["gone"]},
    "off": {"type": "remote", "url": "https://e.x/mcp", "enabled": false},
    "github": {"enabled": false}
  },
  "agent": {
    "build": {"permission": {"bash": "ask"}},
    "reviewer": {"description": "Review", "mode": "subagent", "prompt": "Old.", "temperature": 0.1},
    "old": {"description": "Old", "mode": "subagent", "prompt": "Old."},
    "mine": {"description": "Mine", "mode": "subagent", "prompt": "Mine."}
  }
}
"#,
        )
        .unwrap();
        let config = NormalizedConfig {
            mcp_servers: vec![fs_server()],
            agents: vec![crate::config::NormalizedAgent {
                name: "reviewer".to_string(),
                description: "Review".to_string(),
                content: "Review.".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = adapter.generate(tmp.path(), &config).unwrap();
        let file = |rel: &str| {
            let (_, text) = files
                .iter()
                .find(|(p, _)| *p == tmp.path().join(rel))
                .unwrap_or_else(|| panic!("{rel} not generated: {files:?}"));
            crate::json_settings::parse_jsonc(text).unwrap()
        };

        let root = file("opencode.json");
        assert_eq!(root["theme"], "dark");
        // Only the toggle conforme cannot express is left in `mcp`.
        assert_eq!(
            root["mcp"],
            serde_json::json!({"github": {"enabled": false}})
        );
        assert_eq!(
            root["agent"],
            serde_json::json!({
                "build": {"permission": {"bash": "ask"}},
                "reviewer": {"temperature": 0.1},
                "mine": {"description": "Mine", "mode": "subagent", "prompt": "Mine."}
            })
        );

        let servers = &file(".opencode/opencode.json")["mcp"]["servers"];
        assert_eq!(
            servers["fs"],
            serde_json::json!({
                "type": "local",
                "command": ["npx"],
                "timeout": {"catalog": 5000, "execution": 5000}
            })
        );
        assert_eq!(
            servers["off"],
            serde_json::json!({"type": "remote", "url": "https://e.x/mcp", "disabled": true})
        );
        assert!(servers.get("gone").is_none(), "{servers}");

        // A second sync, once these files are written, changes nothing.
        for (path, text) in &files {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let again = adapter.generate(tmp.path(), &config).unwrap();
        let configs: Vec<_> = again
            .iter()
            .filter(|(p, _)| p.extension().is_some_and(|e| e == "json"))
            .collect();
        assert_eq!(configs.len(), 1, "{configs:?}");
        assert_eq!(
            configs[0].1,
            std::fs::read_to_string(&configs[0].0).unwrap()
        );
    }

    #[test]
    fn test_sse_server_is_warned_about() {
        let config = NormalizedConfig {
            mcp_servers: vec![crate::config::NormalizedMcpServer {
                name: "legacy".to_string(),
                transport: crate::config::McpTransport::Sse {
                    url: "https://e.x/sse".to_string(),
                    headers: Default::default(),
                },
                env: Default::default(),
            }],
            ..Default::default()
        };
        let warnings = make_adapter().warnings(Path::new("/tmp/test"), &config);
        assert!(
            warnings.iter().any(|w| w.contains("streamable HTTP only")),
            "{warnings:?}"
        );
    }
}
