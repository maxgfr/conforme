use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
use crate::config::{
    rule_file_name, ActivationMode, NormalizedAgent, NormalizedConfig, NormalizedRule,
    NormalizedSkill,
};
use crate::frontmatter;

/// Parse a frontmatter tool list that may be either a scalar string
/// (space- or comma-separated, e.g. `Read, Bash Write`) or a YAML sequence
/// (`- Read` / `- Bash`). Claude Code accepts all of these forms for the
/// `tools` (subagents) and `allowed-tools` (skills/commands) fields, so the
/// reader must handle every one to round-trip correctly.
fn parse_tool_list(value: Option<&serde_yaml_ng::Value>) -> Vec<String> {
    match value {
        Some(serde_yaml_ng::Value::String(s)) => s
            .split_whitespace()
            .flat_map(|t| t.split(','))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        Some(serde_yaml_ng::Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

pub struct ClaudeAdapter;

/// Whether a file is one Claude Code loads as a subagent: it needs a
/// non-empty `name` and `description` (a file without `name` is documentation
/// kept beside the agents, one without `description` is skipped).
fn is_claude_agent_file(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| frontmatter::parse(&content).ok())
        .is_some_and(|(fields, _)| {
            ["name", "description"].iter().all(|key| {
                fields
                    .get(*key)
                    .and_then(|v| v.as_str())
                    .is_some_and(|v| !v.trim().is_empty())
            })
        })
}

/// Resolve the project instruction file.
///
/// Claude Code accepts a project `CLAUDE.md` at either `./CLAUDE.md` or
/// `./.claude/CLAUDE.md`. conforme uses the root file by default, but honours an
/// existing `.claude/CLAUDE.md` when no root file is present — otherwise reading
/// such a project as the sync source lost the instructions entirely, and writing
/// to it created a second instruction file that Claude Code would load on top of
/// the first.
fn claude_md_path(project_root: &Path) -> PathBuf {
    let root = project_root.join("CLAUDE.md");
    if root.exists() {
        return root;
    }
    let nested = project_root.join(".claude").join("CLAUDE.md");
    if nested.exists() {
        return nested;
    }
    root
}

/// `AGENTS.md` files Claude Code loads when the project has no `CLAUDE.md`,
/// `.claude/CLAUDE.md` nor `CLAUDE.local.md`.
const AGENTS_MD_FALLBACKS: &[&str] = &["AGENTS.md", ".claude/AGENTS.md"];

/// Whether Claude Code reads `AGENTS.md` in this project: no `CLAUDE.md`
/// exists, and an `AGENTS.md` does. A personal `CLAUDE.local.md` also turns
/// Claude Code's fallback off, but conforme never reads that file: the shared
/// `AGENTS.md` stays the source rather than being regenerated over.
fn reads_agents_md_fallback(project_root: &Path) -> bool {
    let no_claude_md = ["CLAUDE.md", ".claude/CLAUDE.md"]
        .iter()
        .all(|name| !project_root.join(name).exists());
    no_claude_md
        && AGENTS_MD_FALLBACKS
            .iter()
            .any(|name| project_root.join(name).is_file())
}

impl AiToolAdapter for ClaudeAdapter {
    fn name(&self) -> &str {
        "Claude Code"
    }

    fn id(&self) -> &str {
        "claude"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("CLAUDE.md").exists() || project_root.join(".claude").is_dir()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: true,
            skills: true,
            agents: true,
            mcp: true,
        }
    }

    /// `.mcp.json` is merged, not owned: it also holds per-server settings
    /// conforme never writes, and Copilot CLI reads it too.
    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".mcp.json"))
    }

    fn reads_agents_md(&self, project_root: &Path) -> bool {
        reads_agents_md_fallback(project_root)
    }

    fn source_files(&self, project_root: &Path) -> Vec<PathBuf> {
        if reads_agents_md_fallback(project_root) {
            AGENTS_MD_FALLBACKS
                .iter()
                .map(|name| project_root.join(name))
                .filter(|path| path.is_file())
                .collect()
        } else {
            let claude_md = claude_md_path(project_root);
            if claude_md.is_file() {
                vec![claude_md]
            } else {
                Vec::new()
            }
        }
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![
            ManagedDir::files(project_root.join(".claude").join("rules"), ".md"),
            ManagedDir::subdirs(project_root.join(".claude").join("skills")),
            // A `.md` without `name` and `description` there is not an agent
            // Claude Code loads (a README, a draft), so it is never one of
            // conforme's orphans.
            ManagedDir::files_except(project_root.join(".claude").join("agents"), ".md", |path| {
                !is_claude_agent_file(path)
            }),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let claude_md = claude_md_path(project_root);
        let instructions = if claude_md.exists() {
            std::fs::read_to_string(&claude_md)
                .with_context(|| format!("failed to read {}", claude_md.display()))?
                .trim()
                .to_string()
        } else {
            String::new()
        };

        let mut rules = Vec::new();
        let rules_dir = project_root.join(".claude").join("rules");
        // `.claude/rules/` is discovered recursively by Claude Code.
        for path in crate::adapters::collect_rule_files(&rules_dir, "md")? {
            let content = std::fs::read_to_string(&path)?;
            let (fields, body) = frontmatter::parse(&content)?;
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // `paths` is a YAML list or a comma-separated string; a rule
            // without it always loads.
            // Brace groups (`src/**/*.{ts,tsx}`, the form Claude Code
            // recommends) keep their inner commas.
            let globs = crate::config::yaml_globs(fields.get("paths"));
            let activation = if globs.is_empty() {
                ActivationMode::Always
            } else {
                ActivationMode::GlobMatch(globs)
            };

            rules.push(NormalizedRule {
                name,
                content: body.trim().to_string(),
                activation,
            });
        }

        // Read skills from .claude/skills/<name>/SKILL.md
        let mut skills = Vec::new();
        let skills_dir = project_root.join(".claude").join("skills");
        if skills_dir.is_dir() {
            let mut entries: Vec<_> = std::fs::read_dir(&skills_dir)?
                .filter_map(|e| e.ok())
                .collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let skill_dir = entry.path();
                if skill_dir.is_dir() {
                    let skill_file = skill_dir.join("SKILL.md");
                    if skill_file.exists() {
                        let content = std::fs::read_to_string(&skill_file)?;
                        let (fields, body) = frontmatter::parse(&content)?;
                        let name = fields
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or_else(|| skill_dir.file_name().unwrap().to_str().unwrap())
                            .to_string();
                        let description = fields
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let allowed_tools = parse_tool_list(fields.get("allowed-tools"));
                        skills.push(NormalizedSkill {
                            name,
                            description,
                            content: body.trim().to_string(),
                            allowed_tools,
                            manual_invocation: crate::skills::read_manual_invocation(
                                &fields, &skill_dir,
                            )?,
                            files: crate::skills::read_bundled_files(&skill_dir)?,
                        });
                    }
                }
            }
        }

        // Read commands from .claude/commands/**/*.md (mapped to skills for
        // cross-tool sync). A nested command is namespaced by its folders:
        // `frontend/component.md` is `/frontend:component`.
        let commands_dir = project_root.join(".claude").join("commands");
        let mut command_paths = crate::adapters::collect_rule_files(&commands_dir, "md")?;
        command_paths.sort();
        for path in command_paths {
            let content = std::fs::read_to_string(&path)?;
            let (fields, body) = frontmatter::parse(&content)?;
            let name = path
                .strip_prefix(&commands_dir)
                .unwrap_or(&path)
                .with_extension("")
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(":");
            // A skill and a command of the same name: Claude Code runs the skill.
            if skills.iter().any(|s| s.name == name) {
                continue;
            }
            let description = fields
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let allowed_tools = parse_tool_list(fields.get("allowed-tools"));
            skills.push(NormalizedSkill {
                name,
                description,
                content: body.trim().to_string(),
                allowed_tools,
                manual_invocation: fields
                    .get("disable-model-invocation")
                    .and_then(crate::skills::yaml_flag)
                    == Some(true),
                files: Default::default(),
            });
        }

        // Read agents from .claude/agents/**/*.md — Claude Code scans the
        // agents directory recursively, like the rules directory.
        let mut agents = Vec::new();
        let agents_dir = project_root.join(".claude").join("agents");
        for path in crate::adapters::collect_rule_files(&agents_dir, "md")? {
            let content = std::fs::read_to_string(&path)?;
            let (fields, body) = frontmatter::parse(&content)?;
            // Claude Code treats a file without `name` as documentation kept
            // beside the agents (a README), and skips one without
            // `description`; neither is an agent to propagate.
            let field = |key: &str| {
                fields
                    .get(key)
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_string)
            };
            let (Some(name), Some(description)) = (field("name"), field("description")) else {
                continue;
            };
            let model = fields
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let tools = parse_tool_list(fields.get("tools"));
            let color = fields
                .get("color")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let permission_mode = fields
                .get("permissionMode")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            agents.push(NormalizedAgent {
                name,
                description,
                content: body.trim().to_string(),
                model,
                tools,
                color,
                permission_mode,
            });
        }

        // Read MCP servers from .mcp.json
        let mut mcp_servers = Vec::new();
        let mcp_path = project_root.join(".mcp.json");
        if mcp_path.exists() {
            let mcp_content = std::fs::read_to_string(&mcp_path)?;
            mcp_servers = crate::mcp::parse_mcp_json(&mcp_content)?;
        }

        let config = NormalizedConfig {
            instructions,
            rules,
            skills,
            agents,
            mcp_servers,
        };
        if !reads_agents_md_fallback(project_root) {
            return Ok(config);
        }
        // Without CLAUDE.md, Claude Code loads AGENTS.md: its `## Rule:`
        // sections join `.claude/rules/`, whose files win on a name clash.
        let claude_rules = config.rules.clone();
        // Claude Code loads both files, the root one first.
        let files: Vec<PathBuf> = AGENTS_MD_FALLBACKS
            .iter()
            .map(|name| project_root.join(name))
            .collect();
        let mut config = crate::markdown::read_agents_md_files(&files, config)?;
        config
            .rules
            .retain(|r| !claude_rules.iter().any(|c| c.name == r.name));
        config.rules.extend(claude_rules);
        Ok(config)
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        let mut files = Vec::new();

        let claude_md = claude_md_path(project_root);
        let mut claude_content = config.instructions.clone();

        let mut rule_files: Vec<(&NormalizedRule, String)> = Vec::new();
        for rule in &config.rules {
            match &rule.activation {
                ActivationMode::Always => {
                    claude_content.push_str("\n\n## ");
                    claude_content.push_str(&rule.name);
                    claude_content.push_str("\n\n");
                    claude_content.push_str(&rule.content);
                }
                _ => {
                    let filename = format!("{}.md", rule_file_name(&rule.name));
                    rule_files.push((rule, filename));
                }
            }
        }

        // No blank CLAUDE.md for a config with neither instructions nor
        // always-on rules; the other outputs below stand on their own.
        let claude_content = claude_content.trim();
        if !claude_content.is_empty() {
            files.push((claude_md, format!("{}\n", claude_content)));
        }

        if !rule_files.is_empty() {
            let rules_dir = project_root.join(".claude").join("rules");
            for (rule, filename) in rule_files {
                let mut fields = BTreeMap::new();
                if let ActivationMode::GlobMatch(globs) = &rule.activation {
                    let yaml_globs: Vec<serde_yaml_ng::Value> = globs
                        .iter()
                        .map(|g| serde_yaml_ng::Value::String(g.clone()))
                        .collect();
                    fields.insert(
                        "paths".to_string(),
                        serde_yaml_ng::Value::Sequence(yaml_globs),
                    );
                }

                let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
                files.push((rules_dir.join(filename), content));
            }
        }

        // Generate skills as .claude/skills/<name>/SKILL.md
        files.extend(crate::skills::generate_claude_skills(
            project_root,
            &config.skills,
        )?);

        // Generate subagents as .claude/agents/<name>.md
        files.extend(crate::skills::generate_claude_agents(
            project_root,
            &config.agents,
        )?);

        // Merge MCP servers into .mcp.json. Per-server keys conforme never
        // writes (`oauth`, `headersHelper`, `timeout`, …) survive a sync.
        files.extend(crate::json_settings::server_settings_file(
            &project_root.join(".mcp.json"),
            "mcpServers",
            crate::mcp::build_claude_servers_object(&config.mcp_servers),
            crate::mcp::CLAUDE_OWNED_SERVER_KEYS,
            &[],
        )?);

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ActivationMode, McpTransport, NormalizedAgent, NormalizedConfig, NormalizedMcpServer,
        NormalizedRule, NormalizedSkill,
    };
    use std::collections::BTreeMap;
    use std::path::Path;

    fn test_config() -> NormalizedConfig {
        NormalizedConfig {
            instructions: "Be helpful.".to_string(),
            rules: vec![
                NormalizedRule {
                    name: "TypeScript".to_string(),
                    content: "Use strict mode.".to_string(),
                    activation: ActivationMode::Always,
                },
                NormalizedRule {
                    name: "API Rules".to_string(),
                    content: "Follow REST.".to_string(),
                    activation: ActivationMode::GlobMatch(vec!["src/api/**".to_string()]),
                },
                NormalizedRule {
                    name: "Smart Rule".to_string(),
                    content: "Decide wisely.".to_string(),
                    activation: ActivationMode::AgentDecision {
                        description: "API context".to_string(),
                    },
                },
                NormalizedRule {
                    name: "Manual Rule".to_string(),
                    content: "Only when asked.".to_string(),
                    activation: ActivationMode::Manual,
                },
            ],
            skills: vec![],
            mcp_servers: vec![],
            agents: vec![],
        }
    }

    #[test]
    fn test_generate_instructions_only() {
        let adapter = ClaudeAdapter;
        let config = NormalizedConfig {
            instructions: "Be helpful.".to_string(),
            rules: vec![],
            skills: vec![],
            mcp_servers: vec![],
            agents: vec![],
        };
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].0.ends_with("CLAUDE.md"));
        assert!(files[0].1.contains("Be helpful."));
    }

    #[test]
    fn test_generate_with_always_rules() {
        let adapter = ClaudeAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        // The CLAUDE.md file should contain the always rule inlined
        let claude_md = files
            .iter()
            .find(|(p, _)| p.ends_with("CLAUDE.md"))
            .unwrap();
        assert!(claude_md.1.contains("## TypeScript"));
        assert!(claude_md.1.contains("Use strict mode."));
    }

    #[test]
    fn test_generate_with_glob_rules() {
        let adapter = ClaudeAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        // Glob rules go to .claude/rules/<name>.md with paths: frontmatter
        let api_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("api-rules.md"))
            .unwrap();
        assert!(api_rule
            .0
            .to_string_lossy()
            .contains(".claude/rules/api-rules.md"));
        assert!(api_rule.1.contains("paths:"));
        assert!(api_rule.1.contains("src/api/**"));
        assert!(api_rule.1.contains("Follow REST."));
    }

    #[test]
    fn test_generate_with_skills() {
        let adapter = ClaudeAdapter;
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy app".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec!["Bash".to_string()],
                ..Default::default()
            }],
            mcp_servers: vec![],
            agents: vec![],
        };
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let skill_file = files.iter().find(|(p, _)| p.ends_with("SKILL.md")).unwrap();
        assert!(skill_file
            .0
            .to_string_lossy()
            .contains(".claude/skills/deploy/SKILL.md"));
        assert!(skill_file.1.contains("name: deploy"));
        assert!(skill_file.1.contains("description: Deploy app"));
        assert!(skill_file.1.contains("allowed-tools: Bash"));
        assert!(skill_file.1.contains("Run deploy."));
    }

    #[test]
    fn test_generate_with_agents() {
        let adapter = ClaudeAdapter;
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            skills: vec![],
            mcp_servers: vec![],
            agents: vec![NormalizedAgent {
                name: "reviewer".to_string(),
                description: "Code review".to_string(),
                content: "Review code.".to_string(),
                model: Some("gpt-4o".to_string()),
                tools: vec!["codebase".to_string()],
                ..Default::default()
            }],
        };
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let agent_file = files
            .iter()
            .find(|(p, _)| p.to_string_lossy().contains(".claude/agents/"))
            .unwrap();
        assert!(agent_file.0.ends_with("reviewer.md"));
        assert!(agent_file.1.contains("description: Code review"));
        assert!(agent_file.1.contains("Review code."));
        // A Copilot tool name is translated (an unresolvable list makes Claude
        // Code refuse to launch the agent) and another vendor's model is left
        // out.
        assert!(agent_file.1.contains("tools: Grep\n"), "{}", agent_file.1);
        assert!(!agent_file.1.contains("model:"), "{}", agent_file.1);
    }

    #[test]
    fn test_agent_tools_from_other_hosts_are_translated() {
        let config = NormalizedConfig {
            agents: vec![NormalizedAgent {
                name: "reviewer".to_string(),
                description: "Review".to_string(),
                model: Some("opus".to_string()),
                tools: vec![
                    "read_file".to_string(),
                    "run_shell_command".to_string(),
                    "mcp_github_list_issues".to_string(),
                    "@slack".to_string(),
                    "Agent(worker)".to_string(),
                    "githubRepo".to_string(),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = ClaudeAdapter.generate(Path::new("/r"), &config).unwrap();
        assert!(
            files[0].1.contains(
                "tools: Read, Bash, mcp__github__list_issues, mcp__slack, Agent(worker)\n"
            ),
            "{}",
            files[0].1
        );
        assert!(files[0].1.contains("model: opus\n"));
    }

    #[test]
    fn test_read_skips_agent_docs_and_reads_nested_commands() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".claude/agents")).unwrap();
        std::fs::write(root.join(".claude/agents/README.md"), "# Our agents\n").unwrap();
        std::fs::write(
            root.join(".claude/agents/draft.md"),
            "---\nname: draft\n---\nNo description.\n",
        )
        .unwrap();
        std::fs::write(
            root.join(".claude/agents/reviewer.md"),
            "---\nname: reviewer\ndescription: Review\n---\nReview.\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join(".claude/commands/frontend")).unwrap();
        std::fs::write(
            root.join(".claude/commands/frontend/component.md"),
            "---\ndescription: Make a component\n---\nBuild it.\n",
        )
        .unwrap();

        let config = ClaudeAdapter.read(root).unwrap();
        let agents: Vec<_> = config.agents.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(agents, vec!["reviewer"]);
        assert_eq!(config.skills[0].name, "frontend:component");

        // Orphan cleanup keeps the README: it is not an agent.
        let generated = ClaudeAdapter
            .generate(root, &NormalizedConfig::default())
            .unwrap();
        let cleaned =
            crate::adapters::clean_orphans(&ClaudeAdapter.managed_directories(root), &generated)
                .unwrap();
        assert!(root.join(".claude/agents/README.md").exists());
        assert!(cleaned.contains(&root.join(".claude/agents/reviewer.md")));
    }

    #[test]
    fn test_read_paths_string_keeps_brace_groups() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".claude/rules")).unwrap();
        std::fs::write(
            tmp.path().join(".claude/rules/ts.md"),
            "---\npaths: \"src/**/*.{ts,tsx}, lib/**\"\n---\nTS.\n",
        )
        .unwrap();
        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert_eq!(
            config.rules[0].activation,
            ActivationMode::GlobMatch(vec!["src/**/*.{ts,tsx}".into(), "lib/**".into()])
        );
    }

    #[test]
    fn test_generate_with_mcp() {
        let adapter = ClaudeAdapter;
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            skills: vec![],
            mcp_servers: vec![NormalizedMcpServer {
                name: "test-server".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@test/server".to_string()],
                },
                env: BTreeMap::new(),
            }],
            agents: vec![],
        };
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let mcp_file = files
            .iter()
            .find(|(p, _)| p.ends_with(".mcp.json"))
            .unwrap();
        assert!(mcp_file.1.contains("mcpServers"));
        assert!(mcp_file.1.contains("test-server"));
        assert!(mcp_file.1.contains("npx"));
        assert!(mcp_file.1.contains("@test/server"));
    }

    #[test]
    fn test_generate_empty_config() {
        let adapter = ClaudeAdapter;
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            skills: vec![],
            mcp_servers: vec![],
            agents: vec![],
        };
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        // No blank CLAUDE.md is dropped into the project for an empty config.
        assert!(files.is_empty());
    }

    #[test]
    fn test_parse_tool_list_all_forms() {
        use serde_yaml_ng::Value;
        // Comma-separated string
        assert_eq!(
            parse_tool_list(Some(&Value::String("Read, Bash, Write".to_string()))),
            vec!["Read", "Bash", "Write"]
        );
        // Space-separated string (the form conforme writes for subagents/skills)
        assert_eq!(
            parse_tool_list(Some(&Value::String("Read Bash Write".to_string()))),
            vec!["Read", "Bash", "Write"]
        );
        // Mixed comma + space (must not collapse into one token)
        assert_eq!(
            parse_tool_list(Some(&Value::String("Read, Bash Write".to_string()))),
            vec!["Read", "Bash", "Write"]
        );
        // YAML sequence (Claude's own `--agents` JSON / list-style frontmatter)
        let seq = Value::Sequence(vec![
            Value::String("Read".to_string()),
            Value::String("Grep".to_string()),
        ]);
        assert_eq!(parse_tool_list(Some(&seq)), vec!["Read", "Grep"]);
        // Missing field
        assert!(parse_tool_list(None).is_empty());
    }
}
