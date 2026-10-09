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
            "gemini" => fs::create_dir_all(dir.path().join(".gemini")).unwrap(),
            "opencode" => fs::create_dir_all(dir.path().join(".opencode")).unwrap(),
            "kilo" => fs::create_dir_all(dir.path().join(".kilo")).unwrap(),
            "vibe" => fs::create_dir_all(dir.path().join(".vibe")).unwrap(),
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

    let claude_skill = dir.path().join(".claude/skills/deploy/SKILL.md");
    assert!(claude_skill.exists());
    let claude_content = std::fs::read_to_string(&claude_skill).unwrap();
    assert!(claude_content.contains("name: deploy"));
    assert!(claude_content.contains("Run npm run deploy."));

    // Copilot skills live at .github/skills/<name>/SKILL.md
    let copilot_skill = dir.path().join(".github/skills/deploy/SKILL.md");
    assert!(copilot_skill.exists());
    let copilot_content = std::fs::read_to_string(&copilot_skill).unwrap();
    assert!(copilot_content.contains("name: deploy"));
    assert!(copilot_content.contains("description: Deploy the app"));
    assert!(copilot_content.contains("allowed-tools: Bash"));
    assert!(copilot_content.contains("Run npm run deploy."));
    assert!(!dir.path().join(".github/prompts").exists());
}

// ===== Empty instructions, no rules =====

#[test]
fn test_sync_empty_config() {
    let agents_md = "# Instructions\n";
    let dir = create_project_with_tools(agents_md, &["cursor", "devin"]);

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

// ===== Devin (formerly Windsurf) =====

#[test]
fn test_sync_migrates_a_legacy_windsurf_project_to_devin() {
    let agents_md = r#"# Instructions
Be helpful.

## Skill: deploy
<!-- description: Deploy -->
Run deploy.

## MCP: test-server
<!-- command: npx -->
<!-- args: -y, @test/server -->
<!-- env: TOKEN=${GH_TOKEN} -->
"#;
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("AGENTS.md"), agents_md).unwrap();
    // What an earlier conforme version wrote for the `windsurf` id, plus a
    // skill the user authored there by hand.
    fs::create_dir_all(root.join(".windsurf/rules")).unwrap();
    fs::write(
        root.join(".windsurf/rules/general.md"),
        "---\ntrigger: always_on\n---\nOld.\n",
    )
    .unwrap();
    for skill in ["deploy", "hand-made"] {
        let skill_dir = root.join(".windsurf/skills").join(skill);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: x\n---\nOld.\n").unwrap();
    }

    conforme()
        .args(["-C", root.to_str().unwrap(), "sync"])
        .assert()
        .success()
        .stderr(predicate::str::contains("does not support MCP").not());

    // Devin loads `.windsurf/` *and* `.devin/`: the old copies are gone so
    // nothing is applied twice, while the user's own skill is kept.
    let general = fs::read_to_string(root.join(".devin/rules/general.md")).unwrap();
    assert!(general.contains("Be helpful."));
    assert!(!root.join(".windsurf/rules/general.md").exists());
    assert!(root.join(".devin/skills/deploy/SKILL.md").exists());
    assert!(!root.join(".windsurf/skills/deploy").exists());
    assert!(root.join(".windsurf/skills/hand-made/SKILL.md").exists());

    // Devin Local reads project MCP servers from `.devin/mcp_config.json`,
    // with `${env:VAR}` references.
    let mcp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(".devin/mcp_config.json")).unwrap())
            .unwrap();
    assert_eq!(mcp["mcpServers"]["test-server"]["command"], "npx");
    assert_eq!(
        mcp["mcpServers"]["test-server"]["env"]["TOKEN"],
        "${env:GH_TOKEN}"
    );
}

#[test]
fn test_sync_copilot_preserves_user_prompt_files() {
    let agents_md = r#"# Instructions
Be helpful.

## Skill: deploy
<!-- description: Deploy the app -->

Run npm run deploy.
"#;
    let dir = create_project_with_tools(agents_md, &["copilot"]);

    // VS Code prompt files are a separate, user-authored feature. They live
    // next to the generated skills but must never be treated as orphans.
    let prompts_dir = dir.path().join(".github/prompts");
    fs::create_dir_all(&prompts_dir).unwrap();
    let user_prompt = prompts_dir.join("release-notes.prompt.md");
    fs::write(
        &user_prompt,
        "---\ndescription: Draft notes\n---\nWrite notes.\n",
    )
    .unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    assert!(dir.path().join(".github/skills/deploy/SKILL.md").exists());
    assert!(user_prompt.exists());
    assert_eq!(
        fs::read_to_string(&user_prompt).unwrap(),
        "---\ndescription: Draft notes\n---\nWrite notes.\n"
    );
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

    assert!(dir.path().join(".cursor/agents/reviewer.md").exists());
    let agent = fs::read_to_string(dir.path().join(".cursor/agents/reviewer.md")).unwrap();
    assert!(agent.contains("name: reviewer"));
    assert!(agent.contains("description: Code review agent"));
    assert!(agent.contains("model: gpt-4o"));
    // Cursor subagents do not recognize a `tools` frontmatter field.
    assert!(!agent.contains("tools:"));
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

    // OpenCode reads MCP from opencode.json under the `mcp` key (not a
    // standalone file); a new one goes to `.opencode/`, which Kilo never reads.
    let opencode_json = dir.path().join(".opencode/opencode.json");
    assert!(!dir.path().join("opencode.json").exists());
    assert!(opencode_json.exists());
    let content = fs::read_to_string(&opencode_json).unwrap();
    assert!(content.contains("\"mcp\""));
    assert!(content.contains("\"type\": \"local\""));
    assert!(content.contains("test-server"));
    // command is a single combined array, not separate command+args.
    assert!(content.contains("\"command\": [\n"));
    assert!(!content.contains("\"args\""));
}

#[test]
fn test_opencode_and_kilo_both_load_their_mcp_servers() {
    // Kilo loads the root opencode.json and refuses a project file holding
    // `{env:VAR}`: OpenCode's servers go to `.opencode/`, Kilo's carry none.
    let agents_md = r#"# Instructions
Be helpful.

## MCP: files
<!-- command: npx -->
<!-- args: -y, @test/server -->
<!-- env: TOKEN=${TOKEN} -->

## MCP: remote
<!-- url: https://example.com/mcp -->
<!-- headers: Authorization=Bearer ${API_TOKEN} -->
"#;
    let dir = create_project_with_tools(agents_md, &["opencode", "kilo"]);

    let output = conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&output);

    let opencode = fs::read_to_string(dir.path().join(".opencode/opencode.json")).unwrap();
    assert!(opencode.contains("{env:TOKEN}"), "{opencode}");
    assert!(!dir.path().join("opencode.json").exists());
    let kilo = fs::read_to_string(dir.path().join(".kilo/kilo.jsonc")).unwrap();
    assert!(!kilo.contains("{env:"), "{kilo}");
    assert!(!kilo.contains("TOKEN\""), "an inherited variable: {kilo}");
    assert!(
        stderr.contains("MCP server remote: Kilo resolves no variable"),
        "{stderr}"
    );
}

#[test]
fn test_kilo_warns_about_a_root_opencode_json_it_refuses() {
    let dir = create_project_with_tools("# Instructions\nBe helpful.\n", &["kilo"]);
    fs::write(
        dir.path().join("opencode.json"),
        r#"{"mcp":{"files":{"type":"local","command":["npx"],"environment":{"TOKEN":"{env:TOKEN}"}}}}"#,
    )
    .unwrap();

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync", "--only", "kilo"])
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "opencode.json holds {env:VAR} references, and Kilo refuses that file",
        ));
}

#[test]
fn test_only_the_skill_names_claude_code_reserves_warn() {
    // Claude Code skips `synced` and `anthropic-skills`; `claude-ai`, reserved
    // in 2.1.282 only, loads again.
    let agents_md = "# Instructions\nBe helpful.\n\n## Skill: synced\n<!-- description: A -->\nA.\n\n## Skill: claude-ai\n<!-- description: B -->\nB.\n";
    let dir = create_project_with_tools(agents_md, &["claude"]);
    let output = conforme()
        .args(["-C", dir.path().to_str().unwrap(), "check"])
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("Skill 'synced' uses a name Claude Code reserves"),
        "{text}"
    );
    assert!(!text.contains("Skill 'claude-ai' uses a name"), "{text}");
}

#[test]
fn test_sync_agents_to_opencode() {
    let agents_md = r#"# Instructions
Be helpful.

## Agent: reviewer
<!-- description: Code review agent -->
<!-- model: openai/gpt-4o -->

Review all changes for bugs.
"#;
    let dir = create_project_with_tools(agents_md, &["opencode"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Agents go into opencode.json under the `agent` key AND per-project markdown.
    let opencode_json = dir.path().join(".opencode/opencode.json");
    assert!(opencode_json.exists());
    let json_content = fs::read_to_string(&opencode_json).unwrap();
    assert!(json_content.contains("\"agent\""));
    assert!(json_content.contains("\"reviewer\""));
    assert!(json_content.contains("\"mode\": \"subagent\""));
    assert!(json_content.contains("\"model\": \"openai/gpt-4o\""));

    let md_agent = dir.path().join(".opencode/agents/reviewer.md");
    assert!(md_agent.exists());
    let md = fs::read_to_string(&md_agent).unwrap();
    assert!(md.contains("description: Code review agent"));
    assert!(md.contains("mode: subagent"));
    assert!(md.contains("model: openai/gpt-4o"));
    assert!(md.contains("Review all changes for bugs."));
}

/// Gemini CLI rejects a whole subagent whose `tools` holds a name it does not
/// know, and Kiro only accepts its own tags. Claude/Copilot tool names are
/// translated, and names with no equivalent are dropped.
#[test]
fn test_sync_agent_tools_use_target_vocabulary() {
    let agents_md = r#"# Instructions
Be helpful.

## Agent: reviewer
<!-- description: Code review agent -->
<!-- tools: Read, Grep, Bash, codebase, mcp__github__list_issues, NoSuchTool -->
<!-- model: sonnet -->

Review all changes for bugs.
"#;
    let dir = create_project_with_tools(agents_md, &["gemini", "kiro"]);

    conforme()
        .args(["-C", dir.path().to_str().unwrap(), "sync"])
        .assert()
        .success();

    // Gemini rejects `mcp__…` (empty server component) and unknown names, so
    // the Claude spelling is rewritten to `mcp_<server>_<tool>`.
    let gemini = fs::read_to_string(dir.path().join(".gemini/agents/reviewer.md")).unwrap();
    assert!(
        gemini.contains(
            "tools:\n- read_file\n- grep_search\n- run_shell_command\n- mcp_github_list_issues\n"
        ),
        "{gemini}"
    );
    assert!(!gemini.contains("codebase"));
    assert!(!gemini.contains("NoSuchTool"));
    // Gemini would send `sonnet` to its own API.
    assert!(!gemini.contains("model:"), "{gemini}");

    let kiro = fs::read_to_string(dir.path().join(".kiro/agents/reviewer.md")).unwrap();
    assert!(
        kiro.contains("tools:\n- read\n- grep\n- shell\n- '@github/list_issues'\n"),
        "{kiro}"
    );
    assert!(!kiro.contains("Bash"));
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
