use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn conforme() -> Command {
    Command::cargo_bin("conforme").unwrap()
}

fn create_project(agents_md: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("AGENTS.md"), agents_md).unwrap();
    dir
}

fn create_project_with_tools(agents_md: &str, tools: &[&str]) -> TempDir {
    let dir = create_project(agents_md);
    for tool in tools {
        match *tool {
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
            "codex" => fs::create_dir_all(dir.path().join(".codex")).unwrap(),
            "opencode" => fs::create_dir_all(dir.path().join(".opencode")).unwrap(),
            "zoocode" => fs::create_dir_all(dir.path().join(".roo")).unwrap(),
            "gemini" => fs::create_dir_all(dir.path().join(".gemini")).unwrap(),
            "zed" => fs::write(dir.path().join(".rules"), "").unwrap(),
            "kiro" => fs::create_dir_all(dir.path().join(".kiro")).unwrap(),
            "vibe" => fs::create_dir_all(dir.path().join(".vibe")).unwrap(),
            "kilo" => fs::create_dir_all(dir.path().join(".kilo")).unwrap(),
            "deepseek" => fs::create_dir_all(dir.path().join(".dsh")).unwrap(),
            _ => {}
        }
    }
    dir
}

// ===== Version / Help =====

#[test]
fn test_version_flag() {
    conforme()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("conforme"));
}

#[test]
fn test_help_flag() {
    conforme()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Conforme synchronizes configuration",
        ));
}

// ===== Init =====

#[test]
fn test_init_creates_agents_md() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".cursor")).unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created AGENTS.md template"));

    assert!(dir.path().join("AGENTS.md").exists());
}

#[test]
fn test_init_does_not_overwrite_without_force() {
    let dir = create_project("# Existing content\n");

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));

    let content = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(content.contains("Existing content"));
}

#[test]
fn test_init_overwrites_with_force() {
    let dir = create_project("# Old content\n");

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "init", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created AGENTS.md template"));
}

// ===== Sync =====

#[test]
fn test_sync_requires_source() {
    let dir = TempDir::new().unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No source configured"));
}

#[test]
fn test_sync_creates_cursor_rules() {
    let agents_md = r#"# Instructions
Use TypeScript.

## Rule: TypeScript
<!-- activation: glob **/*.ts -->

Use strict mode.
"#;
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let general = dir.path().join(".cursor/rules/general.mdc");
    assert!(general.exists());
    let content = fs::read_to_string(&general).unwrap();
    assert!(content.contains("alwaysApply: true"));
    assert!(content.contains("Use TypeScript."));

    let ts_rule = dir.path().join(".cursor/rules/typescript.mdc");
    assert!(ts_rule.exists());
    let content = fs::read_to_string(&ts_rule).unwrap();
    assert!(content.contains("globs:"));
    assert!(content.contains("Use strict mode."));
}

#[test]
fn test_sync_creates_claude_config() {
    let agents_md = r#"# Instructions
General rules.

## Rule: Always On
<!-- activation: always -->

Keep it simple.

## Rule: API Rules
<!-- activation: glob src/api/** -->

Follow REST conventions.
"#;
    let dir = create_project_with_tools(agents_md, &["claude"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let claude_md = dir.path().join("CLAUDE.md");
    assert!(claude_md.exists());
    let content = fs::read_to_string(&claude_md).unwrap();
    assert!(content.contains("General rules."));
    assert!(content.contains("Keep it simple."));

    let api_rule = dir.path().join(".claude/rules/api-rules.md");
    assert!(api_rule.exists());
    let content = fs::read_to_string(&api_rule).unwrap();
    assert!(content.contains("paths:"));
    assert!(content.contains("src/api/**"));
}

#[test]
fn test_sync_creates_devin_config() {
    let agents_md = r#"# Instructions
Be helpful.

## Rule: Testing
<!-- activation: glob **/*.test.ts -->

Write thorough tests.
"#;
    let dir = create_project_with_tools(agents_md, &["devin"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let general = dir.path().join(".devin/rules/general.md");
    assert!(general.exists());
    let content = fs::read_to_string(&general).unwrap();
    assert!(content.contains("trigger: always_on"));

    let testing = dir.path().join(".devin/rules/testing.md");
    assert!(testing.exists());
    let content = fs::read_to_string(&testing).unwrap();
    assert!(content.contains("trigger: glob"));
    assert!(content.contains("**/*.test.ts"));
}

#[test]
fn test_sync_creates_copilot_config() {
    let agents_md = r#"# Instructions
Project guidelines.

## Rule: Python Rules
<!-- activation: glob **/*.py -->

Use type hints.
"#;
    let dir = create_project_with_tools(agents_md, &["copilot"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let instructions = dir.path().join(".github/copilot-instructions.md");
    assert!(instructions.exists());
    let content = fs::read_to_string(&instructions).unwrap();
    assert!(content.contains("Project guidelines."));

    let python_rule = dir
        .path()
        .join(".github/instructions/python-rules.instructions.md");
    assert!(python_rule.exists());
    let content = fs::read_to_string(&python_rule).unwrap();
    assert!(content.contains("applyTo:"));
    assert!(content.contains("Use type hints."));
}

#[test]
fn test_sync_dry_run_no_changes() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run"));

    // Should NOT have created files
    assert!(!dir.path().join(".cursor/rules/general.mdc").exists());
}

#[test]
fn test_sync_only_flag() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor", "devin"]);

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "sync",
            "--only",
            "cursor",
        ])
        .assert()
        .success();

    // Cursor should have files
    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    // Devin should NOT
    assert!(!dir.path().join(".devin/rules/general.md").exists());
}

#[test]
fn test_sync_idempotent() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    // First sync
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success()
        .stdout(predicate::str::contains("wrote"));

    // Second sync — should be unchanged
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already in sync"));
}

// ===== Check =====

#[test]
fn test_check_requires_source() {
    let dir = TempDir::new().unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No source configured"));
}

#[test]
fn test_check_in_sync_exits_0() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    // Sync first
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Check should pass
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("All configs in sync"));
}

#[test]
fn test_check_out_of_sync_exits_1() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    // Sync first
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Modify a synced file
    fs::write(
        dir.path().join(".cursor/rules/general.mdc"),
        "modified content",
    )
    .unwrap();

    // Check should fail
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("out of sync"));
}

// ===== Status =====

#[test]
fn test_status_shows_tools() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "status"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("AGENTS.md")
                .and(predicate::str::contains("Cursor"))
                .and(predicate::str::contains("Claude Code")),
        );
}

#[test]
fn test_status_no_agents_md() {
    let dir = TempDir::new().unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("conforme init"));
}

// ===== Multi-tool sync =====

#[test]
fn test_sync_all_tools() {
    let agents_md = r#"# Instructions
Global rules.

## Rule: Frontend
<!-- activation: glob src/components/**/*.tsx -->

Use React best practices.
"#;
    let dir = create_project_with_tools(agents_md, &["cursor", "claude", "devin", "copilot"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Verify all tools got files
    assert!(dir.path().join("CLAUDE.md").exists());
    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    assert!(dir.path().join(".devin/rules/general.md").exists());
    assert!(dir.path().join(".github/copilot-instructions.md").exists());
}

// ===== New adapters =====

#[test]
fn test_sync_creates_gemini_config() {
    let agents_md = "# Instructions\nBe helpful.\n";
    let dir = create_project_with_tools(agents_md, &["gemini"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let gemini = dir.path().join("GEMINI.md");
    assert!(gemini.exists());
    let content = fs::read_to_string(&gemini).unwrap();
    assert!(content.contains("Be helpful."));
}

#[test]
fn test_sync_creates_zoocode_config() {
    let agents_md = r#"# Instructions
General rules.

## Rule: Testing
<!-- activation: always -->

Write tests.
"#;
    let dir = create_project_with_tools(agents_md, &["zoocode"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".roo/rules/00-general.md").exists());
    assert!(dir.path().join(".roo/rules/01-testing.md").exists());
}

#[test]
fn test_sync_creates_zed_config() {
    let agents_md = "# Instructions\nUse Rust.\n";
    let dir = create_project_with_tools(agents_md, &["zed"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let rules = dir.path().join(".rules");
    assert!(rules.exists());
    let content = fs::read_to_string(&rules).unwrap();
    assert!(content.contains("Use Rust."));
}

#[test]
fn test_sync_deepseek_skills() {
    let agents_md = "# Instructions\nGlobal.\n\n## Skill: Deploy App\n<!-- description: Deploy it -->\nRun deploy.\n";
    let dir = create_project_with_tools(agents_md, &["deepseek"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let skill = dir.path().join(".dsh/skills/deploy-app/SKILL.md");
    assert!(skill.exists());
    let content = fs::read_to_string(&skill).unwrap();
    assert!(content.contains("name: deploy-app"));
    assert!(content.contains("description: Deploy it"));

    // AGENTS.md is read natively — the harness needs no generated copy of it.
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .success();
}

#[test]
fn test_sync_every_tool() {
    let agents_md = "# Instructions\nGlobal.\n";
    let dir = create_project_with_tools(
        agents_md,
        &[
            "cursor", "claude", "devin", "copilot", "codex", "opencode", "zoocode", "gemini",
            "zed", "kiro", "deepseek", "vibe", "kilo",
        ],
    );

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join("CLAUDE.md").exists());
    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    assert!(dir.path().join(".devin/rules/general.md").exists());
    assert!(dir.path().join("GEMINI.md").exists());
    assert!(dir.path().join(".roo/rules/00-general.md").exists());
    assert!(dir.path().join(".rules").exists());
    assert!(dir.path().join(".kiro/steering/general.md").exists());
}

// ===== Hook =====

#[test]
fn test_hook_install_requires_git() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# test").unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "hook", "install"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No .git directory"));
}

#[test]
fn test_hook_install_and_uninstall() {
    let dir = TempDir::new().unwrap();
    // Create a fake git repo
    fs::create_dir_all(dir.path().join(".git/hooks")).unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# test").unwrap();

    // Install
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "hook", "install"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Pre-commit hook installed"));

    let hook = dir.path().join(".git/hooks/pre-commit");
    assert!(hook.exists());
    let content = fs::read_to_string(&hook).unwrap();
    assert!(content.contains("conforme check"));

    // Install again — should say already installed
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "hook", "install"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already installed"));

    // Uninstall
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "hook", "uninstall"])
        .assert()
        .success()
        .stdout(predicate::str::contains("uninstalled"));

    assert!(!hook.exists());
}

#[test]
fn test_sync_creates_kiro_config() {
    let agents_md = r#"# Instructions
Follow AWS patterns.

## Rule: Lambda
<!-- activation: glob **/*.lambda.ts -->

Use handler pattern.
"#;
    let dir = create_project_with_tools(agents_md, &["kiro"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let general = dir.path().join(".kiro/steering/general.md");
    assert!(general.exists());
    let content = fs::read_to_string(&general).unwrap();
    assert!(content.contains("inclusion: always"));

    let lambda = dir.path().join(".kiro/steering/lambda.md");
    assert!(lambda.exists());
    let content = fs::read_to_string(&lambda).unwrap();
    assert!(content.contains("inclusion: fileMatch"));
    assert!(content.contains("fileMatchPattern"));
}

#[test]
fn test_sync_skills_and_mcp() {
    let agents_md = r#"# Instructions
Use TypeScript.

## Skill: deploy
<!-- description: Deploy the application -->
<!-- tools: Bash -->

Run `npm run deploy`.

## MCP: filesystem
<!-- command: npx -->
<!-- args: -y, @modelcontextprotocol/server-filesystem -->

## Agent: reviewer
<!-- description: Code review agent -->
<!-- model: gpt-4o -->
<!-- tools: codebase, terminal -->

Review all changes for bugs.
"#;
    let dir = create_project_with_tools(agents_md, &["claude", "copilot", "codex"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Claude: skills + MCP
    assert!(dir.path().join(".claude/skills/deploy/SKILL.md").exists());
    let skill = fs::read_to_string(dir.path().join(".claude/skills/deploy/SKILL.md")).unwrap();
    assert!(skill.contains("name: deploy"));
    assert!(skill.contains("allowed-tools: Bash"));

    assert!(dir.path().join(".mcp.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".mcp.json")).unwrap();
    assert!(mcp.contains("filesystem"));
    assert!(mcp.contains("npx"));

    // Copilot: skills + agents + MCP
    let copilot_skill = dir.path().join(".github/skills/deploy/SKILL.md");
    assert!(copilot_skill.exists());
    let copilot_skill_content = fs::read_to_string(&copilot_skill).unwrap();
    assert!(copilot_skill_content.contains("name: deploy"));
    assert!(copilot_skill_content.contains("allowed-tools: Bash"));
    assert!(dir.path().join(".github/agents/reviewer.agent.md").exists());
    let agent = fs::read_to_string(dir.path().join(".github/agents/reviewer.agent.md")).unwrap();
    assert!(agent.contains("name: reviewer"));
    assert!(agent.contains("model: gpt-4o"));

    assert!(dir.path().join(".vscode/mcp.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".vscode/mcp.json")).unwrap();
    assert!(mcp.contains("\"servers\""));

    // Codex: skills + project-scoped MCP TOML
    assert!(dir.path().join(".agents/skills/deploy/SKILL.md").exists());
    let codex_mcp = fs::read_to_string(dir.path().join(".codex/config.toml")).unwrap();
    assert!(codex_mcp.contains("[mcp_servers.filesystem]"));
    assert!(codex_mcp.contains("command = \"npx\""));
}

// ===== Remove =====

#[test]
fn test_remove_deletes_tool_files() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor", "devin"]);

    // Sync first
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    assert!(dir.path().join(".devin/rules/general.md").exists());

    // Remove cursor only
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "remove", "cursor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("removed"));

    // Cursor files should be gone
    assert!(!dir.path().join(".cursor/rules/general.mdc").exists());
    // Devin files should still exist
    assert!(dir.path().join(".devin/rules/general.md").exists());
}

#[test]
fn test_remove_leaves_no_empty_tool_directory_behind() {
    // An emptied `.cursor/` kept Cursor detected, so the next sync (or the
    // pre-commit `check`) wrote back or demanded what `remove` deleted.
    let agents_md =
        "# Instructions\nHello.\n\n## Skill: deploy\n<!-- description: Deploy -->\nRun deploy.\n";
    let dir = create_project_with_tools(agents_md, &["cursor", "devin"]);
    let root = dir.path().to_str().unwrap();

    conforme().args(["-C", root, "sync"]).assert().success();
    assert!(dir.path().join(".cursor/skills/deploy/SKILL.md").exists());

    conforme()
        .args(["-C", root, "remove", "cursor"])
        .assert()
        .success();

    assert!(!dir.path().join(".cursor").exists());
    assert!(dir.path().join(".devin/rules/general.md").exists());
    conforme().args(["-C", root, "check"]).assert().success();
}

#[test]
fn test_remove_says_how_to_stop_a_tool_still_detected() {
    // `.cursor/mcp.json` is merged, never deleted: Cursor stays detected and
    // the next sync writes its files back unless it is excluded.
    let agents_md =
        "# Instructions\nHello.\n\n## MCP: fs\n<!-- command: npx -->\n<!-- args: -y, fs -->\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);
    let root = dir.path().to_str().unwrap();

    conforme().args(["-C", root, "sync"]).assert().success();
    conforme()
        .args(["-C", root, "remove", "cursor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("exclude"));

    assert!(dir.path().join(".cursor/mcp.json").exists());
    assert!(!dir.path().join(".cursor/rules").exists());
}

#[test]
fn test_remove_no_files() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    // Remove without syncing first — no files to remove
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "remove", "cursor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No files to remove"));
}

// ===== Help AI =====

#[test]
fn test_help_ai() {
    conforme().arg("help-ai").assert().success().stdout(
        predicate::str::contains("Claude Code")
            .and(predicate::str::contains("Cursor"))
            .and(predicate::str::contains("Devin Desktop"))
            .and(predicate::str::contains("Kiro"))
            .and(predicate::str::contains("Mistral Vibe"))
            .and(predicate::str::contains("Kilo Code"))
            .and(predicate::str::contains("AGENTS.md")),
    );
}

// ===== Source-based flow =====

#[test]
fn test_sync_from_claude_source() {
    // Create a project with Claude as source and Cursor as target
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".claude/rules")).unwrap();
    fs::create_dir_all(dir.path().join(".cursor")).unwrap();

    // Write Claude config (source)
    fs::write(
        dir.path().join("CLAUDE.md"),
        "# Instructions\nUse TypeScript.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".claude/rules/api.md"),
        "---\npaths:\n  - src/api/**\n---\n\nFollow REST conventions.\n",
    )
    .unwrap();

    // Configure source
    fs::write(dir.path().join(".conformerc.toml"), "source = \"claude\"\n").unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Cursor should have the rules
    let general = dir.path().join(".cursor/rules/general.mdc");
    assert!(general.exists());
    let content = fs::read_to_string(&general).unwrap();
    assert!(content.contains("Use TypeScript."));
}

#[test]
fn test_sync_from_claude_mcp_to_codex() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".claude")).unwrap();
    fs::create_dir_all(dir.path().join(".codex")).unwrap();
    fs::write(
        dir.path().join("CLAUDE.md"),
        "# Instructions\nBe concise.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".mcp.json"),
        r#"{
  "mcpServers": {
    "dsfr": { "type": "stdio", "command": "npx", "args": ["dsfr-mcp"] },
    "docs": { "type": "http", "url": "https://example.com/mcp" }
  }
}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join(".codex/config.toml"),
        "# Preserve me\nmodel = \"gpt-test\"\n\n[mcp_servers.local]\nurl = \"http://localhost:3000/mcp\"\n",
    )
    .unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "sync",
            "--from",
            "claude",
            "--only",
            "codex",
        ])
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join(".codex/config.toml")).unwrap();
    assert!(content.contains("# Preserve me"));
    assert!(content.contains("model = \"gpt-test\""));
    assert!(content.contains("[mcp_servers.local]"));
    assert!(content.contains("[mcp_servers.dsfr]"));
    assert!(content.contains("[mcp_servers.docs]"));
    assert!(content.contains("command = \"npx\""));
    assert!(content.contains("url = \"https://example.com/mcp\""));
}

#[test]
fn test_remove_codex_preserves_shared_config() {
    let agents_md = r#"# Instructions

## MCP: filesystem
<!-- command: npx -->
"#;
    let dir = create_project_with_tools(agents_md, &["codex"]);
    fs::write(
        dir.path().join(".codex/config.toml"),
        "model = \"gpt-test\"\n\n[mcp_servers.filesystem]\ncommand = \"npx\"\n",
    )
    .unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "remove", "codex"])
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join(".codex/config.toml")).unwrap();
    assert!(content.contains("model = \"gpt-test\""));
    assert!(content.contains("[mcp_servers.filesystem]"));
}

/// Zed, Gemini, Kilo and OpenCode merge MCP servers into a settings file that
/// also holds the user's own configuration. `remove <tool>` must leave that
/// file in place rather than deleting the user's settings along with it.
#[test]
fn test_remove_preserves_merged_settings_files() {
    let agents_md = r#"# Instructions
Be helpful.

## MCP: filesystem
<!-- command: npx -->
"#;
    let cases = [
        (
            "zed",
            ".zed/settings.json",
            r#"{"theme":"One Dark"}"#,
            "One Dark",
        ),
        (
            "gemini",
            ".gemini/settings.json",
            r#"{"theme":"GitHub"}"#,
            "GitHub",
        ),
        (
            "kilo",
            ".kilo/kilo.jsonc",
            r#"{"model":"anthropic/claude-sonnet-4-5"}"#,
            "claude-sonnet",
        ),
        (
            "opencode",
            "opencode.json",
            r#"{"theme":"opencode"}"#,
            "theme",
        ),
    ];

    for (tool, settings, user_content, user_marker) in cases {
        let dir = create_project_with_tools(agents_md, &[tool]);
        let settings_path = dir.path().join(settings);
        fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        fs::write(&settings_path, user_content).unwrap();

        conforme()
            .args(["-C", dir.path().to_str().unwrap(), "sync"])
            .assert()
            .success();
        let merged = fs::read_to_string(&settings_path).unwrap();
        assert!(
            merged.contains(user_marker),
            "{tool}: user key lost on sync"
        );
        assert!(
            merged.contains("filesystem"),
            "{tool}: MCP server not merged"
        );

        conforme()
            .args(["-C", dir.path().to_str().unwrap(), "remove", tool])
            .assert()
            .success();
        assert!(
            settings_path.exists(),
            "{tool}: `remove` deleted the user's {settings}"
        );
        let after = fs::read_to_string(&settings_path).unwrap();
        assert!(
            after.contains(user_marker),
            "{tool}: user key lost on remove"
        );
    }
}

/// The top-level `.opencode/` directory holds user-owned files next to the
/// generated agents (`package.json` for plugins, commands, tools). Orphan
/// cleanup must not sweep them.
#[test]
fn test_sync_opencode_preserves_user_files_in_dot_opencode() {
    let agents_md = r#"# Instructions
Be helpful.

## Agent: reviewer
<!-- description: Code review -->

Review for bugs.
"#;
    let dir = create_project_with_tools(agents_md, &["opencode"]);
    let package_json = dir.path().join(".opencode/package.json");
    fs::write(&package_json, r#"{"dependencies":{"some-plugin":"1.0.0"}}"#).unwrap();
    let command = dir.path().join(".opencode/commands/release.md");
    fs::create_dir_all(command.parent().unwrap()).unwrap();
    fs::write(&command, "Cut a release.\n").unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".opencode/agents/reviewer.md").exists());
    assert!(package_json.exists(), "sync deleted .opencode/package.json");
    assert!(command.exists(), "sync deleted a user command");
}

/// Zed, OpenCode, VS Code (Copilot), Gemini, Kilo and Zoo Code settings may be
/// JSONC. A comment or trailing comma used to make conforme fall back to an
/// empty object and rewrite the file with its own keys only, wiping every
/// user setting. The merge must keep user keys and comments, and a second
/// run must see the file as in sync.
#[test]
fn test_sync_preserves_jsonc_settings_files() {
    let agents_md = r#"# Instructions
Be helpful.

## MCP: filesystem
<!-- command: npx -->
"#;
    let cases = [
        ("zed", ".zed/settings.json"),
        ("gemini", ".gemini/settings.json"),
        ("kilo", ".kilo/kilo.jsonc"),
        ("opencode", "opencode.json"),
        ("copilot", ".vscode/mcp.json"),
        ("zoocode", ".roo/mcp.json"),
    ];
    let user_content = "// project settings\n{\n  // keep me\n  \"user_setting\": \"kept\",\n}\n";

    for (tool, settings) in cases {
        let dir = create_project_with_tools(agents_md, &[tool]);
        let settings_path = dir.path().join(settings);
        fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        fs::write(&settings_path, user_content).unwrap();

        conforme()
            .args(["-C", dir.path().to_str().unwrap(), "sync"])
            .assert()
            .success();
        let merged = fs::read_to_string(&settings_path).unwrap();
        assert!(
            merged.contains("\"kept\""),
            "{tool}: user key lost\n{merged}"
        );
        assert!(
            merged.contains("// keep me"),
            "{tool}: comment lost\n{merged}"
        );
        assert!(
            merged.contains("filesystem"),
            "{tool}: MCP not merged\n{merged}"
        );

        conforme()
            .args(["-C", dir.path().to_str().unwrap(), "check"])
            .assert()
            .success();
    }
}

/// A settings file conforme cannot parse must be left untouched: the sync
/// fails loudly instead of replacing it with conforme's keys alone.
#[test]
fn test_sync_refuses_to_overwrite_unparsable_settings() {
    let agents_md = r#"# Instructions
Be helpful.

## MCP: filesystem
<!-- command: npx -->
"#;
    let dir = create_project_with_tools(agents_md, &["zed"]);
    let settings_path = dir.path().join(".zed/settings.json");
    fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    let broken = "{ \"theme\": \"One Dark\", \"vim_mode\": ";
    fs::write(&settings_path, broken).unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("will not overwrite"));
    assert_eq!(fs::read_to_string(&settings_path).unwrap(), broken);
}

/// `.vscode/mcp.json` and `.roo/mcp.json` also hold state conforme does not
/// generate: VS Code `inputs` (prompted secrets), and Zoo Code's per-server
/// `alwaysAllow`/`disabledTools`. Sync must keep it, and `remove` must not
/// delete the file wholesale.
#[test]
fn test_sync_and_remove_preserve_tool_state_in_mcp_files() {
    let agents_md = r#"# Instructions
Be helpful.

## MCP: filesystem
<!-- command: npx -->
"#;
    let cases = [
        (
            "copilot",
            ".vscode/mcp.json",
            r#"{"inputs":[{"id":"token","type":"promptString"}],"servers":{"filesystem":{"command":"old","envFile":".env"}}}"#,
            ["\"promptString\"", "\"envFile\""],
        ),
        (
            "zoocode",
            ".roo/mcp.json",
            r#"{"mcpServers":{"filesystem":{"command":"old","alwaysAllow":["read_file"],"disabledTools":["write_file"]}}}"#,
            ["\"read_file\"", "\"write_file\""],
        ),
    ];

    for (tool, mcp_file, user_content, markers) in cases {
        let dir = create_project_with_tools(agents_md, &[tool]);
        let mcp_path = dir.path().join(mcp_file);
        fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
        fs::write(&mcp_path, user_content).unwrap();

        conforme()
            .args(["-C", dir.path().to_str().unwrap(), "sync"])
            .assert()
            .success();
        let merged = fs::read_to_string(&mcp_path).unwrap();
        assert!(
            merged.contains("\"npx\""),
            "{tool}: server not updated\n{merged}"
        );
        assert!(
            !merged.contains("\"old\""),
            "{tool}: stale command kept\n{merged}"
        );
        for marker in markers {
            assert!(merged.contains(marker), "{tool}: lost {marker}\n{merged}");
        }

        conforme()
            .args(["-C", dir.path().to_str().unwrap(), "remove", tool])
            .assert()
            .success();
        assert!(mcp_path.exists(), "{tool}: `remove` deleted {mcp_file}");
    }
}

/// Orphan cleanup only deletes the kind of file conforme writes into a
/// directory. Kiro also accepts `.kiro/agents/<name>.json`, the DeepSeek
/// Harness accepts flat `.dsh/skills/<name>.md` skills, and a Copilot agent
/// may be a plain `.github/agents/<name>.md`; conforme generates none of
/// them, so a sync must never delete them.
#[test]
fn test_orphan_cleanup_spares_files_conforme_never_writes() {
    let agents_md = r#"# Instructions
Be helpful.

## Skill: deploy
<!-- description: Deploy the app -->
Run deploy.

## Agent: reviewer
<!-- description: Code review -->
Review for bugs.
"#;
    let dir = create_project_with_tools(agents_md, &["kiro", "deepseek", "copilot"]);
    let user_files = [
        (".kiro/agents/planner.json", r#"{"name":"planner"}"#),
        (
            ".dsh/skills/release.md",
            "---\nname: release\ndescription: Cut a release\n---\nRelease.\n",
        ),
        (
            ".github/agents/triage.md",
            "---\ndescription: Triage\n---\nTriage issues.\n",
        ),
    ];
    for (path, content) in user_files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
    }
    // A stale file of the generated kind is still an orphan.
    fs::create_dir_all(dir.path().join(".kiro/agents")).unwrap();
    fs::write(dir.path().join(".kiro/agents/old.md"), "Old.\n").unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    for (path, _) in user_files {
        assert!(dir.path().join(path).exists(), "sync deleted {path}");
    }
    assert!(dir.path().join(".kiro/agents/reviewer.md").exists());
    assert!(!dir.path().join(".kiro/agents/old.md").exists());
}

#[test]
fn test_migrate_from_codex_preserves_shared_config() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".codex")).unwrap();
    fs::create_dir_all(dir.path().join(".cursor")).unwrap();
    fs::write(
        dir.path().join(".codex/config.toml"),
        "model = \"gpt-test\"\n\n[mcp_servers.filesystem]\ncommand = \"npx\"\n",
    )
    .unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "codex",
            "--output",
            "cursor",
        ])
        .assert()
        .success();

    assert!(dir.path().join(".codex/config.toml").exists());
    let codex = fs::read_to_string(dir.path().join(".codex/config.toml")).unwrap();
    assert!(codex.contains("model = \"gpt-test\""));
    let cursor = fs::read_to_string(dir.path().join(".cursor/mcp.json")).unwrap();
    assert!(cursor.contains("filesystem"));
}

#[test]
fn test_sync_from_flag_override() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();
    fs::create_dir_all(dir.path().join(".windsurf")).unwrap();

    // Write Cursor rules (will be used as source via --from)
    fs::write(
        dir.path().join(".cursor/rules/general.mdc"),
        "---\nalwaysApply: true\n---\n\nCursor instructions.\n",
    )
    .unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "sync",
            "--from",
            "cursor",
        ])
        .assert()
        .success();

    // Devin should have been synced
    let devin_general = dir.path().join(".devin/rules/general.md");
    assert!(devin_general.exists());
    let content = fs::read_to_string(&devin_general).unwrap();
    assert!(content.contains("Cursor instructions."));
}

// ===== Orphan cleanup =====

#[test]
fn test_orphan_cleanup_on_rule_rename() {
    let agents_v1 =
        "# Instructions\nHello.\n\n## Rule: OldRule\n<!-- activation: always -->\n\nOld content.\n";
    let dir = create_project_with_tools(agents_v1, &["cursor"]);

    // First sync creates oldrule.mdc
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();
    assert!(dir.path().join(".cursor/rules/oldrule.mdc").exists());

    // Rename the rule in AGENTS.md
    let agents_v2 =
        "# Instructions\nHello.\n\n## Rule: NewRule\n<!-- activation: always -->\n\nNew content.\n";
    fs::write(dir.path().join("AGENTS.md"), agents_v2).unwrap();

    // Second sync should create newrule.mdc and remove oldrule.mdc
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();
    assert!(dir.path().join(".cursor/rules/newrule.mdc").exists());
    assert!(!dir.path().join(".cursor/rules/oldrule.mdc").exists());
}

#[test]
fn test_no_clean_flag_keeps_orphans() {
    let agents_v1 =
        "# Instructions\nHello.\n\n## Rule: OldRule\n<!-- activation: always -->\n\nOld.\n";
    let dir = create_project_with_tools(agents_v1, &["cursor"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();
    assert!(dir.path().join(".cursor/rules/oldrule.mdc").exists());

    let agents_v2 =
        "# Instructions\nHello.\n\n## Rule: NewRule\n<!-- activation: always -->\n\nNew.\n";
    fs::write(dir.path().join("AGENTS.md"), agents_v2).unwrap();

    // With --no-clean, old file should remain
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync", "--no-clean"])
        .assert()
        .success();
    assert!(dir.path().join(".cursor/rules/newrule.mdc").exists());
    assert!(dir.path().join(".cursor/rules/oldrule.mdc").exists()); // still there!
}

// ===== Diff command =====

#[test]
fn test_diff_no_changes() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "diff"])
        .assert()
        .success()
        .stdout(predicate::str::contains("All configs in sync"));
}

#[test]
fn test_diff_shows_changes() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Modify AGENTS.md
    fs::write(dir.path().join("AGENTS.md"), "# Instructions\nUpdated.\n").unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "diff"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cursor"));
}

// ===== Add command =====

#[test]
fn test_add_rule() {
    let dir = TempDir::new().unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "add",
            "rule",
            "TypeScript",
            "--activation",
            "glob **/*.ts",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added rule"));

    let content = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(content.contains("## Rule: TypeScript"));
    assert!(content.contains("activation: glob **/*.ts"));
}

#[test]
fn test_add_refuses_when_agents_md_is_regenerated_from_a_tool() {
    // With a Cursor source, AGENTS.md is an output: the next sync would drop
    // the added rule.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join(".conformerc.toml"), "source = \"cursor\"\n").unwrap();
    fs::write(root.join("AGENTS.md"), "Generated.\n").unwrap();

    run(
        root,
        &["add", "rule", "TypeScript", "--activation", "always"],
    )
    .failure()
    .stderr(predicate::str::contains("cursor"));
    assert_eq!(
        fs::read_to_string(root.join("AGENTS.md")).unwrap(),
        "Generated.\n"
    );

    // A source reading AGENTS.md itself (Codex) keeps the addition.
    fs::write(root.join(".conformerc.toml"), "source = \"codex\"\n").unwrap();
    run(
        root,
        &["add", "rule", "TypeScript", "--activation", "always"],
    )
    .success();
}

#[test]
fn test_add_mcp() {
    let dir = TempDir::new().unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "add",
            "mcp",
            "filesystem",
            "--command",
            "npx",
            "--args=-y,@mcp/server-fs",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added MCP server"));

    let content = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(content.contains("## MCP: filesystem"));
    assert!(content.contains("command: npx"));
}

#[test]
fn test_add_skill() {
    let dir = TempDir::new().unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "add",
            "skill",
            "deploy",
            "--description",
            "Deploy app",
            "--tools",
            "Bash",
        ])
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(content.contains("## Skill: deploy"));
    assert!(content.contains("description: Deploy app"));
    assert!(content.contains("tools: Bash"));
}

// ===== Validation =====

#[test]
fn test_sync_fails_on_duplicate_rule_names() {
    let agents_md = "# Test\n\n## Rule: Same\n<!-- activation: always -->\nA.\n\n## Rule: Same\n<!-- activation: always -->\nB.\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Duplicate rule name"));
}

// ===== .conformerc.toml =====

#[test]
fn test_conformerc_exclude() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor", "devin"]);

    // Exclude devin
    fs::write(
        dir.path().join(".conformerc.toml"),
        "exclude = [\"devin\"]\n",
    )
    .unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Cursor should have files
    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    // Devin should NOT (excluded)
    assert!(!dir.path().join(".devin/rules/general.md").exists());
}

#[test]
fn test_conformerc_only() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor", "devin", "claude"]);

    // Only sync to cursor
    fs::write(dir.path().join(".conformerc.toml"), "only = [\"cursor\"]\n").unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    assert!(!dir.path().join(".devin/rules/general.md").exists());
    // Claude would be skipped because only=cursor
}

// ===== Full real-world scenario =====

#[test]
fn test_full_config_all_tools() {
    let agents_md = r#"# Full Project Config

Use TypeScript with strict mode.

## Rule: TypeScript
<!-- activation: glob **/*.ts,**/*.tsx -->

Use strict TypeScript.

## Rule: Testing
<!-- activation: agent-decision -->
<!-- description: Apply for test files -->

Write thorough tests.

## Skill: deploy
<!-- description: Deploy to production -->
<!-- tools: Bash -->

Run `npm run deploy`.

## Agent: reviewer
<!-- description: Code review agent -->
<!-- model: gpt-4o -->
<!-- tools: Read, Grep -->

Review all changes for bugs.

## MCP: filesystem
<!-- command: npx -->
<!-- args: -y, @modelcontextprotocol/server-filesystem, /tmp -->
"#;
    let dir = create_project_with_tools(
        agents_md,
        &[
            "cursor", "claude", "devin", "copilot", "kiro", "zoocode", "gemini", "zed",
        ],
    );

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Claude: CLAUDE.md + .claude/rules/ + skills + agents + .mcp.json
    assert!(dir.path().join("CLAUDE.md").exists());
    assert!(dir.path().join(".claude/skills/deploy/SKILL.md").exists());
    assert!(dir.path().join(".claude/agents/reviewer.md").exists());
    assert!(dir.path().join(".mcp.json").exists());

    // Cursor: rules + agents + mcp
    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    assert!(dir.path().join(".cursor/rules/typescript.mdc").exists());
    assert!(dir.path().join(".cursor/agents/reviewer.md").exists());
    assert!(dir.path().join(".cursor/mcp.json").exists());

    // Copilot: instructions + skills + agents + mcp
    assert!(dir.path().join(".github/copilot-instructions.md").exists());
    assert!(dir.path().join(".github/skills/deploy/SKILL.md").exists());
    assert!(dir.path().join(".github/agents/reviewer.agent.md").exists());
    assert!(dir.path().join(".vscode/mcp.json").exists());

    // Kiro: steering + skills + agents + mcp
    assert!(dir.path().join(".kiro/steering/general.md").exists());
    assert!(dir.path().join(".kiro/steering/typescript.md").exists());
    assert!(dir.path().join(".kiro/skills/deploy/SKILL.md").exists());
    assert!(dir.path().join(".kiro/agents/reviewer.md").exists());
    assert!(dir.path().join(".kiro/settings/mcp.json").exists());

    // Gemini: GEMINI.md + agents + mcp
    assert!(dir.path().join("GEMINI.md").exists());
    assert!(dir.path().join(".gemini/agents/reviewer.md").exists());
    assert!(dir.path().join(".gemini/settings.json").exists());

    // Zoo Code: rules + mcp
    assert!(dir.path().join(".roo/rules/00-general.md").exists());
    assert!(dir.path().join(".roo/mcp.json").exists());

    // Devin: rules + project MCP file
    assert!(dir.path().join(".devin/rules/general.md").exists());
    assert!(dir.path().join(".devin/mcp_config.json").exists());

    // Zed: .rules + settings
    assert!(dir.path().join(".rules").exists());
    assert!(dir.path().join(".zed/settings.json").exists());

    // Check should pass after sync
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .success();
}

// ===== Init creates config =====

#[test]
fn test_init_creates_conformerc() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".cursor")).unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created .conformerc.toml"));

    assert!(dir.path().join(".conformerc.toml").exists());
    assert!(dir.path().join("AGENTS.md").exists());

    let config = fs::read_to_string(dir.path().join(".conformerc.toml")).unwrap();
    assert!(config.contains("source"));
    assert!(config.contains("clean = true"));
}

// ===== End-to-end source flow =====

#[test]
fn test_end_to_end_claude_source_flow() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".claude/rules")).unwrap();
    fs::create_dir_all(dir.path().join(".cursor")).unwrap();
    fs::create_dir_all(dir.path().join(".windsurf")).unwrap();

    fs::write(dir.path().join("CLAUDE.md"), "Use TypeScript.\n").unwrap();
    fs::write(
        dir.path().join(".claude/rules/api.md"),
        "---\npaths:\n  - src/api/**\n---\n\nFollow REST.\n",
    )
    .unwrap();
    fs::write(dir.path().join(".conformerc.toml"), "source = \"claude\"\n").unwrap();

    // Sync from Claude source
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Cursor should have general + api rules
    let cursor_general = fs::read_to_string(dir.path().join(".cursor/rules/general.mdc")).unwrap();
    assert!(cursor_general.contains("Use TypeScript"));

    let cursor_api = fs::read_to_string(dir.path().join(".cursor/rules/api.mdc")).unwrap();
    assert!(cursor_api.contains("Follow REST"));
    assert!(cursor_api.contains("globs"));

    // Devin should have rules too
    let ws_general = fs::read_to_string(dir.path().join(".devin/rules/general.md")).unwrap();
    assert!(ws_general.contains("Use TypeScript"));

    // AGENTS.md should be generated
    assert!(dir.path().join("AGENTS.md").exists());
    let agents = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(agents.contains("Use TypeScript"));

    // Check should pass
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .success();

    // Diff should show in sync
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "diff"])
        .assert()
        .success()
        .stdout(predicate::str::contains("in sync"));
}

// ===== Add then sync round-trip =====

#[test]
fn test_add_then_sync_round_trip() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".cursor")).unwrap();

    // Init
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "init"])
        .assert()
        .success();

    // Add a rule via CLI
    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "add",
            "rule",
            "Security",
            "--activation",
            "agent-decision",
            "--content",
            "Check for XSS.",
        ])
        .assert()
        .success();

    // Sync
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Verify the rule made it to Cursor with correct frontmatter
    let security = fs::read_to_string(dir.path().join(".cursor/rules/security.mdc")).unwrap();
    assert!(security.contains("Check for XSS"));
}

// ===== Status shows source =====

#[test]
fn test_status_shows_configured_source() {
    let agents_md = "# Test\n";
    let dir = create_project_with_tools(agents_md, &["cursor"]);
    fs::write(dir.path().join(".conformerc.toml"), "source = \"cursor\"\n").unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cursor"));
}

// ===== Migrate =====

#[test]
fn test_migrate_gemini_to_opencode() {
    let dir = TempDir::new().unwrap();
    // Set up gemini source
    fs::create_dir_all(dir.path().join(".gemini")).unwrap();
    fs::write(
        dir.path().join("GEMINI.md"),
        "# Instructions\n\nUse TypeScript.\n",
    )
    .unwrap();
    // Set up opencode target directory
    fs::create_dir_all(dir.path().join(".opencode")).unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "gemini",
            "--output",
            "opencode",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Migrated"));

    // Source file should be deleted
    assert!(!dir.path().join("GEMINI.md").exists());
}

#[test]
fn test_migrate_gemini_to_cursor() {
    let dir = TempDir::new().unwrap();
    // Set up gemini source with content
    fs::create_dir_all(dir.path().join(".gemini")).unwrap();
    fs::write(
        dir.path().join("GEMINI.md"),
        "# Instructions\n\nUse TypeScript strict mode.\n",
    )
    .unwrap();
    // Set up cursor target
    fs::create_dir_all(dir.path().join(".cursor")).unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "gemini",
            "--output",
            "cursor",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Migrated"));

    // Source should be deleted
    assert!(!dir.path().join("GEMINI.md").exists());
    // Cursor rules should exist
    assert!(dir.path().join(".cursor/rules").is_dir());
}

#[test]
fn test_migrate_dry_run_does_not_modify() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".gemini")).unwrap();
    fs::write(dir.path().join("GEMINI.md"), "Hello world.\n").unwrap();
    fs::create_dir_all(dir.path().join(".opencode")).unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "gemini",
            "--output",
            "opencode",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run"));

    // Source file must still exist
    assert!(dir.path().join("GEMINI.md").exists());
}

#[test]
fn test_migrate_same_source_and_output_fails() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".gemini")).unwrap();
    fs::write(dir.path().join("GEMINI.md"), "Hello.\n").unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "gemini",
            "--output",
            "gemini",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be the same"));
}

#[test]
fn test_migrate_unknown_source_fails() {
    let dir = TempDir::new().unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "nonexistent",
            "--output",
            "cursor",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown source tool"));
}

#[test]
fn test_migrate_unknown_output_fails() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".gemini")).unwrap();
    fs::write(dir.path().join("GEMINI.md"), "Hello.\n").unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "gemini",
            "--output",
            "nonexistent",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown output tool"));
}

#[test]
fn test_migrate_source_not_detected_fails() {
    let dir = TempDir::new().unwrap();
    // No gemini files at all

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "gemini",
            "--output",
            "cursor",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not detected"));
}

#[test]
fn test_migrate_deletes_managed_directories_content() {
    let dir = TempDir::new().unwrap();
    // Set up gemini source with skills
    fs::create_dir_all(dir.path().join(".gemini/skills/deploy")).unwrap();
    fs::write(
        dir.path().join(".gemini/skills/deploy/SKILL.md"),
        "---\nname: deploy\n---\nDeploy.\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join(".gemini/agents")).unwrap();
    fs::write(
        dir.path().join(".gemini/agents/reviewer.md"),
        "---\nkind: local\n---\nReview.\n",
    )
    .unwrap();
    fs::write(dir.path().join("GEMINI.md"), "Instructions.\n").unwrap();
    // Set up opencode target
    fs::create_dir_all(dir.path().join(".opencode")).unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "gemini",
            "--output",
            "opencode",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Migrated"));

    // Source files should be deleted
    assert!(!dir.path().join("GEMINI.md").exists());
    assert!(!dir.path().join(".gemini/skills/deploy/SKILL.md").exists());
    assert!(!dir.path().join(".gemini/agents/reviewer.md").exists());
}

// ===== Watch requires source =====

#[test]
fn test_watch_requires_source() {
    let dir = TempDir::new().unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "watch"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No source"));
}

// ===== Settings conforme merges into, never owns =====

const MCP_AGENTS_MD: &str = r#"# Instructions
Be helpful.

## MCP: fs
<!-- command: npx -->
<!-- args: -y, @mcp/fs -->
"#;

#[test]
fn test_sync_keeps_kiro_per_server_state_and_remove_keeps_the_file() {
    let dir = create_project_with_tools(MCP_AGENTS_MD, &["kiro"]);
    let mcp = dir.path().join(".kiro/settings/mcp.json");
    fs::create_dir_all(mcp.parent().unwrap()).unwrap();
    fs::write(
        &mcp,
        r#"{"mcpServers": {"fs": {"command": "old", "autoApprove": ["read"], "disabledTools": ["delete"], "disabled": true}}}"#,
    )
    .unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp).unwrap()).unwrap();
    let fs_server = &value["mcpServers"]["fs"];
    assert_eq!(fs_server["command"], "npx");
    assert_eq!(fs_server["autoApprove"], serde_json::json!(["read"]));
    assert_eq!(fs_server["disabledTools"], serde_json::json!(["delete"]));
    // A synced server is re-enabled, so `check` never passes while it is hidden.
    assert!(fs_server.get("disabled").is_none());

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "remove", "kiro"])
        .assert()
        .success();
    assert!(mcp.exists(), "remove deleted the shared Kiro MCP file");
}

#[test]
fn test_sync_keeps_claude_and_cursor_per_server_settings() {
    let dir = create_project_with_tools(MCP_AGENTS_MD, &["claude", "cursor"]);
    fs::write(
        dir.path().join(".mcp.json"),
        r#"{"mcpServers": {"fs": {"command": "old", "timeout": 30000}}}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join(".cursor/mcp.json"),
        r#"{"mcpServers": {"fs": {"command": "old", "envFile": ".env"}}}"#,
    )
    .unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let read = |p: &str| -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(dir.path().join(p)).unwrap()).unwrap()
    };
    let claude = read(".mcp.json");
    assert_eq!(claude["mcpServers"]["fs"]["command"], "npx");
    assert_eq!(claude["mcpServers"]["fs"]["timeout"], 30000);
    let cursor = read(".cursor/mcp.json");
    assert_eq!(cursor["mcpServers"]["fs"]["command"], "npx");
    assert_eq!(cursor["mcpServers"]["fs"]["envFile"], ".env");
}

#[test]
fn test_sync_keeps_user_opencode_agents() {
    let agents_md = r#"# Instructions
Be helpful.

## Agent: reviewer
<!-- description: Review -->
Review.
"#;
    let dir = create_project_with_tools(agents_md, &["opencode"]);
    let config = dir.path().join("opencode.json");
    fs::write(
        &config,
        r#"{"agent": {"build": {"permission": {"bash": "ask"}}, "reviewer": {"description": "old", "mode": "subagent", "temperature": 0.2}}}"#,
    )
    .unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(value["agent"]["build"]["permission"]["bash"], "ask");
    assert_eq!(value["agent"]["reviewer"]["description"], "Review");
    assert_eq!(value["agent"]["reviewer"]["temperature"], 0.2);
}

#[test]
fn test_sync_without_source_servers_keeps_hand_written_mcp() {
    // A project that syncs only rules with conforme keeps its MCP servers in
    // each tool by hand: with no server in the source, they must survive.
    let dir = create_project_with_tools("# Instructions\nBe helpful.\n", &["zed", "cursor"]);
    let settings = dir.path().join(".zed/settings.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let zed = r#"{"theme": "One Dark", "context_servers": {"fs": {"command": "npx"}}}"#;
    fs::write(&settings, zed).unwrap();
    let cursor = dir.path().join(".cursor/mcp.json");
    let cursor_mcp = r#"{"mcpServers": {"mine": {"command": "node"}}}"#;
    fs::write(&cursor, cursor_mcp).unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(&settings).unwrap(), zed);
    assert_eq!(fs::read_to_string(&cursor).unwrap(), cursor_mcp);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .success();
}

#[test]
fn test_migrate_between_tools_sharing_agents_skills_keeps_the_output() {
    let dir = TempDir::new().unwrap();
    let skill = dir.path().join(".agents/skills/deploy/SKILL.md");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    fs::write(dir.path().join(".rules"), "").unwrap();
    fs::write(
        &skill,
        "---\nname: deploy\ndescription: Deploy\n---\nRun.\n",
    )
    .unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "zed",
            "--output",
            "codex",
        ])
        .assert()
        .success();

    // Codex reads the same `.agents/skills/` Zed did: migrating must not
    // delete what it has just written there.
    let content = fs::read_to_string(&skill).unwrap();
    assert!(content.contains("name: deploy"), "{content}");
}

/// Every tool can be the source, and a second sync from it changes nothing.
/// Tools that read AGENTS.md natively (Codex, OpenCode, DeepSeek, Vibe, Kilo) used to
/// read back the AGENTS.md the previous sync generated as one block, so their
/// instructions grew on every sync and `check` never passed.
#[test]
fn test_sync_from_every_tool_is_idempotent() {
    let agents_md = r#"# Instructions
Be helpful.

## Rule: TS
<!-- activation: glob src/**/*.{ts,tsx} -->
Use TS.

## Rule: Docs
<!-- activation: agent-decision -->
<!-- description: Writing docs -->
Docs.

## Skill: Déployer App
<!-- description: Deploy -->
<!-- invocation: manual -->
Run deploy.

## Agent: reviewer
<!-- description: Review -->
<!-- model: sonnet -->
<!-- tools: Read, Grep, Bash, mcp__github__list_issues -->
Review.

## MCP: fs
<!-- command: npx -->
<!-- args: -y, @mcp/fs -->
<!-- env: TOKEN=${GH_TOKEN} -->

## MCP: api
<!-- url: https://example.com/mcp -->
<!-- headers: Authorization=Bearer ${API_KEY} -->
"#;
    let tools = [
        "claude", "cursor", "devin", "copilot", "codex", "opencode", "zoocode", "gemini", "zed",
        "kiro", "deepseek", "vibe", "kilo",
    ];
    for source in conforme::adapters::all_adapters() {
        let dir = create_project_with_tools(agents_md, &tools);
        let root = dir.path().to_str().unwrap();
        conforme().args(["-C", root, "sync"]).assert().success();

        for _ in 0..2 {
            conforme()
                .args(["-C", root, "sync", "--from", source.id()])
                .assert()
                .success();
        }
        conforme()
            .args(["-C", root, "sync", "--from", source.id()])
            .assert()
            .success()
            .stdout(predicate::str::contains("All configs already in sync."));
        conforme()
            .args(["-C", root, "check", "--from", source.id()])
            .assert()
            .success();
    }
}

// ===== Regressions found by the 2026-10-01 review =====

#[test]
fn test_old_windsurf_id_is_an_error_not_silently_ignored() {
    let dir = create_project_with_tools("# Instructions\nBe helpful.\n", &["devin"]);
    let root = dir.path().to_str().unwrap();
    fs::create_dir_all(dir.path().join(".windsurf/rules")).unwrap();
    fs::write(dir.path().join(".windsurf/rules/team.md"), "Mine.\n").unwrap();

    // An ignored `exclude` would sync (and clean) the tool the user excluded.
    fs::write(
        dir.path().join(".conformerc.toml"),
        "exclude = [\"windsurf\"]\n",
    )
    .unwrap();
    conforme()
        .args(["-C", root, "sync"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("renamed `devin`"));
    assert!(!dir.path().join(".devin/rules/general.md").exists());
    assert!(dir.path().join(".windsurf/rules/team.md").exists());

    fs::remove_file(dir.path().join(".conformerc.toml")).unwrap();
    for args in [
        vec!["sync", "--from", "windsurf"],
        vec!["sync", "--only", "windsurf"],
        vec!["remove", "windsurf"],
        vec!["migrate", "--source", "windsurf", "--output", "cursor"],
    ] {
        conforme()
            .arg("-C")
            .arg(root)
            .args(&args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("renamed `devin`"));
    }
}

#[test]
fn test_targets_leave_the_source_skills_root_alone() {
    // Codex is the source and reads a nested skill from `.agents/skills/`;
    // Zed writes skills to the same root and must not add a flat copy
    // there (Codex would load the skill twice).
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("AGENTS.md"), "Be helpful.\n").unwrap();
    fs::write(root.join(".conformerc.toml"), "source = \"codex\"\n").unwrap();
    fs::write(root.join(".rules"), "").unwrap();
    let nested = root.join(".agents/skills/team/deploy");
    fs::create_dir_all(nested.join("scripts")).unwrap();
    fs::write(
        nested.join("SKILL.md"),
        "---\nname: deploy\ndescription: Deploy\n---\nRun.\n",
    )
    .unwrap();
    fs::write(nested.join("scripts/run.sh"), "echo").unwrap();

    conforme()
        .args(["-C", root.to_str().unwrap(), "sync"])
        .assert()
        .success();
    assert!(!root.join(".agents/skills/deploy").exists());
    conforme()
        .args(["-C", root.to_str().unwrap(), "check"])
        .assert()
        .success();

    // Removing Zed does not delete what Codex, the source, reads.
    conforme()
        .args(["-C", root.to_str().unwrap(), "remove", "zed"])
        .assert()
        .success();
    assert!(nested.join("SKILL.md").exists());
    assert!(nested.join("scripts/run.sh").exists());
}

#[test]
fn test_migrate_between_tools_sharing_agents_skills_keeps_bundled_files() {
    let dir = TempDir::new().unwrap();
    let skill = dir.path().join(".agents/skills/deploy");
    fs::create_dir_all(skill.join("scripts")).unwrap();
    fs::write(dir.path().join(".rules"), "").unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: deploy\ndescription: Deploy\n---\nRun.\n",
    )
    .unwrap();
    fs::write(skill.join("scripts/run.sh"), "echo").unwrap();

    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "zed",
            "--output",
            "codex",
        ])
        .assert()
        .success();
    assert!(skill.join("SKILL.md").exists());
    assert!(skill.join("scripts/run.sh").exists());
}

const NATIVE_AGENTS_MD: &str =
    "# Project\nAlways run make test.\n\n## Rule: TS\n<!-- activation: glob src/**/*.ts -->\nUse TS.\n";

#[test]
fn test_claude_source_reading_agents_md_keeps_it() {
    // Without CLAUDE.md, Claude Code loads AGENTS.md: it is the source's own
    // instruction file, never regenerated from the rest of the config.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("AGENTS.md"), NATIVE_AGENTS_MD).unwrap();
    let skill = root.join(".claude/skills/deploy/SKILL.md");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    fs::write(
        &skill,
        "---\nname: deploy\ndescription: Deploy\n---\nRun.\n",
    )
    .unwrap();
    fs::create_dir_all(root.join(".gemini")).unwrap();
    fs::write(root.join(".conformerc.toml"), "source = \"claude\"\n").unwrap();

    conforme()
        .args(["-C", root.to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(root.join("AGENTS.md")).unwrap(),
        NATIVE_AGENTS_MD
    );
    let gemini = fs::read_to_string(root.join("GEMINI.md")).unwrap();
    assert!(gemini.contains("Always run make test."), "{gemini}");
    assert!(gemini.contains("Use TS."), "{gemini}");
    assert!(root.join(".gemini/skills/deploy/SKILL.md").exists());

    conforme()
        .args(["-C", root.to_str().unwrap(), "gitignore", "install"])
        .assert()
        .success();
    let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(
        !gitignore.lines().any(|l| l.trim() == "/AGENTS.md"),
        "{gitignore}"
    );
}

#[test]
fn test_gemini_source_with_agents_md_context_file_keeps_it() {
    // `context.fileName` makes Gemini CLI load AGENTS.md instead of GEMINI.md.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("AGENTS.md"), NATIVE_AGENTS_MD).unwrap();
    fs::create_dir_all(root.join(".gemini")).unwrap();
    fs::write(
        root.join(".gemini/settings.json"),
        "{\n  // where Gemini reads its instructions\n  \"context\": {\"fileName\": [\"AGENTS.md\"]}\n}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join(".claude")).unwrap();

    conforme()
        .args(["-C", root.to_str().unwrap(), "sync", "--from", "gemini"])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(root.join("AGENTS.md")).unwrap(),
        NATIVE_AGENTS_MD
    );
    let claude = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
    assert!(claude.contains("Always run make test."), "{claude}");
    assert!(root.join(".claude/rules/ts.md").exists());
}

fn run(root: &std::path::Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut full = vec!["-C", root.to_str().unwrap()];
    full.extend_from_slice(args);
    conforme().args(full).assert()
}

const CLAUDE_MD_SOURCE: &str =
    "Top.\n\n## Rule: ts\n<!-- activation: glob **/*.ts -->\nStrict TS.\n";

#[test]
fn test_source_reading_claude_md_keeps_it_through_sync_remove_and_gitignore() {
    // OpenCode reads CLAUDE.md when there is no AGENTS.md: that file is the
    // source. The Claude Code target must not rewrite it (which moved the rule
    // out, and the next sync then cleaned it everywhere), `remove claude`
    // must not delete it and `gitignore install` must not ignore it.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".opencode")).unwrap();
    fs::create_dir_all(root.join(".claude")).unwrap();
    fs::create_dir_all(root.join(".cursor")).unwrap();
    fs::write(root.join("CLAUDE.md"), CLAUDE_MD_SOURCE).unwrap();
    fs::write(root.join(".conformerc.toml"), "source = \"opencode\"\n").unwrap();

    run(root, &["sync"]).success();
    run(root, &["sync"]).success();
    run(root, &["check"]).success();

    assert_eq!(
        fs::read_to_string(root.join("CLAUDE.md")).unwrap(),
        CLAUDE_MD_SOURCE
    );
    let cursor = fs::read_to_string(root.join(".cursor/rules/ts.mdc")).unwrap();
    assert!(cursor.contains("Strict TS."), "{cursor}");

    run(root, &["gitignore", "install"]).success();
    let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(!gitignore.lines().any(|l| l == "/CLAUDE.md"), "{gitignore}");

    run(root, &["remove", "claude"]).success();
    assert_eq!(
        fs::read_to_string(root.join("CLAUDE.md")).unwrap(),
        CLAUDE_MD_SOURCE
    );
}

#[test]
fn test_deepseek_shared_skills_fallback_is_the_sources() {
    // dsh reads `.agents/skills` when `.dsh/skills` has none: Codex must not
    // rewrite those skills, `remove codex` must not delete them, and they
    // stay tracked.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".dsh")).unwrap();
    fs::create_dir_all(root.join(".codex")).unwrap();
    fs::write(root.join("AGENTS.md"), "Be helpful.\n").unwrap();
    let skill = root.join(".agents/skills/lint/SKILL.md");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    let text =
        "---\nname: lint\ndescription: Lint\nlicense: MIT\nallowed-tools: Bash\n---\nLint.\n";
    fs::write(&skill, text).unwrap();
    fs::write(root.join(".conformerc.toml"), "source = \"deepseek\"\n").unwrap();

    run(root, &["sync"]).success();
    assert_eq!(fs::read_to_string(&skill).unwrap(), text);

    run(root, &["gitignore", "install"]).success();
    let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(!gitignore.contains(".agents/skills/"), "{gitignore}");

    run(root, &["remove", "codex"]).success();
    assert_eq!(fs::read_to_string(&skill).unwrap(), text);
}

#[test]
fn test_check_status_diff_and_gitignore_respect_exclude() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("AGENTS.md"), "Instr.\n").unwrap();
    fs::create_dir_all(root.join(".cursor/rules")).unwrap();
    fs::create_dir_all(root.join(".kiro")).unwrap();
    fs::write(
        root.join(".conformerc.toml"),
        "exclude = [\"kiro\", \"cursor\"]\n",
    )
    .unwrap();
    fs::write(root.join(".cursor/rules/mine.mdc"), "Hand-written.\n").unwrap();

    run(root, &["sync"]).success();
    // Excluded tools are not "out of sync": the pre-commit hook passes.
    run(root, &["check"]).success();
    run(root, &["diff"])
        .success()
        .stdout(predicate::str::contains("All configs in sync"));
    run(root, &["status"])
        .success()
        .stdout(predicate::str::contains("Excluded"));

    run(root, &["gitignore", "install"]).success();
    let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(!gitignore.contains(".cursor/"), "{gitignore}");
    assert!(!gitignore.contains(".kiro/"), "{gitignore}");
}

#[test]
fn test_check_reports_a_stale_generated_agents_md() {
    // With a Claude Code source, AGENTS.md is generated for Codex, OpenCode,
    // DeepSeek, Vibe and Kilo: `check` must notice when it is stale.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".codex")).unwrap();
    fs::write(root.join("CLAUDE.md"), "First.\n").unwrap();
    fs::write(root.join(".conformerc.toml"), "source = \"claude\"\n").unwrap();

    run(root, &["sync"]).success();
    run(root, &["check"]).success();
    fs::write(root.join("CLAUDE.md"), "Second.\n").unwrap();
    run(root, &["check"])
        .failure()
        .stdout(predicate::str::contains("AGENTS.md"));
}

#[test]
fn test_gemini_target_writes_the_context_file_it_loads() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("AGENTS.md"), "Instr.\n").unwrap();
    fs::create_dir_all(root.join(".gemini")).unwrap();
    fs::write(
        root.join(".gemini/settings.json"),
        r#"{"context": {"fileName": "CONTEXT.md"}}"#,
    )
    .unwrap();

    run(root, &["sync"]).success();
    assert_eq!(
        fs::read_to_string(root.join("CONTEXT.md")).unwrap(),
        "Instr.\n"
    );
    assert!(!root.join("GEMINI.md").exists());

    // Gemini loading AGENTS.md alone already gets everything from it.
    fs::remove_file(root.join("CONTEXT.md")).unwrap();
    fs::write(
        root.join(".gemini/settings.json"),
        r#"{"context": {"fileName": ["AGENTS.md"]}}"#,
    )
    .unwrap();
    run(root, &["sync"]).success();
    assert!(!root.join("GEMINI.md").exists());
    assert!(!root.join("CONTEXT.md").exists());
}

#[test]
fn test_claude_local_md_does_not_turn_agents_md_into_an_output() {
    // A personal CLAUDE.local.md must not make conforme regenerate the shared
    // AGENTS.md of a Claude Code source on one machine and not on another.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".claude/rules")).unwrap();
    fs::create_dir_all(root.join(".cursor")).unwrap();
    fs::write(root.join(".claude/rules/style.md"), "Use tabs.\n").unwrap();
    fs::write(root.join("AGENTS.md"), NATIVE_AGENTS_MD).unwrap();
    fs::write(root.join("CLAUDE.local.md"), "My notes.\n").unwrap();
    fs::write(root.join(".conformerc.toml"), "source = \"claude\"\n").unwrap();

    run(root, &["sync"]).success();
    assert_eq!(
        fs::read_to_string(root.join("AGENTS.md")).unwrap(),
        NATIVE_AGENTS_MD
    );
}

#[test]
fn test_sync_from_an_empty_source_writes_and_cleans_nothing() {
    // A source tool that reads back as empty (a bare `.roo/`) must neither
    // write a blank AGENTS.md nor clean every rule of the other tools.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".roo")).unwrap();
    fs::create_dir_all(root.join(".cursor/rules")).unwrap();
    let old = "---\nalwaysApply: true\n---\nOld.\n";
    fs::write(root.join(".cursor/rules/old.mdc"), old).unwrap();

    conforme()
        .args(["-C", root.to_str().unwrap(), "sync", "--from", "zoocode"])
        .assert()
        .success()
        .stderr(predicate::str::contains("nothing to sync"));

    assert!(!root.join("AGENTS.md").exists());
    assert_eq!(
        fs::read_to_string(root.join(".cursor/rules/old.mdc")).unwrap(),
        old
    );
}

fn migrate(root: &std::path::Path, source: &str, output: &str) {
    conforme()
        .args([
            "-C",
            root.to_str().unwrap(),
            "migrate",
            "--source",
            source,
            "--output",
            output,
        ])
        .assert()
        .success();
}

#[test]
fn test_migrate_codex_to_zed_leaves_shared_skills_byte_identical() {
    // Codex and Zed read the same `.agents/skills/`: migrating between them
    // must not rewrite the user's skills (dropping `license`, `metadata` or
    // keys another tool reads, such as `mcpServers`) nor add flat copies of
    // nested ones.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".codex")).unwrap();
    fs::write(root.join("AGENTS.md"), "Be helpful.\n").unwrap();
    let linear = root.join(".agents/skills/linear/SKILL.md");
    let nested = root.join(".agents/skills/nested/deep/SKILL.md");
    fs::create_dir_all(linear.parent().unwrap()).unwrap();
    fs::create_dir_all(nested.parent().unwrap()).unwrap();
    let linear_text = "---\nname: linear\ndescription: Linear issues\nlicense: MIT\nmetadata:\n  short-description: Issues\nmcpServers:\n  linear:\n    url: https://mcp.linear.app/mcp\n---\nUse Linear.\n";
    let nested_text = "---\nname: deep\ndescription: Deep\n---\nDeep.\n";
    fs::write(&linear, linear_text).unwrap();
    fs::write(&nested, nested_text).unwrap();

    migrate(root, "codex", "zed");

    assert_eq!(fs::read_to_string(&linear).unwrap(), linear_text);
    assert_eq!(fs::read_to_string(&nested).unwrap(), nested_text);
    assert!(!root.join(".agents/skills/deep").exists());
}

#[test]
fn test_migrate_keeps_skills_another_detected_tool_reads() {
    // Zed → Claude while Codex is still set up: `.agents/skills/` is Codex's
    // too, so the migration must leave it in place.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join(".rules"), "Be helpful.\n").unwrap();
    fs::create_dir_all(root.join(".codex")).unwrap();
    let skill = root.join(".agents/skills/deploy/SKILL.md");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    fs::write(
        &skill,
        "---\nname: deploy\ndescription: Deploy\n---\nRun.\n",
    )
    .unwrap();

    migrate(root, "zed", "claude");

    assert!(!root.join(".rules").exists());
    assert!(root.join(".claude/skills/deploy/SKILL.md").exists());
    assert_eq!(
        fs::read_to_string(&skill).unwrap(),
        "---\nname: deploy\ndescription: Deploy\n---\nRun.\n"
    );
}

#[test]
fn test_migrate_is_not_blocked_by_another_tools_unreadable_settings() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join(".rules"), "Be helpful.\n").unwrap();
    fs::create_dir_all(root.join(".gemini")).unwrap();
    fs::write(root.join(".gemini/settings.json"), "{ not json").unwrap();
    fs::create_dir_all(root.join(".zed")).unwrap();
    fs::write(
        root.join(".zed/settings.json"),
        r#"{"context_servers": {"fs": {"command": "npx"}}}"#,
    )
    .unwrap();

    migrate(root, "zed", "claude");

    assert!(root.join("CLAUDE.md").exists());
    assert_eq!(
        fs::read_to_string(root.join(".gemini/settings.json")).unwrap(),
        "{ not json"
    );
}

#[test]
fn test_migrate_to_a_tool_reading_agents_md_writes_it() {
    // Codex keeps its instructions in AGENTS.md: migrating there must write
    // it, not delete CLAUDE.md and the rules into nothing.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".claude/rules")).unwrap();
    fs::write(root.join("CLAUDE.md"), "Precious.\n").unwrap();
    fs::write(root.join(".claude/rules/style.md"), "Tabs.\n").unwrap();

    migrate(root, "claude", "codex");

    let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(agents.contains("Precious."), "{agents}");
    assert!(agents.contains("## Rule: style"), "{agents}");
    assert!(agents.contains("Tabs."), "{agents}");
    assert!(!root.join("CLAUDE.md").exists());
}

#[test]
fn test_migrate_refuses_to_overwrite_a_different_agents_md() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".claude")).unwrap();
    fs::write(root.join("CLAUDE.md"), "Precious.\n").unwrap();
    fs::write(root.join("AGENTS.md"), "Something else.\n").unwrap();

    run(
        root,
        &["migrate", "--source", "claude", "--output", "codex"],
    )
    .failure()
    .stderr(predicate::str::contains("AGENTS.md"));

    assert_eq!(
        fs::read_to_string(root.join("AGENTS.md")).unwrap(),
        "Something else.\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("CLAUDE.md")).unwrap(),
        "Precious.\n"
    );
}

#[test]
fn test_migrate_keeps_what_the_output_cannot_hold() {
    // A skill's text files travel with it; a file conforme cannot carry (not
    // text) keeps its folder in place; Zed has no agents, so they stay.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("GEMINI.md"), "Instr.\n").unwrap();
    let skill = root.join(".gemini/skills/deploy");
    fs::create_dir_all(skill.join("scripts")).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: deploy\ndescription: Deploy\n---\nRun scripts/run.sh.\n",
    )
    .unwrap();
    fs::write(skill.join("scripts/run.sh"), "echo deploy\n").unwrap();
    let logo = root.join(".gemini/skills/brand");
    fs::create_dir_all(&logo).unwrap();
    fs::write(
        logo.join("SKILL.md"),
        "---\nname: brand\ndescription: Brand\n---\nUse logo.png.\n",
    )
    .unwrap();
    fs::write(
        logo.join("logo.png"),
        [0x89u8, 0x50, 0x4e, 0x47, 0xff, 0xfe],
    )
    .unwrap();
    fs::create_dir_all(root.join(".gemini/agents")).unwrap();
    let agent = "---\nname: rev\ndescription: Review\nkind: local\n---\nReview.\n";
    fs::write(root.join(".gemini/agents/rev.md"), agent).unwrap();

    migrate(root, "gemini", "zed");

    assert_eq!(
        fs::read_to_string(root.join(".agents/skills/deploy/scripts/run.sh")).unwrap(),
        "echo deploy\n"
    );
    assert!(!skill.join("SKILL.md").exists());
    assert!(!skill.join("scripts/run.sh").exists());
    assert!(logo.join("logo.png").exists());
    assert!(logo.join("SKILL.md").exists());
    assert_eq!(
        fs::read_to_string(root.join(".gemini/agents/rev.md")).unwrap(),
        agent
    );
    assert!(!root.join("GEMINI.md").exists());
}

/// Every skills directory each tool reads, relative to the project root.
const SKILL_ROOTS: &[&str] = &[
    ".cursor/skills",
    ".devin/skills",
    ".github/skills",
    ".agents/skills",
    ".opencode/skills",
    ".roo/skills",
    ".gemini/skills",
    ".kiro/skills",
    ".dsh/skills",
];

#[test]
fn test_bundled_skill_files_reach_every_tool_and_stay_in_step() {
    // A skill's scripts and references are part of it: every tool's copy gets
    // them, and a file removed from the source leaves every copy.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let skill = root.join(".claude/skills/deploy");
    fs::create_dir_all(skill.join("scripts")).unwrap();
    fs::create_dir_all(skill.join("references")).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: deploy\ndescription: Deploy\n---\nRun scripts/run.sh, see references/env.md.\n",
    )
    .unwrap();
    fs::write(skill.join("scripts/run.sh"), "echo deploy\n").unwrap();
    fs::write(skill.join("references/env.md"), "# Env\n").unwrap();
    fs::write(root.join("CLAUDE.md"), "Be helpful.\n").unwrap();
    fs::write(root.join(".conformerc.toml"), "source = \"claude\"\n").unwrap();
    for tool_dir in [
        ".cursor",
        ".devin",
        ".codex",
        ".opencode",
        ".roo",
        ".gemini",
        ".kiro",
        ".dsh",
    ] {
        fs::create_dir_all(root.join(tool_dir)).unwrap();
    }
    fs::create_dir_all(root.join(".github/skills")).unwrap();

    run(root, &["sync"]).success();
    for skills_root in SKILL_ROOTS {
        let copy = root.join(skills_root).join("deploy");
        assert_eq!(
            fs::read_to_string(copy.join("scripts/run.sh")).unwrap(),
            "echo deploy\n",
            "{skills_root}"
        );
        assert_eq!(
            fs::read_to_string(copy.join("references/env.md")).unwrap(),
            "# Env\n",
            "{skills_root}"
        );
    }
    run(root, &["check"]).success();

    fs::remove_file(skill.join("references/env.md")).unwrap();
    run(root, &["check"]).failure();
    run(root, &["sync"]).success();
    for skills_root in SKILL_ROOTS {
        let copy = root.join(skills_root).join("deploy");
        assert!(!copy.join("references/env.md").exists(), "{skills_root}");
        assert!(copy.join("scripts/run.sh").exists(), "{skills_root}");
    }
    run(root, &["check"]).success();
}

#[test]
fn test_a_skill_removed_from_the_source_leaves_every_copy() {
    // conforme marks the copies it generates; a marked copy whose skill left
    // the source is deleted, a skill written by hand in a tool is kept.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    for name in ["deploy", "review"] {
        let skill = root.join(".claude/skills").join(name);
        fs::create_dir_all(skill.join("scripts")).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {name}\n---\nDo {name}.\n"),
        )
        .unwrap();
        fs::write(skill.join("scripts/run.sh"), "echo\n").unwrap();
    }
    fs::write(root.join("CLAUDE.md"), "Be helpful.\n").unwrap();
    fs::write(root.join(".conformerc.toml"), "source = \"claude\"\n").unwrap();
    for tool_dir in [
        ".cursor",
        ".devin",
        ".codex",
        ".opencode",
        ".roo",
        ".gemini",
        ".kiro",
        ".dsh",
    ] {
        fs::create_dir_all(root.join(tool_dir)).unwrap();
    }
    fs::create_dir_all(root.join(".github/skills")).unwrap();
    let own = root.join(".cursor/skills/my-own");
    fs::create_dir_all(&own).unwrap();
    fs::write(
        own.join("SKILL.md"),
        "---\nname: my-own\ndescription: Mine\n---\nMine.\n",
    )
    .unwrap();

    run(root, &["sync"]).success();
    for skills_root in SKILL_ROOTS {
        assert!(
            root.join(skills_root).join("review/.conforme").is_file(),
            "{skills_root}"
        );
    }

    fs::remove_dir_all(root.join(".claude/skills/review")).unwrap();
    run(root, &["check"])
        .failure()
        .stdout(predicate::str::contains("review"));
    run(root, &["sync"]).success();
    run(root, &["check"]).success();

    for skills_root in SKILL_ROOTS {
        assert!(
            !root.join(skills_root).join("review").exists(),
            "{skills_root}"
        );
        assert!(
            root.join(skills_root).join("deploy/SKILL.md").exists(),
            "{skills_root}"
        );
    }
    assert_eq!(
        fs::read_to_string(own.join("SKILL.md")).unwrap(),
        "---\nname: my-own\ndescription: Mine\n---\nMine.\n"
    );
}

#[test]
fn test_marked_skills_in_the_sources_own_directory_are_kept() {
    // With Codex as the source, `.agents/skills` is its config: Zed shares
    // the directory, and the copies an earlier sync marked there are now
    // the source's skills, never orphans.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".codex")).unwrap();
    fs::write(root.join(".rules"), "").unwrap();
    fs::write(root.join("AGENTS.md"), "Be helpful.\n").unwrap();
    let skill = root.join(".agents/skills/deploy");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: deploy\ndescription: Deploy\n---\nRun.\n",
    )
    .unwrap();
    fs::write(skill.join(".conforme"), "generated earlier\n").unwrap();
    fs::write(root.join(".conformerc.toml"), "source = \"codex\"\n").unwrap();

    run(root, &["sync"]).success();
    run(root, &["check"]).success();

    assert!(skill.join("SKILL.md").exists());
}

#[test]
fn test_migrate_refuses_an_empty_source() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".roo")).unwrap();

    run(
        root,
        &["migrate", "--source", "zoocode", "--output", "claude"],
    )
    .failure()
    .stderr(predicate::str::contains("nothing to migrate"));
}

#[test]
fn test_migrate_keeps_files_conforme_never_reads() {
    // Kiro `.json` agents and Zoo Code `.txt` rules are not read by conforme:
    // migrating away must not delete them along with the generated files.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".kiro/agents")).unwrap();
    fs::create_dir_all(root.join(".kiro/steering")).unwrap();
    fs::write(root.join(".kiro/steering/style.md"), "Use tabs.\n").unwrap();
    fs::write(
        root.join(".kiro/agents/reviewer.md"),
        "---\nname: reviewer\ndescription: Review\n---\nReview.\n",
    )
    .unwrap();
    fs::write(
        root.join(".kiro/agents/json-agent.json"),
        "{\"name\": \"j\"}",
    )
    .unwrap();

    migrate(root, "kiro", "claude");

    assert!(!root.join(".kiro/agents/reviewer.md").exists());
    assert_eq!(
        fs::read_to_string(root.join(".kiro/agents/json-agent.json")).unwrap(),
        "{\"name\": \"j\"}"
    );

    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".roo/rules")).unwrap();
    fs::write(root.join(".roo/rules/01-style.md"), "Use tabs.\n").unwrap();
    fs::write(root.join(".roo/rules/notes.txt"), "Plain notes.\n").unwrap();

    migrate(root, "zoocode", "claude");

    assert!(!root.join(".roo/rules/01-style.md").exists());
    assert_eq!(
        fs::read_to_string(root.join(".roo/rules/notes.txt")).unwrap(),
        "Plain notes.\n"
    );
}

#[test]
fn test_opencode_env_references_reach_other_tools_in_their_syntax() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("AGENTS.md"), "Be helpful.\n").unwrap();
    fs::create_dir_all(root.join(".cursor")).unwrap();
    fs::write(
        root.join("opencode.json"),
        r#"{"mcp": {"api": {"type": "remote", "url": "https://e.x/mcp", "headers": {"Authorization": "Bearer {env:GH_TOKEN}"}}}}"#,
    )
    .unwrap();

    conforme()
        .args(["-C", root.to_str().unwrap(), "sync", "--from", "opencode"])
        .assert()
        .success();
    let cursor: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(".cursor/mcp.json")).unwrap()).unwrap();
    assert_eq!(
        cursor["mcpServers"]["api"]["headers"]["Authorization"],
        "Bearer ${env:GH_TOKEN}"
    );
}

#[test]
fn test_orphan_cleanup_keeps_agent_drafts_and_docs() {
    let agents_md = "# Instructions\nBe helpful.\n\n## Agent: reviewer\n<!-- description: Review -->\nReview.\n";
    let dir = create_project_with_tools(agents_md, &["claude", "gemini"]);
    let root = dir.path();
    fs::create_dir_all(root.join(".claude/agents")).unwrap();
    fs::create_dir_all(root.join(".gemini/agents")).unwrap();
    fs::write(root.join(".claude/agents/README.md"), "# Our agents\n").unwrap();
    fs::write(
        root.join(".claude/agents/draft.md"),
        "---\nname: draft\n---\nNot ready.\n",
    )
    .unwrap();
    fs::write(
        root.join(".claude/agents/old.md"),
        "---\nname: old\ndescription: Old\n---\nOld.\n",
    )
    .unwrap();
    fs::write(
        root.join(".gemini/agents/_draft.md"),
        "---\nname: draft\ndescription: d\n---\nNot ready.\n",
    )
    .unwrap();

    conforme()
        .args(["-C", root.to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(root.join(".claude/agents/README.md").exists());
    assert!(root.join(".claude/agents/draft.md").exists());
    assert!(root.join(".gemini/agents/_draft.md").exists());
    // A real agent the source no longer has is still cleaned.
    assert!(!root.join(".claude/agents/old.md").exists());
    assert!(root.join(".claude/agents/reviewer.md").exists());
}

const GEMINI_REMOTE_AGENT: &str =
    "---\nkind: remote\nname: remote-helper\nagent_card_url: https://agents.example.com/card.json\n---\n";
const GEMINI_REMOTE_AGENTS_LIST: &str =
    "---\n- kind: remote\n  name: a\n  agent_card_url: https://a.example.com/card.json\n- kind: remote\n  name: b\n  agent_card_url: https://b.example.com/card.json\n---\n";

#[test]
fn test_gemini_remote_agents_survive_a_sync() {
    // Remote (A2A) agents have no portable form: conforme must not sweep them
    // as orphans when Gemini is a target.
    let agents_md = "# Instructions\nBe helpful.\n\n## Agent: reviewer\n<!-- description: Review -->\nReview.\n";
    let dir = create_project_with_tools(agents_md, &["gemini"]);
    let root = dir.path();
    fs::create_dir_all(root.join(".gemini/agents")).unwrap();
    fs::write(
        root.join(".gemini/agents/remote-helper.md"),
        GEMINI_REMOTE_AGENT,
    )
    .unwrap();
    fs::write(
        root.join(".gemini/agents/fleet.md"),
        GEMINI_REMOTE_AGENTS_LIST,
    )
    .unwrap();
    // `kind` defaults to `remote` on a remote agent: one carrying only an
    // agent card is remote too.
    let implicit = "---\nname: card-only\nagent_card_url: https://c.example.com/card.json\n---\n";
    fs::write(root.join(".gemini/agents/card-only.md"), implicit).unwrap();

    conforme()
        .args(["-C", root.to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(root.join(".gemini/agents/card-only.md")).unwrap(),
        implicit
    );
    assert_eq!(
        fs::read_to_string(root.join(".gemini/agents/remote-helper.md")).unwrap(),
        GEMINI_REMOTE_AGENT
    );
    assert_eq!(
        fs::read_to_string(root.join(".gemini/agents/fleet.md")).unwrap(),
        GEMINI_REMOTE_AGENTS_LIST
    );
    assert!(root.join(".gemini/agents/reviewer.md").exists());
}

#[test]
fn test_gemini_remote_agents_are_not_read_as_local_agents() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("GEMINI.md"), "Be helpful.\n").unwrap();
    fs::create_dir_all(root.join(".gemini/agents")).unwrap();
    fs::create_dir_all(root.join(".claude")).unwrap();
    fs::write(
        root.join(".gemini/agents/remote-helper.md"),
        GEMINI_REMOTE_AGENT,
    )
    .unwrap();
    fs::write(
        root.join(".gemini/agents/fleet.md"),
        GEMINI_REMOTE_AGENTS_LIST,
    )
    .unwrap();
    fs::write(
        root.join(".gemini/agents/reviewer.md"),
        "---\nname: reviewer\ndescription: Review\n---\nReview.\n",
    )
    .unwrap();

    conforme()
        .args(["-C", root.to_str().unwrap(), "sync", "--from", "gemini"])
        .assert()
        .success();

    assert!(root.join(".claude/agents/reviewer.md").exists());
    assert!(!root.join(".claude/agents/remote-helper.md").exists());
    assert!(!root.join(".claude/agents/fleet.md").exists());
}

const CLAUDE_SOURCE_WITH_EVERYTHING: &[(&str, &str)] = &[
    ("CLAUDE.md", "# Project\n\nUse pnpm.\n"),
    (
        ".claude/rules/ts.md",
        "---\npaths:\n  - \"**/*.ts\"\n---\nUse strict TypeScript.\n",
    ),
    (
        ".claude/skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Draft release notes\ndisable-model-invocation: true\n---\nList merged PRs.\n",
    ),
    (
        ".claude/agents/reviewer.md",
        "---\nname: reviewer\ndescription: Reviews code\n---\nReview the diff.\n",
    ),
    (
        ".mcp.json",
        "{\"mcpServers\":{\"fs\":{\"type\":\"stdio\",\"command\":\"npx\",\"args\":[\"fs\"]}}}",
    ),
    (".conformerc.toml", "source = \"claude\"\n"),
];

fn claude_source_with_everything(tools: &[&str]) -> TempDir {
    let dir = TempDir::new().unwrap();
    for (path, content) in CLAUDE_SOURCE_WITH_EVERYTHING {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    for tool in tools {
        fs::create_dir_all(dir.path().join(tool)).unwrap();
    }
    dir
}

#[test]
fn test_generated_agents_md_holds_only_instructions_and_rules() {
    // Codex, OpenCode, Kilo, Vibe, DeepSeek (and Cursor, Copilot, Zoo Code,
    // Kiro, Devin) load AGENTS.md as instructions: a skill body there is
    // always in the model's context, a manual one included, and Codex reads
    // only 32 KiB of it. Each tool gets skills, agents and MCP servers in
    // its own files.
    let dir = claude_source_with_everything(&[".codex", ".opencode"]);
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let agents_md = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("Use pnpm."), "{agents_md}");
    assert!(agents_md.contains("## Rule: ts"), "{agents_md}");
    for section in ["## Skill:", "## Agent:", "## MCP:", "List merged PRs"] {
        assert!(!agents_md.contains(section), "{section} in {agents_md}");
    }
    assert!(dir.path().join(".agents/skills/notes/SKILL.md").exists());
    assert!(dir.path().join(".opencode/agents/reviewer.md").exists());
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .success();
}

#[test]
fn test_migrated_agents_md_keeps_only_what_the_output_cannot_hold() {
    // Codex holds skills (`.agents/skills`) and MCP (`.codex/config.toml`)
    // but no agent: the agent section stays in AGENTS.md so a later switch
    // still has it.
    let dir = claude_source_with_everything(&[]);
    conforme()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "migrate",
            "--source",
            "claude",
            "--output",
            "codex",
        ])
        .assert()
        .success();

    let agents_md = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("Use pnpm."), "{agents_md}");
    assert!(agents_md.contains("## Agent: reviewer"), "{agents_md}");
    assert!(!agents_md.contains("## Skill:"), "{agents_md}");
    assert!(!agents_md.contains("## MCP:"), "{agents_md}");
    assert!(dir.path().join(".agents/skills/notes/SKILL.md").exists());
    assert!(fs::read_to_string(dir.path().join(".codex/config.toml"))
        .unwrap()
        .contains("[mcp_servers.fs]"));
}

/// Changing `source` by hand would rewrite every tool from a source that
/// holds less (Codex has no agents; the generated AGENTS.md has only
/// instructions and rules) and delete the old source's own agents: sync
/// refuses before touching anything and points to `migrate`.
#[test]
fn test_sync_refuses_a_source_changed_by_hand() {
    for new_source in ["source = \"codex\"\n", ""] {
        let dir = claude_source_with_everything(&[".codex"]);
        let root = dir.path();
        conforme()
            .args(["-C", root.to_str().unwrap(), "sync"])
            .assert()
            .success();
        let agents_md = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(
            agents_md.starts_with("<!-- Generated by conforme from claude"),
            "{agents_md}"
        );

        fs::write(root.join(".conformerc.toml"), new_source).unwrap();
        for command in ["sync", "check"] {
            conforme()
                .args(["-C", root.to_str().unwrap(), command])
                .assert()
                .failure()
                .stderr(predicate::str::contains("source = \"claude\""));
        }

        assert!(root.join(".claude/agents/reviewer.md").exists());
        assert!(root.join(".agents/skills/notes/SKILL.md").exists());
        assert_eq!(
            fs::read_to_string(root.join("AGENTS.md")).unwrap(),
            agents_md
        );
    }
}

#[test]
fn test_source_changed_after_migrate_is_accepted() {
    // `migrate` moves the AGENTS.md marker to the output, so setting the new
    // source afterwards is not taken for a switch by hand.
    let dir = claude_source_with_everything(&[".cursor"]);
    let root = dir.path();
    conforme()
        .args(["-C", root.to_str().unwrap(), "sync"])
        .assert()
        .success();
    conforme()
        .args([
            "-C",
            root.to_str().unwrap(),
            "migrate",
            "--source",
            "claude",
            "--output",
            "cursor",
        ])
        .assert()
        .success();
    fs::write(root.join(".conformerc.toml"), "source = \"cursor\"\n").unwrap();
    conforme()
        .args(["-C", root.to_str().unwrap(), "sync"])
        .assert()
        .success();
    assert!(fs::read_to_string(root.join("AGENTS.md"))
        .unwrap()
        .starts_with("<!-- Generated by conforme from cursor"));
}

#[test]
fn test_agent_leaving_the_source_leaves_opencode_json_too() {
    // conforme writes an agent both as `.opencode/agents/<name>.md` and as
    // `agent.<name>` in opencode.json; only the file was cleaned, so OpenCode
    // still loaded the agent. An entry the user extended (`permission`) or
    // wrote without a markdown file stays.
    let dir = claude_source_with_everything(&[".opencode"]);
    let root = dir.path();
    conforme()
        .args(["-C", root.to_str().unwrap(), "sync"])
        .assert()
        .success();
    let json_path = root.join(".opencode/opencode.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&json_path).unwrap()).unwrap();
    json["agent"]["mine"] =
        serde_json::json!({"description": "Mine", "mode": "subagent", "prompt": "Mine."});
    fs::write(&json_path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

    fs::remove_file(root.join(".claude/agents/reviewer.md")).unwrap();
    conforme()
        .args(["-C", root.to_str().unwrap(), "sync"])
        .assert()
        .success();

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&json_path).unwrap()).unwrap();
    assert!(json["agent"].get("reviewer").is_none(), "{json}");
    assert!(json["agent"].get("mine").is_some(), "{json}");
    assert!(!root.join(".opencode/agents/reviewer.md").exists());
    conforme()
        .args(["-C", root.to_str().unwrap(), "check"])
        .assert()
        .success();
}

#[test]
fn test_sync_warns_when_codex_would_truncate_agents_md() {
    // Codex reads `project_doc_max_bytes` = 32768 bytes of instruction files
    // and truncates the rest (codex-rs/config/defaults.toml, core agents_md.rs).
    let big = format!("# Instructions\n{}\n", "Keep this rule. ".repeat(2200));
    let dir = create_project_with_tools(&big, &["codex"]);
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success()
        .stderr(predicate::str::contains("32 KiB"));

    let small = create_project_with_tools("# Instructions\nShort.\n", &["codex"]);
    conforme()
        .args(["-C", small.path().to_str().unwrap(), "sync"])
        .assert()
        .success()
        .stderr(predicate::str::contains("32 KiB").not());
}

#[test]
fn test_long_skill_description_warning_names_the_tools_that_skip_it() {
    // Zoo Code and Mistral Vibe skip a skill whose description is over 1024
    // characters (Zoo SkillsManager.ts: "must be 1-1024 characters", Vibe
    // skills/models.py `max_length=1024`); Codex 0.162.0 checks
    // only the name length (codex-rs/skills/src/parser.rs).
    let agents_md = format!(
        "# Instructions\nGlobal.\n\n## Skill: deploy\n<!-- description: {} -->\nRun deploy.\n",
        "d".repeat(1025)
    );
    let dir = create_project_with_tools(&agents_md, &["zoocode", "codex"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Zoo Code and Mistral Vibe skip it",
        ))
        .stderr(predicate::str::contains("Codex").not());
}
