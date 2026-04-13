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
            "windsurf" => fs::create_dir_all(dir.path().join(".windsurf")).unwrap(),
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
            "roocode" => fs::create_dir_all(dir.path().join(".roo")).unwrap(),
            "gemini" => fs::create_dir_all(dir.path().join(".gemini")).unwrap(),
            "continue" => fs::create_dir_all(dir.path().join(".continue")).unwrap(),
            "zed" => fs::write(dir.path().join(".rules"), "").unwrap(),
            "amazonq" => fs::create_dir_all(dir.path().join(".amazonq")).unwrap(),
            "kiro" => fs::create_dir_all(dir.path().join(".kiro")).unwrap(),
            "amp" => fs::create_dir_all(dir.path().join(".amp")).unwrap(),
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
fn test_sync_creates_windsurf_config() {
    let agents_md = r#"# Instructions
Be helpful.

## Rule: Testing
<!-- activation: glob **/*.test.ts -->

Write thorough tests.
"#;
    let dir = create_project_with_tools(agents_md, &["windsurf"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let general = dir.path().join(".windsurf/rules/general.md");
    assert!(general.exists());
    let content = fs::read_to_string(&general).unwrap();
    assert!(content.contains("trigger: always_on"));

    let testing = dir.path().join(".windsurf/rules/testing.md");
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
    let dir = create_project_with_tools(agents_md, &["cursor", "windsurf"]);

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
    // Windsurf should NOT
    assert!(!dir.path().join(".windsurf/rules/general.md").exists());
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
    let dir = create_project_with_tools(agents_md, &["cursor", "claude", "windsurf", "copilot"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Verify all tools got files
    assert!(dir.path().join("CLAUDE.md").exists());
    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    assert!(dir.path().join(".windsurf/rules/general.md").exists());
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
fn test_sync_creates_roocode_config() {
    let agents_md = r#"# Instructions
General rules.

## Rule: Testing
<!-- activation: always -->

Write tests.
"#;
    let dir = create_project_with_tools(agents_md, &["roocode"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".roo/rules/00-general.md").exists());
    assert!(dir.path().join(".roo/rules/01-testing.md").exists());
}

#[test]
fn test_sync_creates_continue_config() {
    let agents_md = r#"# Instructions
Be consistent.

## Rule: TypeScript
<!-- activation: glob **/*.ts -->

Use strict mode.
"#;
    let dir = create_project_with_tools(agents_md, &["continue"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    let general = dir.path().join(".continue/rules/general.md");
    assert!(general.exists());
    let content = fs::read_to_string(&general).unwrap();
    assert!(content.contains("alwaysApply: true"));

    let ts = dir.path().join(".continue/rules/typescript.md");
    assert!(ts.exists());
    let content = fs::read_to_string(&ts).unwrap();
    assert!(content.contains("globs:"));
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
fn test_sync_creates_amazonq_config() {
    let agents_md = r#"# Instructions
Follow AWS best practices.

## Rule: Security
<!-- activation: always -->

Use IAM roles.
"#;
    let dir = create_project_with_tools(agents_md, &["amazonq"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".amazonq/rules/general.md").exists());
    assert!(dir.path().join(".amazonq/rules/security.md").exists());
}

#[test]
fn test_sync_all_11_tools() {
    let agents_md = "# Instructions\nGlobal.\n";
    let dir = create_project_with_tools(
        agents_md,
        &[
            "cursor", "claude", "windsurf", "copilot", "codex", "opencode", "roocode", "gemini",
            "continue", "zed", "amazonq", "kiro", "amp",
        ],
    );

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join("CLAUDE.md").exists());
    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    assert!(dir.path().join(".windsurf/rules/general.md").exists());
    assert!(dir.path().join("GEMINI.md").exists());
    assert!(dir.path().join(".roo/rules/00-general.md").exists());
    assert!(dir.path().join(".continue/rules/general.md").exists());
    assert!(dir.path().join(".rules").exists());
    assert!(dir.path().join(".amazonq/rules/general.md").exists());
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

    // Copilot: prompts + agents + MCP
    assert!(dir.path().join(".github/prompts/deploy.prompt.md").exists());
    assert!(dir.path().join(".github/agents/reviewer.agent.md").exists());
    let agent = fs::read_to_string(dir.path().join(".github/agents/reviewer.agent.md")).unwrap();
    assert!(agent.contains("name: reviewer"));
    assert!(agent.contains("model: gpt-4o"));

    assert!(dir.path().join(".vscode/mcp.json").exists());
    let mcp = fs::read_to_string(dir.path().join(".vscode/mcp.json")).unwrap();
    assert!(mcp.contains("\"servers\""));

    // Codex: skills
    assert!(dir.path().join(".agents/skills/deploy/SKILL.md").exists());
}

// ===== Remove =====

#[test]
fn test_remove_deletes_tool_files() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor", "windsurf"]);

    // Sync first
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    assert!(dir.path().join(".windsurf/rules/general.md").exists());

    // Remove cursor only
    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "remove", "cursor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("removed"));

    // Cursor files should be gone
    assert!(!dir.path().join(".cursor/rules/general.mdc").exists());
    // Windsurf files should still exist
    assert!(dir.path().join(".windsurf/rules/general.md").exists());
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
            .and(predicate::str::contains("Windsurf"))
            .and(predicate::str::contains("Kiro"))
            .and(predicate::str::contains("Amp"))
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

    // Windsurf should have been synced
    let windsurf_general = dir.path().join(".windsurf/rules/general.md");
    assert!(windsurf_general.exists());
    let content = fs::read_to_string(&windsurf_general).unwrap();
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
    let dir = create_project_with_tools(agents_md, &["cursor", "windsurf"]);

    // Exclude windsurf
    fs::write(
        dir.path().join(".conformerc.toml"),
        "exclude = [\"windsurf\"]\n",
    )
    .unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Cursor should have files
    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    // Windsurf should NOT (excluded)
    assert!(!dir.path().join(".windsurf/rules/general.md").exists());
}

#[test]
fn test_conformerc_only() {
    let agents_md = "# Instructions\nHello.\n";
    let dir = create_project_with_tools(agents_md, &["cursor", "windsurf", "claude"]);

    // Only sync to cursor
    fs::write(dir.path().join(".conformerc.toml"), "only = [\"cursor\"]\n").unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".cursor/rules/general.mdc").exists());
    assert!(!dir.path().join(".windsurf/rules/general.md").exists());
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
            "cursor", "claude", "windsurf", "copilot", "kiro", "roocode", "amazonq", "continue",
            "gemini", "zed",
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
    assert!(dir.path().join(".cursor/agents/reviewer.mdc").exists());
    assert!(dir.path().join(".cursor/mcp.json").exists());

    // Copilot: instructions + prompts + agents + mcp
    assert!(dir.path().join(".github/copilot-instructions.md").exists());
    assert!(dir.path().join(".github/prompts/deploy.prompt.md").exists());
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

    // Roo Code: rules + mcp
    assert!(dir.path().join(".roo/rules/00-general.md").exists());
    assert!(dir.path().join(".roo/mcp.json").exists());

    // Windsurf: rules + mcp
    assert!(dir.path().join(".windsurf/rules/general.md").exists());
    assert!(dir.path().join(".windsurf/mcp.json").exists());

    // Continue: rules + mcp
    assert!(dir.path().join(".continue/rules/general.md").exists());
    assert!(dir.path().join(".continue/mcpServers/mcp.json").exists());

    // Zed: .rules + settings
    assert!(dir.path().join(".rules").exists());
    assert!(dir.path().join(".zed/settings.json").exists());

    // Amazon Q: rules + mcp
    assert!(dir.path().join(".amazonq/rules/general.md").exists());
    assert!(dir.path().join(".amazonq/mcp.json").exists());

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

    // Windsurf should have rules too
    let ws_general = fs::read_to_string(dir.path().join(".windsurf/rules/general.md")).unwrap();
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
