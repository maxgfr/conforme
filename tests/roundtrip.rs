//! Round-trip tests: write config → read back → compare.
//! Validates that adapters with read() support can faithfully
//! round-trip a NormalizedConfig through write → read.

use conforme::adapters::AiToolAdapter;
use conforme::config::{
    ActivationMode, McpTransport, NormalizedAgent, NormalizedConfig, NormalizedMcpServer,
    NormalizedRule, NormalizedSkill,
};
use std::fs;
use tempfile::TempDir;

/// A config exercising skills, an agent, and both MCP transports — used to
/// verify the read()/generate() round-trip for those features.
fn rich_config() -> NormalizedConfig {
    NormalizedConfig {
        instructions: "Be helpful.".to_string(),
        rules: vec![],
        skills: vec![NormalizedSkill {
            name: "deploy".to_string(),
            description: "Deploy the app".to_string(),
            content: "Run deploy.".to_string(),
            allowed_tools: vec![],
            ..Default::default()
        }],
        agents: vec![NormalizedAgent {
            name: "reviewer".to_string(),
            description: "Review code".to_string(),
            content: "Look for bugs.".to_string(),
            model: Some("sonnet".to_string()),
            tools: vec!["Read".to_string(), "Grep".to_string()],
            ..Default::default()
        }],
        mcp_servers: vec![
            NormalizedMcpServer {
                name: "fs".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/fs".to_string()],
                },
                env: Default::default(),
            },
            NormalizedMcpServer {
                name: "api".to_string(),
                transport: McpTransport::Http {
                    url: "https://example.com/mcp".to_string(),
                    headers: Default::default(),
                },
                env: Default::default(),
            },
        ],
    }
}

fn mcp_names(config: &NormalizedConfig) -> Vec<String> {
    let mut names: Vec<String> = config.mcp_servers.iter().map(|s| s.name.clone()).collect();
    names.sort();
    names
}

fn find_http_url(config: &NormalizedConfig, name: &str) -> Option<String> {
    config
        .mcp_servers
        .iter()
        .find(|s| s.name == name)
        .and_then(|s| match &s.transport {
            McpTransport::Http { url, .. } => Some(url.clone()),
            _ => None,
        })
}

fn roundtrip_config() -> NormalizedConfig {
    NormalizedConfig {
        instructions: "Be helpful and concise.".to_string(),
        rules: vec![
            NormalizedRule {
                name: "TypeScript".to_string(),
                content: "Use strict mode.".to_string(),
                activation: ActivationMode::Always,
            },
            NormalizedRule {
                name: "API Rules".to_string(),
                content: "Follow REST conventions.".to_string(),
                activation: ActivationMode::GlobMatch(vec!["src/api/**".to_string()]),
            },
        ],
        skills: vec![],
        mcp_servers: vec![],
        agents: vec![],
    }
}

fn setup_tool(dir: &TempDir, tool: &str) {
    match tool {
        "cursor" => fs::create_dir_all(dir.path().join(".cursor")).unwrap(),
        "claude" => fs::create_dir_all(dir.path().join(".claude")).unwrap(),
        "devin" => fs::create_dir_all(dir.path().join(".devin")).unwrap(),
        "copilot" => {
            fs::create_dir_all(dir.path().join(".github")).unwrap();
            fs::write(
                dir.path().join(".github").join("copilot-instructions.md"),
                "",
            )
            .unwrap();
        }
        "kiro" => fs::create_dir_all(dir.path().join(".kiro")).unwrap(),
        "zoocode" => fs::create_dir_all(dir.path().join(".roo")).unwrap(),
        _ => {}
    }
}

#[test]
fn test_roundtrip_cursor() {
    let adapter = conforme::adapters::cursor::CursorAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "cursor");

    let config = roundtrip_config();
    adapter.write(dir.path(), &config).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.instructions, config.instructions);
    // Cursor writes always-rule as a separate .mdc file, so both rules come back
    assert_eq!(read_config.rules.len(), 2);
    // Verify the glob rule preserved its activation
    let glob_rule = read_config
        .rules
        .iter()
        .find(|r| matches!(&r.activation, ActivationMode::GlobMatch(_)))
        .unwrap();
    assert!(glob_rule.content.contains("Follow REST conventions."));
}

#[test]
fn test_roundtrip_claude() {
    let adapter = conforme::adapters::claude::ClaudeAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "claude");

    let config = roundtrip_config();
    adapter.write(dir.path(), &config).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    // Claude inlines always-rules into CLAUDE.md
    assert!(read_config.instructions.contains("Be helpful and concise."));
    assert!(read_config.instructions.contains("Use strict mode."));
    // Glob rule goes to a separate file
    assert_eq!(read_config.rules.len(), 1);
    assert!(matches!(
        &read_config.rules[0].activation,
        ActivationMode::GlobMatch(g) if g.contains(&"src/api/**".to_string())
    ));
}

#[test]
fn test_roundtrip_devin() {
    let adapter = conforme::adapters::devin::DevinAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "devin");

    let config = roundtrip_config();
    adapter.write(dir.path(), &config).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.instructions, config.instructions);
    assert_eq!(read_config.rules.len(), 2);
}

#[test]
fn test_roundtrip_copilot() {
    let adapter = conforme::adapters::copilot::CopilotAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "copilot");

    let config = roundtrip_config();
    adapter.write(dir.path(), &config).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    // Copilot inlines always-rules into copilot-instructions.md
    assert!(read_config.instructions.contains("Be helpful and concise."));
    assert!(read_config.instructions.contains("Use strict mode."));
    // Glob rule goes to .github/instructions/
    assert_eq!(read_config.rules.len(), 1);
}

#[test]
fn test_roundtrip_kiro() {
    let adapter = conforme::adapters::kiro::KiroAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "kiro");

    let config = roundtrip_config();
    adapter.write(dir.path(), &config).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.instructions, config.instructions);
    assert_eq!(read_config.rules.len(), 2);
    // Verify glob rule preserved
    let glob_rule = read_config
        .rules
        .iter()
        .find(|r| matches!(&r.activation, ActivationMode::GlobMatch(_)))
        .unwrap();
    assert!(glob_rule.content.contains("Follow REST conventions."));
}

#[test]
fn test_roundtrip_zoocode() {
    let adapter = conforme::adapters::zoocode::ZooCodeAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "zoocode");

    let config = NormalizedConfig {
        instructions: "Be helpful.".to_string(),
        rules: vec![NormalizedRule {
            name: "Security".to_string(),
            content: "No eval.".to_string(),
            activation: ActivationMode::Always,
        }],
        ..Default::default()
    };
    adapter.write(dir.path(), &config).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.instructions, "Be helpful.");
    assert_eq!(read_config.rules.len(), 1);
    assert!(read_config.rules[0].content.contains("No eval."));
}

#[test]
fn test_roundtrip_claude_agent_color() {
    // Claude-specific color + permissionMode must survive write → read.
    let adapter = conforme::adapters::claude::ClaudeAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "claude");

    let config = NormalizedConfig {
        agents: vec![NormalizedAgent {
            name: "reviewer".to_string(),
            description: "Review code".to_string(),
            content: "Look for bugs.".to_string(),
            model: Some("opus".to_string()),
            tools: vec!["Read".to_string(), "Grep".to_string()],
            color: Some("cyan".to_string()),
            permission_mode: Some("plan".to_string()),
        }],
        ..Default::default()
    };
    adapter.write(dir.path(), &config).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.agents.len(), 1);
    let agent = &read_config.agents[0];
    assert_eq!(agent.color.as_deref(), Some("cyan"));
    assert_eq!(agent.permission_mode.as_deref(), Some("plan"));
    assert_eq!(agent.model.as_deref(), Some("opus"));
    assert_eq!(agent.tools, vec!["Read", "Grep"]);
}

#[test]
fn test_roundtrip_cursor_skills() {
    let adapter = conforme::adapters::cursor::CursorAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "cursor");

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.skills[0].name, "deploy");
    assert_eq!(read_config.skills[0].description, "Deploy the app");
    // Cursor writes subagents (.md) and mcp.json too.
    assert_eq!(read_config.agents.len(), 1);
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
}

#[test]
fn test_roundtrip_copilot_skills_agents_mcp() {
    let adapter = conforme::adapters::copilot::CopilotAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "copilot");

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.skills[0].name, "deploy");
    assert_eq!(read_config.agents.len(), 1);
    assert_eq!(read_config.agents[0].name, "reviewer");
    // `sonnet` is a Claude Code alias, not a Copilot model: it is left out.
    assert_eq!(read_config.agents[0].model, None);
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
}

#[test]
fn test_roundtrip_kiro_skills_agents_mcp() {
    let adapter = conforme::adapters::kiro::KiroAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "kiro");

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.agents.len(), 1);
    assert_eq!(read_config.agents[0].name, "reviewer");
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
}

#[test]
fn test_roundtrip_gemini_skills_agents_mcp() {
    let adapter = conforme::adapters::gemini::GeminiAdapter;
    let dir = TempDir::new().unwrap();

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.agents.len(), 1);
    assert_eq!(read_config.agents[0].name, "reviewer");
    // Gemini writes httpUrl (no type) — parser must still recover the HTTP URL.
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
}

#[test]
fn test_roundtrip_zed_skills_mcp() {
    let adapter = conforme::adapters::zed::ZedAdapter;
    let dir = TempDir::new().unwrap();

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.skills[0].name, "deploy");
    // Zed context_servers with a bare `url` (no type) must parse as HTTP.
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
}

#[test]
fn test_roundtrip_devin_skills() {
    let adapter = conforme::adapters::devin::DevinAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "devin");

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.skills[0].name, "deploy");
    // Devin Local reads project MCP servers from `.devin/mcp_config.json`.
    assert!(dir.path().join(".devin/mcp_config.json").exists());
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
}

#[test]
fn test_roundtrip_zoocode_skills_mcp() {
    let adapter = conforme::adapters::zoocode::ZooCodeAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "zoocode");

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.skills[0].name, "deploy");
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
}

#[test]
fn test_roundtrip_codex_skills() {
    let adapter = conforme::adapters::codex::CodexAdapter;
    let dir = TempDir::new().unwrap();

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.skills[0].name, "deploy");
    assert_eq!(read_config.skills[0].content, "Run deploy.");
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
}

#[test]
fn test_codex_config_merge_preserves_user_settings() {
    let adapter = conforme::adapters::codex::CodexAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".codex")).unwrap();
    fs::write(
        dir.path().join(".codex/config.toml"),
        "# Keep this comment\nmodel = \"gpt-test\"\n\n[mcp_servers.local]\nurl = \"http://localhost:3000/mcp\"\nstartup_timeout_sec = 20\n",
    )
    .unwrap();

    adapter.write(dir.path(), &rich_config()).unwrap();

    let content = fs::read_to_string(dir.path().join(".codex/config.toml")).unwrap();
    assert!(content.contains("# Keep this comment"));
    assert!(content.contains("model = \"gpt-test\""));
    assert!(content.contains("[mcp_servers.local]"));
    assert!(content.contains("startup_timeout_sec = 20"));
    assert!(content.contains("[mcp_servers.fs]"));
    assert!(content.contains("[mcp_servers.api]"));
}

#[test]
fn test_roundtrip_amp_skills_mcp() {
    let adapter = conforme::adapters::amp::AmpAdapter;
    let dir = TempDir::new().unwrap();

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.skills[0].name, "deploy");
    // Amp keys its servers under `amp.mcpServers` with no `type` field.
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
}

#[test]
fn test_roundtrip_deepseek_skills() {
    let adapter = conforme::adapters::deepseek::DeepSeekAdapter;
    let dir = TempDir::new().unwrap();

    adapter.write(dir.path(), &rich_config()).unwrap();

    // Skills land in the harness-native project root.
    let skill = dir.path().join(".dsh/skills/deploy/SKILL.md");
    assert!(skill.exists());

    let read_config = adapter.read(dir.path()).unwrap();
    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.skills[0].name, "deploy");
    assert_eq!(read_config.skills[0].description, "Deploy the app");
    // The harness has no project-scoped MCP file and no user-defined agents.
    assert!(read_config.mcp_servers.is_empty());
    assert!(read_config.agents.is_empty());
}

#[test]
fn test_amp_settings_merge_preserves_user_keys() {
    let adapter = conforme::adapters::amp::AmpAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".amp")).unwrap();
    fs::write(
        dir.path().join(".amp").join("settings.json"),
        r#"{ "amp.notifications.enabled": true }"#,
    )
    .unwrap();

    adapter.write(dir.path(), &rich_config()).unwrap();

    let settings = fs::read_to_string(dir.path().join(".amp").join("settings.json")).unwrap();
    assert!(settings.contains("amp.notifications.enabled"));
    assert!(settings.contains("amp.mcpServers"));
}

#[test]
fn test_roundtrip_opencode_skills_agents_mcp() {
    let adapter = conforme::adapters::opencode::OpenCodeAdapter;
    let dir = TempDir::new().unwrap();

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.skills[0].name, "deploy");
    assert_eq!(read_config.agents.len(), 1);
    assert_eq!(read_config.agents[0].name, "reviewer");
    // OpenCode stores `command` as a single [cmd, ...args] array with
    // `type: local`/`remote`, so it needs its own parser.
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
    let fs_server = read_config
        .mcp_servers
        .iter()
        .find(|s| s.name == "fs")
        .unwrap();
    match &fs_server.transport {
        McpTransport::Stdio { command, args } => {
            assert_eq!(command, "npx");
            assert_eq!(args, &["-y".to_string(), "@mcp/fs".to_string()]);
        }
        other => panic!("expected stdio transport, got {other:?}"),
    }
}

// Test that sync → check is consistent (idempotency through the trait)
#[test]
fn test_write_then_generate_matches() {
    let adapter = conforme::adapters::cursor::CursorAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "cursor");

    let config = roundtrip_config();

    // First write
    let report = adapter.write(dir.path(), &config).unwrap();
    assert!(!report.files_written.is_empty());

    // Second write should report no changes
    let report2 = adapter.write(dir.path(), &config).unwrap();
    assert!(report2.files_written.is_empty());
    assert!(!report2.files_unchanged.is_empty());
}

// --- Source-side discovery: nested rule directories and .claude/CLAUDE.md ---
//
// Claude Code, Cursor and Zoo Code all scan their rules directory recursively.
// Reading only the top level silently dropped nested rules when one of those
// tools was used as the sync source.

#[test]
fn test_claude_reads_nested_rules() {
    let adapter = conforme::adapters::claude::ClaudeAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".claude/rules/frontend")).unwrap();
    fs::write(dir.path().join("CLAUDE.md"), "Root instructions.").unwrap();
    fs::write(
        dir.path().join(".claude/rules/testing.md"),
        "---\npaths:\n  - \"**/*.test.ts\"\n---\nUse vitest.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".claude/rules/frontend/react.md"),
        "---\npaths:\n  - \"src/**/*.tsx\"\n---\nPrefer function components.\n",
    )
    .unwrap();

    let config = adapter.read(dir.path()).unwrap();

    assert_eq!(config.instructions, "Root instructions.");
    let names: Vec<&str> = config.rules.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"react"), "nested rule dropped: {:?}", names);
    assert!(names.contains(&"testing"), "top-level rule dropped");
    let react = config.rules.iter().find(|r| r.name == "react").unwrap();
    assert!(matches!(
        &react.activation,
        ActivationMode::GlobMatch(globs) if globs == &["src/**/*.tsx"]
    ));
}

#[test]
fn test_claude_reads_and_writes_dot_claude_claude_md() {
    let adapter = conforme::adapters::claude::ClaudeAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".claude")).unwrap();
    // Claude Code accepts the project CLAUDE.md at ./.claude/CLAUDE.md too.
    fs::write(
        dir.path().join(".claude/CLAUDE.md"),
        "Instructions in .claude/CLAUDE.md.",
    )
    .unwrap();

    let config = adapter.read(dir.path()).unwrap();
    assert_eq!(config.instructions, "Instructions in .claude/CLAUDE.md.");

    // Writing must target the same file, not create a competing root CLAUDE.md
    // that Claude Code would load on top of it.
    let files = adapter.generate(dir.path(), &config).unwrap();
    assert!(
        files
            .iter()
            .any(|(p, _)| p == &dir.path().join(".claude/CLAUDE.md")),
        "expected write to .claude/CLAUDE.md, got {:?}",
        files.iter().map(|(p, _)| p).collect::<Vec<_>>()
    );
    assert!(
        !files
            .iter()
            .any(|(p, _)| p == &dir.path().join("CLAUDE.md")),
        "must not create a second root CLAUDE.md"
    );
}

#[test]
fn test_claude_prefers_root_claude_md_when_both_exist() {
    let adapter = conforme::adapters::claude::ClaudeAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".claude")).unwrap();
    fs::write(dir.path().join("CLAUDE.md"), "Root wins.").unwrap();
    fs::write(dir.path().join(".claude/CLAUDE.md"), "Nested.").unwrap();

    assert_eq!(adapter.read(dir.path()).unwrap().instructions, "Root wins.");
}

#[test]
fn test_cursor_reads_nested_rules() {
    let adapter = conforme::adapters::cursor::CursorAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".cursor/rules/backend")).unwrap();
    fs::write(
        dir.path().join(".cursor/rules/backend/rpc.mdc"),
        "---\ndescription: RPC conventions\nalwaysApply: false\n---\nUse gRPC.\n",
    )
    .unwrap();

    let config = adapter.read(dir.path()).unwrap();

    assert_eq!(config.rules.len(), 1, "nested .mdc rule dropped");
    assert_eq!(config.rules[0].name, "rpc");
    assert!(config.rules[0].content.contains("Use gRPC."));
}

#[test]
fn test_zoocode_reads_nested_rules() {
    let adapter = conforme::adapters::zoocode::ZooCodeAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".roo/rules/nested")).unwrap();
    fs::write(dir.path().join(".roo/rules/00-general.md"), "General.").unwrap();
    fs::write(dir.path().join(".roo/rules/nested/01-style.md"), "Style.").unwrap();

    let config = adapter.read(dir.path()).unwrap();

    assert_eq!(config.instructions, "General.");
    assert_eq!(config.rules.len(), 1, "nested rule dropped");
    // The `NN-` ordering prefix is not part of the rule's name.
    assert_eq!(config.rules[0].name, "style");
}

/// Zoo Code has no activation modes: a glob rule is written always-on with an
/// `Intended scope` comment. Reading it back must restore the scope and the
/// name (without the `NN-` prefix), so a second write is byte-identical
/// instead of growing `01-01-security.md`.
#[test]
fn test_roundtrip_zoocode_is_stable() {
    let adapter = conforme::adapters::zoocode::ZooCodeAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "zoocode");
    let config = NormalizedConfig {
        instructions: "Be helpful.".to_string(),
        rules: vec![
            NormalizedRule {
                name: "security".to_string(),
                content: "No eval.".to_string(),
                activation: ActivationMode::Always,
            },
            NormalizedRule {
                name: "ts".to_string(),
                content: "Use TS.".to_string(),
                activation: ActivationMode::GlobMatch(vec![
                    "src/**/*.{ts,tsx}".to_string(),
                    "lib/**".to_string(),
                ]),
            },
        ],
        ..Default::default()
    };
    let first = adapter.generate(dir.path(), &config).unwrap();
    adapter.write(dir.path(), &config).unwrap();

    let read_config = adapter.read(dir.path()).unwrap();
    assert_eq!(read_config.rules[0].name, "security");
    assert_eq!(read_config.rules[1].name, "ts");
    assert_eq!(read_config.rules[1].activation, config.rules[1].activation);
    assert_eq!(read_config.rules[1].content, "Use TS.");
    assert_eq!(adapter.generate(dir.path(), &read_config).unwrap(), first);
}

#[test]
fn test_zoocode_reads_roorules_when_rules_dir_is_empty() {
    // Zoo Code falls back to `.roorules` when `.roo/rules/` is missing or empty.
    let adapter = conforme::adapters::zoocode::ZooCodeAdapter;
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".roorules"), "Legacy rules.\n").unwrap();
    assert_eq!(
        adapter.read(dir.path()).unwrap().instructions,
        "Legacy rules."
    );

    fs::create_dir_all(dir.path().join(".roo/rules")).unwrap();
    fs::write(dir.path().join(".roo/rules/00-general.md"), "New.\n").unwrap();
    assert_eq!(adapter.read(dir.path()).unwrap().instructions, "New.");
}

#[test]
fn test_zoocode_is_not_detected_from_clinerules() {
    // `.clinerules` is Cline's; Zoo Code only reads it as a legacy file.
    use conforme::adapters::AiToolAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".clinerules")).unwrap();
    assert!(!conforme::adapters::zoocode::ZooCodeAdapter.detect(dir.path()));
}

/// Claude Code accepts `paths` as a comma-separated string, scans
/// `.claude/agents/` recursively, and reads `yes`/`on`/`1` as booleans.
#[test]
fn test_claude_reads_documented_frontmatter_variants() {
    let adapter = conforme::adapters::claude::ClaudeAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".claude/rules")).unwrap();
    fs::write(
        dir.path().join(".claude/rules/web.md"),
        "---\npaths: \"src/**/*.ts, src/**/*.tsx\"\n---\nUse strict mode.\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join(".claude/agents/review")).unwrap();
    fs::write(
        dir.path().join(".claude/agents/review/security.md"),
        "---\nname: security\ndescription: Security review\n---\nCheck auth.\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join(".claude/commands")).unwrap();
    fs::write(
        dir.path().join(".claude/commands/release.md"),
        "---\ndescription: Cut a release\ndisable-model-invocation: yes\n---\nRelease.\n",
    )
    .unwrap();

    let config = adapter.read(dir.path()).unwrap();

    assert_eq!(
        config.rules[0].activation,
        ActivationMode::GlobMatch(vec!["src/**/*.ts".into(), "src/**/*.tsx".into()])
    );
    assert_eq!(config.agents.len(), 1, "nested agent dropped");
    assert_eq!(config.agents[0].name, "security");
    assert!(config.skills[0].manual_invocation, "`yes` not read as true");
}

/// Copilot discovers `*.instructions.md` in sub-directories, and an agent
/// file without a `name` is named after the file without `.agent.md`.
#[test]
fn test_copilot_reads_nested_instructions_and_agent_names() {
    let adapter = conforme::adapters::copilot::CopilotAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "copilot");
    fs::create_dir_all(dir.path().join(".github/instructions/frontend")).unwrap();
    fs::write(
        dir.path()
            .join(".github/instructions/frontend/react.instructions.md"),
        "---\napplyTo: \"**/*.tsx\"\n---\nUse hooks.\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join(".github/agents")).unwrap();
    fs::write(
        dir.path().join(".github/agents/planner.agent.md"),
        "---\ndescription: Plan work\n---\nPlan.\n",
    )
    .unwrap();

    let config = adapter.read(dir.path()).unwrap();

    assert_eq!(config.rules.len(), 1, "nested instructions dropped");
    assert_eq!(config.rules[0].name, "react");
    assert_eq!(config.agents[0].name, "planner");
}

/// OpenCode scans `.opencode/agents/` recursively.
#[test]
fn test_opencode_reads_nested_agents() {
    let adapter = conforme::adapters::opencode::OpenCodeAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".opencode/agents/review")).unwrap();
    fs::write(
        dir.path().join(".opencode/agents/review/security.md"),
        "---\ndescription: Security review\nmode: subagent\n---\nCheck auth.\n",
    )
    .unwrap();

    let config = adapter.read(dir.path()).unwrap();

    assert_eq!(config.agents.len(), 1, "nested agent dropped");
    assert_eq!(config.agents[0].name, "security");
}

/// The DeepSeek Harness accepts flat `.dsh/skills/<name>.md` skills next to
/// `<name>/SKILL.md` bundles; a bundle wins over a flat file of the same name.
#[test]
fn test_deepseek_reads_flat_skills() {
    let adapter = conforme::adapters::deepseek::DeepSeekAdapter;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".dsh/skills/deploy")).unwrap();
    fs::write(
        dir.path().join(".dsh/skills/deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Deploy (bundle)\n---\nDeploy.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".dsh/skills/deploy.md"),
        "---\nname: deploy\ndescription: Deploy (flat)\n---\nOld.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".dsh/skills/release.md"),
        "---\nname: release\ndescription: Cut a release\n---\nRelease.\n",
    )
    .unwrap();

    let config = adapter.read(dir.path()).unwrap();

    let names: Vec<_> = config.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["deploy", "release"]);
    assert_eq!(config.skills[0].description, "Deploy (bundle)");
}

#[test]
fn manual_skill_invocation_survives_every_skill_adapter() {
    for adapter in conforme::adapters::all_adapters() {
        if !adapter.capabilities().skills {
            continue;
        }
        let dir = TempDir::new().unwrap();
        let config = NormalizedConfig {
            skills: vec![NormalizedSkill {
                name: "manual-review".into(),
                description: "Review on explicit request".into(),
                content: "Review the selected change.".into(),
                manual_invocation: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        adapter.write(dir.path(), &config).unwrap();
        let read = adapter.read(dir.path()).unwrap();
        assert_eq!(read.skills.len(), 1, "{}", adapter.id());
        assert!(read.skills[0].manual_invocation, "{}", adapter.id());
        let files = adapter.generate(dir.path(), &config).unwrap();
        // The Codex policy sidecar is only written where Codex reads skills.
        let policy = files
            .iter()
            .find(|(p, _)| p.ends_with("agents/openai.yaml"));
        let in_codex_root = files
            .iter()
            .any(|(p, _)| p.starts_with(dir.path().join(".agents/skills")));
        assert_eq!(policy.is_some(), in_codex_root, "{}", adapter.id());
        if let Some(policy) = policy {
            let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&policy.1).unwrap();
            assert_eq!(
                value["policy"]["allow_implicit_invocation"].as_bool(),
                Some(false)
            );
        }
        // Synchronizing twice must not re-enable the skill or change the bundle.
        assert!(
            adapter
                .write(dir.path(), &read)
                .unwrap()
                .files_written
                .is_empty(),
            "{}",
            adapter.id()
        );
    }
}

#[test]
fn manual_skill_markdown_roundtrip() {
    let input = "## Skill: review\n<!-- description: Review a change -->\n<!-- invocation: manual -->\n\nReview.\n";
    let config = conforme::markdown::parse_agents_md(input).unwrap();
    assert!(config.skills[0].manual_invocation);
    let text = conforme::markdown::export_as_agents_md(&config);
    assert!(conforme::markdown::parse_agents_md(&text).unwrap().skills[0].manual_invocation);
}

#[test]
fn codex_policy_import_and_explicit_reenable_preserve_other_fields() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join(".agents/skills/review");
    fs::create_dir_all(skill_dir.join("agents")).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: review\ndescription: Review\n---\nReview.\n",
    )
    .unwrap();
    let policy = skill_dir.join("agents/openai.yaml");
    fs::write(
        &policy,
        "interface:\n  display_name: Review\npolicy:\n  allow_implicit_invocation: false\n",
    )
    .unwrap();
    let adapter = conforme::adapters::codex::CodexAdapter;
    let mut config = adapter.read(dir.path()).unwrap();
    assert!(config.skills[0].manual_invocation);
    config.skills[0].manual_invocation = false;
    adapter.write(dir.path(), &config).unwrap();
    let value: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&fs::read_to_string(policy).unwrap()).unwrap();
    assert_eq!(
        value["policy"]["allow_implicit_invocation"].as_bool(),
        Some(true)
    );
    assert_eq!(value["interface"]["display_name"].as_str(), Some("Review"));
    assert!(!adapter.read(dir.path()).unwrap().skills[0].manual_invocation);
}

/// No adapter may drop an empty or whitespace-only file into a project: an
/// empty config yields no files at all, and a full config yields no blank file.
#[test]
fn test_no_adapter_writes_blank_files() {
    for adapter in conforme::adapters::all_adapters() {
        let dir = TempDir::new().unwrap();

        let files = adapter
            .generate(dir.path(), &NormalizedConfig::default())
            .unwrap();
        let paths: Vec<_> = files.iter().map(|(p, _)| p.display().to_string()).collect();
        assert!(
            files.is_empty(),
            "{} generated files for an empty config: {paths:?}",
            adapter.id()
        );

        let files = adapter.generate(dir.path(), &rich_config()).unwrap();
        for (path, content) in &files {
            assert!(
                !content.trim().is_empty(),
                "{} wrote a blank file at {}",
                adapter.id(),
                path.display()
            );
        }
    }
}

// ===== Layouts the tools read that conforme does not write =====

fn write_skill(dir: &std::path::Path, name: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {name} skill\n---\nDo it.\n"),
    )
    .unwrap();
}

#[test]
fn test_nested_skills_are_read_where_the_tool_searches_recursively() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_skill(&root.join(".agents/skills/team/release"), "release");
    write_skill(&root.join(".cursor/skills/group/lint"), "lint");
    write_skill(&root.join(".opencode/skills/a/b/fmt"), "fmt");
    write_skill(&root.join(".opencode/skill/legacy"), "legacy");
    fs::create_dir_all(root.join(".amp")).unwrap();

    let names = |config: NormalizedConfig| -> Vec<String> {
        config.skills.into_iter().map(|s| s.name).collect()
    };
    assert_eq!(
        names(conforme::adapters::codex::CodexAdapter.read(root).unwrap()),
        ["release"]
    );
    assert_eq!(
        names(conforme::adapters::amp::AmpAdapter.read(root).unwrap()),
        ["release"]
    );
    assert_eq!(
        names(
            conforme::adapters::cursor::CursorAdapter
                .read(root)
                .unwrap()
        ),
        ["lint"]
    );
    assert_eq!(
        names(
            conforme::adapters::opencode::OpenCodeAdapter
                .read(root)
                .unwrap()
        ),
        ["fmt", "legacy"]
    );
}

#[test]
fn test_codex_source_is_the_shared_agents_md_not_the_personal_override() {
    // `AGENTS.override.md` is a local override: it must not become the
    // config of every other tool.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "Shared.\n").unwrap();
    fs::write(dir.path().join("AGENTS.override.md"), "Local override.\n").unwrap();
    let config = conforme::adapters::codex::CodexAdapter
        .read(dir.path())
        .unwrap();
    assert_eq!(config.instructions, "Shared.");
}

#[test]
fn test_native_agents_md_source_keeps_its_sections_and_is_never_rewritten() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".codex")).unwrap();
    fs::create_dir_all(root.join(".cursor")).unwrap();
    let agents_md = "# Team\nShared.\n\n## Rule: TS\n<!-- activation: glob **/*.ts -->\nUse TS.\n\n## Agent: reviewer\n<!-- description: Review -->\nReview.\n\n## MCP: fs\n<!-- command: npx -->\n";
    fs::write(root.join("AGENTS.md"), agents_md).unwrap();

    let config = conforme::adapters::codex::CodexAdapter.read(root).unwrap();
    assert_eq!(config.instructions, "# Team\nShared.");
    assert_eq!(config.rules.len(), 1);
    assert_eq!(config.agents[0].name, "reviewer");
    assert_eq!(mcp_names(&config), vec!["fs"]);

    assert_cmd::Command::cargo_bin("conforme")
        .unwrap()
        .args(["-C", root.to_str().unwrap(), "sync", "--from", "codex"])
        .assert()
        .success();
    // AGENTS.md is Codex's own config: sync leaves it exactly as written.
    assert_eq!(
        fs::read_to_string(root.join("AGENTS.md")).unwrap(),
        agents_md
    );
    assert!(root.join(".cursor/agents/reviewer.md").exists());
}

#[test]
fn test_amp_uses_settings_jsonc_when_only_that_exists() {
    let adapter = conforme::adapters::amp::AmpAdapter;
    let dir = TempDir::new().unwrap();
    let jsonc = dir.path().join(".amp/settings.jsonc");
    fs::create_dir_all(jsonc.parent().unwrap()).unwrap();
    fs::write(
        &jsonc,
        "{\n  // workspace\n  \"amp.mcpServers\": {\"old\": {\"command\": \"x\"}},\n}\n",
    )
    .unwrap();

    assert_eq!(mcp_names(&adapter.read(dir.path()).unwrap()), vec!["old"]);

    let files = adapter.generate(dir.path(), &rich_config()).unwrap();
    let settings: Vec<_> = files
        .iter()
        .filter(|(p, _)| p.starts_with(dir.path().join(".amp")))
        .collect();
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0].0, jsonc);
    assert!(settings[0].1.contains("// workspace"));
    assert!(adapter.is_shared_file(&jsonc));
    assert!(!dir.path().join(".amp/settings.json").exists());
}

#[test]
fn test_gemini_skips_underscore_agent_drafts() {
    let dir = TempDir::new().unwrap();
    let agents = dir.path().join(".gemini/agents");
    fs::create_dir_all(&agents).unwrap();
    for name in ["reviewer", "_draft"] {
        fs::write(
            agents.join(format!("{name}.md")),
            format!("---\nname: {name}\ndescription: d\n---\nBody.\n"),
        )
        .unwrap();
    }
    let config = conforme::adapters::gemini::GeminiAdapter
        .read(dir.path())
        .unwrap();
    let names: Vec<_> = config.agents.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["reviewer"]);
}

#[test]
fn test_copilot_instructions_without_apply_to() {
    let dir = TempDir::new().unwrap();
    let instructions = dir.path().join(".github/instructions");
    fs::create_dir_all(&instructions).unwrap();
    fs::write(
        instructions.join("docs.instructions.md"),
        "---\ndescription: Writing docs\n---\nDocs.\n",
    )
    .unwrap();
    fs::write(instructions.join("manual.instructions.md"), "Manual.\n").unwrap();
    fs::write(
        instructions.join("ts.instructions.md"),
        "---\napplyTo: \"src/*.{ts,tsx},lib/**\"\n---\nTS.\n",
    )
    .unwrap();

    let config = conforme::adapters::copilot::CopilotAdapter
        .read(dir.path())
        .unwrap();
    let activation = |name: &str| {
        config
            .rules
            .iter()
            .find(|r| r.name == name)
            .unwrap()
            .activation
            .clone()
    };
    assert_eq!(
        activation("docs"),
        ActivationMode::AgentDecision {
            description: "Writing docs".to_string()
        }
    );
    assert_eq!(activation("manual"), ActivationMode::Manual);
    assert_eq!(
        activation("ts"),
        ActivationMode::GlobMatch(vec!["src/*.{ts,tsx}".into(), "lib/**".into()])
    );
}

#[test]
fn test_comma_joined_globs_expand_brace_groups() {
    let config = NormalizedConfig {
        rules: vec![NormalizedRule {
            name: "ts".to_string(),
            content: "TS.".to_string(),
            activation: ActivationMode::GlobMatch(vec!["src/*.{ts,tsx}".to_string()]),
        }],
        ..Default::default()
    };
    let root = std::path::Path::new("/r");
    let copilot = conforme::adapters::copilot::CopilotAdapter
        .generate(root, &config)
        .unwrap();
    assert!(
        copilot[0].1.contains("applyTo: src/*.ts,src/*.tsx\n"),
        "{}",
        copilot[0].1
    );
    let cursor = conforme::adapters::cursor::CursorAdapter
        .generate(root, &config)
        .unwrap();
    assert!(
        cursor[0].1.contains("globs: src/*.ts, src/*.tsx\n"),
        "{}",
        cursor[0].1
    );
    // Claude Code keeps the brace form it recommends.
    let claude = conforme::adapters::claude::ClaudeAdapter
        .generate(root, &config)
        .unwrap();
    assert!(claude[0].1.contains("src/*.{ts,tsx}"), "{}", claude[0].1);
}

#[test]
fn test_agent_decision_rules_always_carry_a_description() {
    let config = NormalizedConfig {
        rules: vec![NormalizedRule {
            name: "smart".to_string(),
            content: "Decide.".to_string(),
            activation: ActivationMode::AgentDecision {
                description: String::new(),
            },
        }],
        ..Default::default()
    };
    let root = std::path::Path::new("/r");
    // Kiro requires a description on `auto` steering; without one Cursor and
    // Devin would treat the rule as manual.
    for files in [
        conforme::adapters::kiro::KiroAdapter
            .generate(root, &config)
            .unwrap(),
        conforme::adapters::cursor::CursorAdapter
            .generate(root, &config)
            .unwrap(),
        conforme::adapters::devin::DevinAdapter
            .generate(root, &config)
            .unwrap(),
    ] {
        assert!(
            files[0].1.contains("description: smart\n"),
            "{}",
            files[0].1
        );
    }
}
