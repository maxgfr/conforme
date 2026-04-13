//! Error and edge case tests.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn conforme() -> Command {
    Command::cargo_bin("conforme").unwrap()
}

fn create_project_with_tools(agents_md: &str, tools: &[&str]) -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("AGENTS.md"), agents_md).unwrap();
    for tool in tools {
        match *tool {
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
            "kiro" => fs::create_dir_all(dir.path().join(".kiro")).unwrap(),
            "continue" => fs::create_dir_all(dir.path().join(".continue")).unwrap(),
            "roocode" => fs::create_dir_all(dir.path().join(".roo")).unwrap(),
            "amazonq" => fs::create_dir_all(dir.path().join(".amazonq")).unwrap(),
            "gemini" => fs::create_dir_all(dir.path().join(".gemini")).unwrap(),
            "opencode" => fs::create_dir_all(dir.path().join(".opencode")).unwrap(),
            _ => {}
        }
    }
    dir
}

// ===== --only with invalid tool name =====

#[test]
fn test_only_unknown_tool_warns() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "sync",
            "--only",
            "nonexistent",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Unknown tool 'nonexistent'"));
}

// ===== AGENTS.md with only MCP sections (no rules) =====

#[test]
fn test_sync_mcp_only_config() {
    let agents_md = r#"# Instructions

## MCP: filesystem
<!-- command: npx -->
<!-- args: -y, @modelcontextprotocol/server-filesystem -->
"#;
    let dir = create_project_with_tools(agents_md, &["claude", "cursor"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Claude should have .mcp.json
    assert!(dir.path().join(".mcp.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".mcp.json")).unwrap();
    assert!(mcp.contains("filesystem"));
    assert!(mcp.contains("npx"));

    // Cursor should have .cursor/mcp.json
    assert!(dir.path().join(".cursor/mcp.json").exists());
}

// ===== AGENTS.md with only skills (no rules) =====

#[test]
fn test_sync_skills_only_config() {
    let agents_md = r#"# Instructions

## Skill: deploy
<!-- description: Deploy the app -->
<!-- tools: Bash -->

Run npm run deploy.
"#;
    let dir = create_project_with_tools(agents_md, &["claude", "copilot"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".claude/skills/deploy/SKILL.md").exists());
    assert!(dir.path().join(".github/prompts/deploy.prompt.md").exists());
}

// ===== Empty instructions, no rules =====

#[test]
fn test_sync_empty_config() {
    let agents_md = "# Instructions\n";
    let dir = create_project_with_tools(agents_md, &["cursor", "windsurf"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // With empty instructions, cursor still generates general.mdc
    // because the instructions string is not empty (it gets "# Instructions" header text)
    // This test verifies sync succeeds without errors on minimal input
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .success();
}

// ===== MCP sync to new adapters (Windsurf, Continue, AmazonQ) =====

#[test]
fn test_sync_mcp_to_windsurf() {
    let agents_md = r#"# Instructions
Be helpful.

## MCP: test-server
<!-- command: npx -->
<!-- args: -y, @test/server -->
"#;
    let dir = create_project_with_tools(agents_md, &["windsurf"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".windsurf/mcp.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".windsurf/mcp.json")).unwrap();
    assert!(mcp.contains("mcpServers"));
    assert!(mcp.contains("test-server"));
}

#[test]
fn test_sync_mcp_to_continue() {
    let agents_md = r#"# Instructions
Be helpful.

## MCP: test-server
<!-- command: node -->
<!-- args: server.js -->
"#;
    let dir = create_project_with_tools(agents_md, &["continue"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".continue/mcp.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".continue/mcp.json")).unwrap();
    assert!(mcp.contains("mcpServers"));
    assert!(mcp.contains("test-server"));
}

#[test]
fn test_sync_mcp_to_amazonq() {
    let agents_md = r#"# Instructions
Be helpful.

## MCP: test-server
<!-- command: npx -->
<!-- args: -y, @test/server -->
"#;
    let dir = create_project_with_tools(agents_md, &["amazonq"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".amazonq/mcp.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".amazonq/mcp.json")).unwrap();
    assert!(mcp.contains("mcpServers"));
    assert!(mcp.contains("test-server"));
}

// ===== Agents sync to Cursor and Kiro =====

#[test]
fn test_sync_agents_to_cursor() {
    let agents_md = r#"# Instructions
Use TypeScript.

## Agent: reviewer
<!-- description: Code review agent -->
<!-- model: gpt-4o -->
<!-- tools: codebase, terminal -->

Review all changes for bugs.
"#;
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".cursor/agents/reviewer.mdc").exists());
    let agent = fs::read_to_string(dir.path().join(".cursor/agents/reviewer.mdc")).unwrap();
    assert!(agent.contains("name: reviewer"));
    assert!(agent.contains("description: Code review agent"));
    assert!(agent.contains("model: gpt-4o"));
    assert!(agent.contains("Review all changes for bugs."));
}

#[test]
fn test_sync_agents_to_kiro() {
    let agents_md = r#"# Instructions
Use TypeScript.

## Agent: reviewer
<!-- description: Code review agent -->
<!-- model: gpt-4o -->
<!-- tools: codebase -->

Review all changes for bugs.
"#;
    let dir = create_project_with_tools(agents_md, &["kiro"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".kiro/agents/reviewer.md").exists());
    let agent = fs::read_to_string(dir.path().join(".kiro/agents/reviewer.md")).unwrap();
    assert!(agent.contains("description: Code review agent"));
    assert!(agent.contains("model: gpt-4o"));
    assert!(agent.contains("Review all changes for bugs."));
}

// ===== All 4 activation modes on a complex adapter =====

#[test]
fn test_sync_all_activation_modes() {
    let agents_md = r#"# Instructions
General rules.

## Rule: Always On
<!-- activation: always -->

Keep it simple.

## Rule: API Rules
<!-- activation: glob src/api/** -->

Follow REST conventions.

## Rule: Smart Rule
<!-- activation: agent-decision -->
<!-- description: Use when discussing architecture -->

Think before acting.

## Rule: Manual Only
<!-- activation: manual -->

Only when explicitly asked.
"#;
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Always rule
    let always = dir.path().join(".cursor/rules/always-on.mdc");
    assert!(always.exists());
    let content = fs::read_to_string(&always).unwrap();
    assert!(content.contains("alwaysApply: true"));

    // Glob rule
    let api = dir.path().join(".cursor/rules/api-rules.mdc");
    assert!(api.exists());
    let content = fs::read_to_string(&api).unwrap();
    assert!(content.contains("globs:"));
    assert!(content.contains("src/api/**"));

    // Agent decision rule
    let smart = dir.path().join(".cursor/rules/smart-rule.mdc");
    assert!(smart.exists());
    let content = fs::read_to_string(&smart).unwrap();
    assert!(content.contains("description:"));
    assert!(content.contains("alwaysApply: false"));

    // Manual rule
    let manual = dir.path().join(".cursor/rules/manual-only.mdc");
    assert!(manual.exists());
    let content = fs::read_to_string(&manual).unwrap();
    assert!(content.contains("alwaysApply: false"));
    assert!(!content.contains("description:"));
    assert!(!content.contains("globs:"));
}

// ===== Check detects MCP changes =====

#[test]
fn test_check_after_mcp_change() {
    let agents_md = r#"# Instructions
Hello.

## MCP: test-server
<!-- command: npx -->
<!-- args: -y, @test/server -->
"#;
    let dir = create_project_with_tools(agents_md, &["claude"]);

    // Sync first
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Verify in sync
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .success();

    // Modify the MCP file
    fs::write(dir.path().join(".mcp.json"), "{}").unwrap();

    // Check should now fail
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("out of sync"));
}

// ===== MCP sync to Gemini, OpenCode, Zed =====

#[test]
fn test_sync_mcp_to_gemini() {
    let agents_md = r#"# Instructions
Be helpful.

## MCP: test-server
<!-- command: npx -->
<!-- args: -y, @test/server -->
"#;
    let dir = create_project_with_tools(agents_md, &["gemini"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".gemini/settings.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".gemini/settings.json")).unwrap();
    assert!(mcp.contains("mcpServers"));
    assert!(mcp.contains("test-server"));
}

#[test]
fn test_sync_mcp_to_opencode() {
    let agents_md = r#"# Instructions
Be helpful.

## MCP: test-server
<!-- command: npx -->
<!-- args: -y, @test/server -->
"#;
    let dir = create_project_with_tools(agents_md, &["opencode"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".opencode/mcp.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".opencode/mcp.json")).unwrap();
    assert!(mcp.contains("\"mcp\""));
    assert!(mcp.contains("\"type\": \"local\""));
    assert!(mcp.contains("test-server"));
}

#[test]
fn test_sync_agents_to_opencode() {
    let agents_md = r#"# Instructions
Be helpful.

## Agent: reviewer
<!-- description: Code review agent -->
<!-- model: gpt-4o -->

Review all changes for bugs.
"#;
    let dir = create_project_with_tools(agents_md, &["opencode"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".opencode/agents.json").exists());
    let agents = fs::read_to_string(dir.path().join(".opencode/agents.json")).unwrap();
    assert!(agents.contains("\"agent\""));
    assert!(agents.contains("\"reviewer\""));
    assert!(agents.contains("\"mode\": \"subagent\""));
    assert!(agents.contains("gpt-4o"));
}

#[test]
fn test_sync_mcp_to_zed() {
    let agents_md = r#"# Instructions
Be helpful.

## MCP: test-server
<!-- command: npx -->
<!-- args: -y, @test/server -->
"#;
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("AGENTS.md"), agents_md).unwrap();
    // Zed detection requires .rules file
    fs::write(dir.path().join(".rules"), "").unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".zed/settings.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".zed/settings.json")).unwrap();
    assert!(mcp.contains("context_servers"));
    assert!(mcp.contains("test-server"));
    assert!(!mcp.contains("mcpServers"));
}

// ===== Agents sync to Gemini =====

#[test]
fn test_sync_agents_to_gemini() {
    let agents_md = r#"# Instructions
Be helpful.

## Agent: reviewer
<!-- description: Code review agent -->
<!-- model: gemini-3-flash -->
<!-- tools: read_file, grep_search -->

Review all changes for bugs.
"#;
    let dir = create_project_with_tools(agents_md, &["gemini"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".gemini/agents/reviewer.md").exists());
    let agent = fs::read_to_string(dir.path().join(".gemini/agents/reviewer.md")).unwrap();
    assert!(agent.contains("kind: local"));
    assert!(agent.contains("description: Code review agent"));
    assert!(agent.contains("model: gemini-3-flash"));
    assert!(agent.contains("- read_file"));
    assert!(agent.contains("Review all changes for bugs."));
}

// ===== Kiro full sync (skills + MCP + rules) =====

#[test]
fn test_sync_kiro_skills_and_mcp() {
    let agents_md = r#"# Instructions
Follow AWS patterns.

## Rule: Lambda
<!-- activation: glob **/*.lambda.ts -->

Use handler pattern.

## Skill: deploy
<!-- description: Deploy the app -->
<!-- tools: Bash -->

Run cdk deploy.

## MCP: filesystem
<!-- command: npx -->
<!-- args: -y, @mcp/server-filesystem -->
"#;
    let dir = create_project_with_tools(agents_md, &["kiro"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Rules
    assert!(dir.path().join(".kiro/steering/general.md").exists());
    assert!(dir.path().join(".kiro/steering/lambda.md").exists());

    // Skills
    assert!(dir.path().join(".kiro/skills/deploy/SKILL.md").exists());
    let skill = fs::read_to_string(dir.path().join(".kiro/skills/deploy/SKILL.md")).unwrap();
    assert!(skill.contains("name: deploy"));
    assert!(skill.contains("Run cdk deploy."));

    // MCP
    assert!(dir.path().join(".kiro/settings/mcp.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".kiro/settings/mcp.json")).unwrap();
    assert!(mcp.contains("filesystem"));
}
