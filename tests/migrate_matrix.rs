//! Switching tools: every `migrate --source X --output Y` pair, X and Y among
//! the 12 adapters. The source tool holds a full config in its own format;
//! after the migration the output tool must read back everything it can hold.

use assert_cmd::Command;
use conforme::adapters::{all_adapters, AiToolAdapter};
use conforme::config::{
    sanitize_name, ActivationMode, McpTransport, NormalizedAgent, NormalizedConfig,
    NormalizedMcpServer, NormalizedRule, NormalizedSkill,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn full_config() -> NormalizedConfig {
    NormalizedConfig {
        instructions: "Project instructions for the switch matrix.".to_string(),
        rules: vec![
            NormalizedRule {
                name: "security".to_string(),
                content: "Never log secrets.".to_string(),
                activation: ActivationMode::Always,
            },
            NormalizedRule {
                name: "typescript".to_string(),
                content: "Use strict TypeScript.".to_string(),
                activation: ActivationMode::GlobMatch(vec!["src/**/*.ts".to_string()]),
            },
        ],
        skills: vec![
            NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy the application".to_string(),
                content: "Run scripts/deploy.sh, then check references/env.md.".to_string(),
                files: BTreeMap::from([
                    ("scripts/deploy.sh".to_string(), "echo deploy\n".to_string()),
                    ("references/env.md".to_string(), "# Env\nPROD\n".to_string()),
                ]),
                ..Default::default()
            },
            NormalizedSkill {
                name: "release".to_string(),
                description: "Cut a release".to_string(),
                content: "Tag and publish.".to_string(),
                manual_invocation: true,
                ..Default::default()
            },
        ],
        agents: vec![NormalizedAgent {
            name: "reviewer".to_string(),
            description: "Reviews diffs for bugs".to_string(),
            content: "Review the diff.".to_string(),
            tools: vec!["Read".to_string(), "Grep".to_string()],
            ..Default::default()
        }],
        mcp_servers: vec![
            NormalizedMcpServer {
                name: "files".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
                },
                env: BTreeMap::from([("TOKEN".to_string(), "${TOKEN}".to_string())]),
            },
            NormalizedMcpServer {
                name: "remote".to_string(),
                transport: McpTransport::Http {
                    url: "https://example.com/mcp".to_string(),
                    headers: BTreeMap::from([(
                        "Authorization".to_string(),
                        "Bearer ${API_TOKEN}".to_string(),
                    )]),
                },
                env: BTreeMap::new(),
            },
        ],
    }
}

/// Write `config` in `tool`'s own format, as a project using only that tool.
fn set_up_source(root: &Path, tool: &dyn AiToolAdapter, config: &NormalizedConfig) {
    tool.write(root, config).unwrap();
    if tool.reads_agents_md(root) {
        fs::write(
            root.join("AGENTS.md"),
            conforme::markdown::export_as_agents_md(&NormalizedConfig {
                instructions: config.instructions.clone(),
                rules: config.rules.clone(),
                ..Default::default()
            }),
        )
        .unwrap();
    }
    assert!(
        tool.detect(root),
        "{} not detected after writing",
        tool.id()
    );
}

/// Instructions and rule bodies as one text: a tool without per-rule files
/// merges the rules into its instructions.
fn all_text(config: &NormalizedConfig) -> String {
    let mut text = config.instructions.clone();
    for rule in &config.rules {
        text.push('\n');
        text.push_str(&rule.content);
    }
    text
}

fn compare(
    source: &NormalizedConfig,
    output: &NormalizedConfig,
    out: &dyn AiToolAdapter,
) -> Vec<String> {
    let mut problems = Vec::new();
    let text = all_text(output);
    for piece in
        std::iter::once(&source.instructions).chain(source.rules.iter().map(|r| &r.content))
    {
        if !piece.trim().is_empty() && !text.contains(piece.trim()) {
            problems.push(format!("text lost: {:?}", piece.trim()));
        }
    }
    let caps = out.capabilities();
    if caps.activation_modes {
        for rule in source
            .rules
            .iter()
            .filter(|r| matches!(r.activation, ActivationMode::GlobMatch(_)))
        {
            let kept = output
                .rules
                .iter()
                .find(|r| r.content.trim() == rule.content.trim())
                .is_some_and(|r| r.activation == rule.activation);
            if !kept {
                problems.push(format!("rule {}: glob scope lost", rule.name));
            }
        }
    }
    if caps.skills {
        for skill in &source.skills {
            let name = sanitize_name(&skill.name);
            match output
                .skills
                .iter()
                .find(|s| sanitize_name(&s.name) == name)
            {
                None => problems.push(format!("skill lost: {name}")),
                Some(found) => {
                    if found.content.trim() != skill.content.trim() {
                        problems.push(format!("skill {name}: body differs"));
                    }
                    if found.files != skill.files {
                        problems.push(format!(
                            "skill {name}: bundled files {:?} != {:?}",
                            found.files.keys().collect::<Vec<_>>(),
                            skill.files.keys().collect::<Vec<_>>()
                        ));
                    }
                    if found.description.trim() != skill.description.trim() {
                        problems.push(format!("skill {name}: description differs"));
                    }
                    if found.manual_invocation != skill.manual_invocation {
                        problems.push(format!("skill {name}: manual invocation lost"));
                    }
                }
            }
        }
    }
    if caps.agents {
        for agent in &source.agents {
            let name = sanitize_name(&agent.name);
            match output
                .agents
                .iter()
                .find(|a| sanitize_name(&a.name) == name)
            {
                None => problems.push(format!("agent lost: {name}")),
                Some(found) if found.content.trim() != agent.content.trim() => {
                    problems.push(format!("agent {name}: prompt differs"))
                }
                Some(_) => {}
            }
        }
    }
    if caps.mcp {
        for server in &source.mcp_servers {
            match output.mcp_servers.iter().find(|s| s.name == server.name) {
                None => problems.push(format!("MCP server lost: {}", server.name)),
                Some(found) if found.transport != server.transport => problems.push(format!(
                    "MCP server {}: {:?} != {:?}",
                    server.name, found.transport, server.transport
                )),
                Some(found) if found.env != server.env => problems.push(format!(
                    "MCP server {}: env {:?} != {:?}",
                    server.name, found.env, server.env
                )),
                Some(_) => {}
            }
        }
    }
    problems
}

/// Migrate along `chain` (first tool set up with the full config) and return
/// the config the first tool held and the one the last tool holds.
fn switch_along(chain: &[&str]) -> (NormalizedConfig, NormalizedConfig) {
    let adapters = all_adapters();
    let tool = |id: &str| adapters.iter().find(|a| a.id() == id).unwrap().as_ref();
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    set_up_source(root, tool(chain[0]), &full_config());
    let start = tool(chain[0]).read(root).unwrap();
    for pair in chain.windows(2) {
        Command::cargo_bin("conforme")
            .unwrap()
            .args([
                "-C",
                root.to_str().unwrap(),
                "migrate",
                "--source",
                pair[0],
                "--output",
                pair[1],
            ])
            .assert()
            .success();
    }
    let end = tool(chain[chain.len() - 1]).read(root).unwrap();
    (start, end)
}

/// Switching repeatedly through the tools that hold activation modes,
/// skills, agents and MCP, then back to the first one, changes nothing.
#[test]
fn test_switching_around_and_back_changes_nothing() {
    let (start, end) = switch_along(&["claude", "cursor", "copilot", "kiro", "claude"]);
    let problems = compare(&start, &end, all_adapters()[0].as_ref());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert_eq!(end.rules.len(), start.rules.len());
    assert_eq!(end.skills.len(), start.skills.len());
    assert_eq!(end.agents.len(), start.agents.len());
    assert_eq!(end.mcp_servers.len(), start.mcp_servers.len());
}

/// Through all 12 tools and back, the instructions, rule texts and skills
/// (bodies, bundled files, manual invocation) survive every hop; only what a
/// tool cannot hold (a rule's scope, agents, MCP) may be dropped on the way.
#[test]
fn test_switching_through_every_tool_keeps_text_and_skills() {
    let chain = [
        "claude", "gemini", "opencode", "codex", "zed", "amp", "deepseek", "zoocode", "devin",
        "copilot", "kiro", "cursor", "claude",
    ];
    let (start, end) = switch_along(&chain);
    let text_and_skills = |c: &NormalizedConfig| NormalizedConfig {
        instructions: c.instructions.clone(),
        rules: c
            .rules
            .iter()
            .map(|r| NormalizedRule {
                activation: ActivationMode::Always,
                ..r.clone()
            })
            .collect(),
        skills: c.skills.clone(),
        ..Default::default()
    };
    let problems = compare(&text_and_skills(&start), &end, all_adapters()[0].as_ref());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn test_migrating_between_any_two_tools_keeps_what_the_output_can_hold() {
    let adapters = all_adapters();
    let mut failures = Vec::new();
    for source_tool in &adapters {
        for output_tool in &adapters {
            if source_tool.id() == output_tool.id() {
                continue;
            }
            let dir = TempDir::new().unwrap();
            let root = dir.path();
            set_up_source(root, source_tool.as_ref(), &full_config());
            let held = source_tool.read(root).unwrap();
            // The source really holds the config: the comparison is not vacuous.
            assert_eq!(held.skills.len(), 2, "{} skills", source_tool.id());
            assert!(
                all_text(&held).contains("Never log secrets."),
                "{} text",
                source_tool.id()
            );
            // Agents or skills the output cannot hold stay with the source.
            let caps = output_tool.capabilities();
            let carried = NormalizedConfig {
                agents: if caps.agents {
                    held.agents.clone()
                } else {
                    Vec::new()
                },
                skills: if caps.skills {
                    held.skills.clone()
                } else {
                    Vec::new()
                },
                ..held.clone()
            };
            let source_files: Vec<_> = source_tool
                .generate(root, &carried)
                .unwrap()
                .into_iter()
                .map(|(path, _)| path)
                .filter(|path| !source_tool.is_shared_file(path))
                .collect();

            let run = Command::cargo_bin("conforme")
                .unwrap()
                .args([
                    "-C",
                    root.to_str().unwrap(),
                    "migrate",
                    "--source",
                    source_tool.id(),
                    "--output",
                    output_tool.id(),
                ])
                .output()
                .unwrap();
            let pair = format!("{} -> {}", source_tool.id(), output_tool.id());
            if !run.status.success() {
                failures.push(format!(
                    "{pair}: migrate failed: {}",
                    String::from_utf8_lossy(&run.stderr).trim()
                ));
                continue;
            }
            let got = output_tool.read(root).unwrap();
            for problem in compare(&held, &got, output_tool.as_ref()) {
                failures.push(format!("{pair}: {problem}"));
            }
            // The switch is complete: the source tool's own files are gone,
            // except those the output now owns (a shared skills directory).
            let output_files: Vec<_> = output_tool
                .generate(root, &got)
                .unwrap()
                .into_iter()
                .map(|(path, _)| path)
                .collect();
            for path in &source_files {
                let kept_for_output = output_files.contains(path)
                    || output_tool
                        .managed_directories(root)
                        .iter()
                        .any(|d| path.starts_with(&d.path));
                if path.exists() && !kept_for_output && !path.ends_with("AGENTS.md") {
                    failures.push(format!(
                        "{pair}: source file left behind: {}",
                        path.strip_prefix(root).unwrap().display()
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} problem(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
