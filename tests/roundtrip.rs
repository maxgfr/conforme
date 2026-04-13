//! Round-trip tests: write config → read back → compare.
//! Validates that adapters with read() support can faithfully
//! round-trip a NormalizedConfig through write → read.

use conforme::adapters::AiToolAdapter;
use conforme::config::{ActivationMode, NormalizedConfig, NormalizedRule};
use std::fs;
use tempfile::TempDir;

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
