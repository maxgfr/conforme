use anyhow::Result;
use serde_json::Value;
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
/// (https://opencode.ai/docs/migrate-v1/#instruction-files).
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

/// A notice naming the project config files that hold OpenCode 1 entries
/// (`mcp.<name>` servers, an `agent` key), which conforme neither reads nor
/// writes: OpenCode 2 still loads them, so they act without being synced.
pub fn opencode_1_notice(project_root: &Path) -> Option<String> {
    let files: Vec<&str> = CONFIG_FILES
        .iter()
        .copied()
        .filter(|name| {
            crate::json_settings::load(&project_root.join(name))
                .ok()
                .flatten()
                .is_some_and(|root| {
                    root.get("agent").is_some_and(|a| a.is_object())
                        || root
                            .get("mcp")
                            .and_then(Value::as_object)
                            .is_some_and(|mcp| {
                                mcp.iter().any(|(name, entry)| {
                                    name != "servers"
                                        && name != "timeout"
                                        && crate::json_settings::is_expressible_server(entry)
                                })
                            })
                })
        })
        .collect();
    (!files.is_empty()).then(|| {
        format!(
            "{} holds OpenCode 1 entries (`mcp.<name>` servers, `agent`), which conforme neither \
             reads nor syncs (it targets OpenCode 2): move them to `mcp.servers` / `agents` to sync them",
            files.join(", ")
        )
    })
}

impl AiToolAdapter for OpenCodeAdapter {
    /// What its variable syntax cannot carry (see [`crate::mcp::env_ref_losses`]),
    /// the SSE servers OpenCode 2 cannot reach, and OpenCode 1 entries left
    /// in its config files.
    fn tool_warnings(&self, project_root: &Path, config: &NormalizedConfig) -> Vec<String> {
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
        warnings.extend(opencode_1_notice(project_root));
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
    /// only merges `mcp.servers` into it, so `remove`/`migrate` must never
    /// delete the file wholesale.
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
            )
            .recursive(),
            ManagedDir::subdirs(project_root.join(".opencode").join("skills")).recursive(),
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
        // `.opencode/agent/`, then the `agents` key in `opencode.json`: the
        // markdown files win on a clash.
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
        // `environment`) — not the standard `mcpServers` layout. OpenCode
        // parses the file as JSONC, so comments are accepted here too. Later
        // files override earlier ones, server by server. OpenCode 1's
        // `mcp.<name>` and `agent` entries are not conforme's concern: they
        // are neither read nor touched.
        let mut mcp_servers: Vec<crate::config::NormalizedMcpServer> = Vec::new();
        for name in CONFIG_FILES {
            let Some(root) = crate::json_settings::load(&project_root.join(name))? else {
                continue;
            };
            if let Some(servers) = root.get("mcp").and_then(|mcp| mcp.get("servers")) {
                for server in crate::mcp::OPENCODE_SHAPE.read(servers) {
                    mcp_servers.retain(|s| s.name != server.name);
                    mcp_servers.push(server);
                }
            }
            if let Some(entries) = root.get("agents") {
                for agent in crate::mcp::parse_opencode_agent_object(entries) {
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
                plugins: Vec::new(),
            },
        )
    }

    fn generate_flat(
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

/// The OpenCode config file a sync rewrites: the servers merged into
/// `mcp.servers` of `.opencode/opencode.json(c)` (see [`config_path`]), every
/// other key of the file kept. Like every other MCP target, a source with no
/// server leaves the file alone. No other config file is written: OpenCode 1
/// entries (`mcp.<name>`, `agent.<name>`), wherever they sit, are the user's.
fn config_files(project_root: &Path, config: &NormalizedConfig) -> Result<Vec<(PathBuf, String)>> {
    if config.mcp_servers.is_empty() {
        return Ok(Vec::new());
    }
    let target = config_path(project_root);
    let existing = crate::json_settings::load(&target)?;
    let existing_mcp = existing.as_ref().and_then(|f| f.get("mcp"));
    let mut mcp = existing_mcp
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let servers = crate::json_settings::merge_server_entries(
        mcp.get("servers"),
        crate::mcp::OPENCODE_SHAPE.build(&config.mcp_servers),
        &crate::mcp::OPENCODE_SHAPE.owned_keys(),
    );
    mcp.insert("servers".into(), Value::Object(servers));
    let mcp = Value::Object(mcp);
    // An unchanged key keeps its text (comments, layout).
    let set: &[(&str, Value)] = if existing_mcp == Some(&mcp) {
        &[]
    } else {
        &[("mcp", mcp)]
    };
    let schema = Value::String("https://opencode.ai/config.json".to_string());
    let json = crate::json_settings::render(existing.as_ref(), set, &[("$schema", schema)])?;
    Ok(vec![(target, json)])
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
    fn test_opencode_1_entries_are_neither_read_nor_touched() {
        // conforme targets OpenCode 2 only: `mcp.<name>` servers and
        // `agent.<name>` entries in OpenCode 1's shape are the user's, in the
        // root file as in the one conforme merges into.
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".opencode")).unwrap();
        let root = r#"{
  "theme": "dark",
  "mcp": {
    "mine": {"type": "local", "command": ["mine"]},
    "fs": {"type": "local", "command": ["old"], "timeout": 5000}
  },
  "agent": {
    "reviewer": {"description": "Review", "mode": "subagent", "prompt": "Old."}
  }
}
"#;
        std::fs::write(tmp.path().join("opencode.json"), root).unwrap();
        std::fs::write(
            tmp.path().join(".opencode/opencode.json"),
            r#"{
  "mcp": {
    "legacy": {"type": "remote", "url": "https://e.x/mcp", "enabled": false},
    "servers": {
      "native": {"type": "local", "command": ["native"]},
      "gone": {"type": "local", "command": ["gone"]}
    }
  },
  "agent": {
    "old": {"description": "Old", "mode": "subagent", "prompt": "Old."}
  }
}
"#,
        )
        .unwrap();

        // Read: only `mcp.servers` and `agents`.
        let read = adapter.read(tmp.path()).unwrap();
        let names: Vec<_> = read.mcp_servers.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["gone", "native"]);
        assert!(read.agents.is_empty(), "{:?}", read.agents);

        // Write: the root file is not generated at all, and in the target
        // only `mcp.servers` changes (a server the source lacks is dropped,
        // as in every target).
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
        assert!(
            !files
                .iter()
                .any(|(p, _)| *p == tmp.path().join("opencode.json")),
            "{files:?}"
        );
        let (_, text) = files
            .iter()
            .find(|(p, _)| *p == tmp.path().join(".opencode/opencode.json"))
            .unwrap();
        let json = crate::json_settings::parse_jsonc(text).unwrap();
        assert_eq!(
            json["mcp"]["legacy"],
            serde_json::json!({"type": "remote", "url": "https://e.x/mcp", "enabled": false})
        );
        assert_eq!(
            json["agent"]["old"],
            serde_json::json!({"description": "Old", "mode": "subagent", "prompt": "Old."})
        );
        let servers = json["mcp"]["servers"].as_object().unwrap();
        assert_eq!(servers.keys().collect::<Vec<_>>(), ["fs"]);
        assert_eq!(servers["fs"]["command"], serde_json::json!(["npx"]));

        // A second sync, once the file is written, changes nothing.
        std::fs::write(tmp.path().join(".opencode/opencode.json"), text).unwrap();
        let again = adapter.generate(tmp.path(), &config).unwrap();
        let (_, again) = again
            .iter()
            .find(|(p, _)| *p == tmp.path().join(".opencode/opencode.json"))
            .unwrap();
        assert_eq!(again, text);
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
