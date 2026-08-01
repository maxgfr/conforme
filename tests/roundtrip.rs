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
        "windsurf" => fs::create_dir_all(dir.path().join(".windsurf")).unwrap(),
        "copilot" => {
            fs::create_dir_all(dir.path().join(".github")).unwrap();
            fs::write(
                dir.path().join(".github").join("copilot-instructions.md"),
                "",
            )
            .unwrap();
        }
        "continue" => fs::create_dir_all(dir.path().join(".continue")).unwrap(),
        "kiro" => fs::create_dir_all(dir.path().join(".kiro")).unwrap(),
        "roocode" => fs::create_dir_all(dir.path().join(".roo")).unwrap(),
        "amazonq" => fs::create_dir_all(dir.path().join(".amazonq")).unwrap(),
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
fn test_roundtrip_windsurf() {
    let adapter = conforme::adapters::windsurf::WindsurfAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "windsurf");

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
fn test_roundtrip_continuedev() {
    let adapter = conforme::adapters::continuedev::ContinueDevAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "continue");

    let config = roundtrip_config();
    adapter.write(dir.path(), &config).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.instructions, config.instructions);
    assert_eq!(read_config.rules.len(), 2);
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
fn test_roundtrip_roocode() {
    let adapter = conforme::adapters::roocode::RooCodeAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "roocode");

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
fn test_roundtrip_amazonq() {
    let adapter = conforme::adapters::amazonq::AmazonQAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "amazonq");

    let config = NormalizedConfig {
        instructions: "Follow AWS best practices.".to_string(),
        rules: vec![NormalizedRule {
            name: "Security".to_string(),
            content: "Use IAM roles.".to_string(),
            activation: ActivationMode::Always,
        }],
        ..Default::default()
    };
    adapter.write(dir.path(), &config).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.instructions, "Follow AWS best practices.");
    assert_eq!(read_config.rules.len(), 1);
    assert!(read_config.rules[0].content.contains("Use IAM roles."));
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
    assert_eq!(read_config.agents[0].model.as_deref(), Some("sonnet"));
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
}

#[test]
fn test_roundtrip_amazonq_agents_mcp() {
    let adapter = conforme::adapters::amazonq::AmazonQAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "amazonq");

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.agents.len(), 1);
    assert_eq!(read_config.agents[0].name, "reviewer");
    assert_eq!(read_config.agents[0].content, "Look for bugs.");
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
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
fn test_roundtrip_windsurf_skills_mcp() {
    let adapter = conforme::adapters::windsurf::WindsurfAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "windsurf");

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    assert_eq!(read_config.skills.len(), 1);
    assert_eq!(read_config.skills[0].name, "deploy");
    // Windsurf writes `serverUrl` and no `type` — the parser must still
    // recognise the remote server as HTTP.
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
}

#[test]
fn test_roundtrip_continuedev_mcp() {
    let adapter = conforme::adapters::continuedev::ContinueDevAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "continue");

    adapter.write(dir.path(), &rich_config()).unwrap();
    let read_config = adapter.read(dir.path()).unwrap();

    // Continue emits `type: streamable-http` — it must parse back as HTTP.
    assert_eq!(mcp_names(&read_config), vec!["api", "fs"]);
    assert_eq!(
        find_http_url(&read_config, "api").as_deref(),
        Some("https://example.com/mcp")
    );
}

#[test]
fn test_roundtrip_roocode_skills_mcp() {
    let adapter = conforme::adapters::roocode::RooCodeAdapter;
    let dir = TempDir::new().unwrap();
    setup_tool(&dir, "roocode");

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
