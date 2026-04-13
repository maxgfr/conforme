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
fn test_sync_requires_agents_md() {
    let dir = TempDir::new().unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No AGENTS.md found"));
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
fn test_check_requires_agents_md() {
    let dir = TempDir::new().unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No AGENTS.md found"));
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
