use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::devin::{build_devin_fields, parse_devin_activation};
use crate::adapters::{AdapterCapabilities, AiToolAdapter, ManagedDir};
use crate::config::{
    rule_file_name, sanitize_name, McpTransport, NormalizedConfig, NormalizedRule,
};
use crate::frontmatter;
use crate::skills::{antigravity_model, description_or_name};
use std::collections::BTreeMap;

/// Antigravity CLI adapter (Google's Antigravity command-line agent).
///
/// Antigravity reads `AGENTS.md`, or `GEMINI.md` when that is absent, and keeps
/// its project files under `.agents/`: rules in `.agents/rules/`, skills in
/// `.agents/skills/`, subagents in `.agents/agents/` and MCP servers in
/// `.agents/mcp_config.json`.
pub struct AntigravityAdapter;

fn rules_dir(project_root: &Path) -> PathBuf {
    project_root.join(".agents").join("rules")
}

fn agents_dir(project_root: &Path) -> PathBuf {
    project_root.join(".agents").join("agents")
}

fn mcp_path(project_root: &Path) -> PathBuf {
    project_root.join(".agents").join("mcp_config.json")
}

/// Subagents Antigravity ships built in; a file of that name would replace one.
const BUILTIN_AGENTS: [&str; 4] = ["research", "browser", "self", "image-generator"];

fn is_builtin_agent(name: &str) -> bool {
    BUILTIN_AGENTS.contains(&sanitize_name(name).as_str())
}

/// An agent file that overrides a built-in subagent is the user's, not
/// conforme's. Antigravity names an agent by its frontmatter `name`, so either
/// the file stem or that `name` naming a built-in one makes it an override.
fn is_builtin_agent_file(path: &Path) -> bool {
    path.file_stem()
        .is_some_and(|stem| is_builtin_agent(&stem.to_string_lossy()))
        || agent_file_fields(path)
            .and_then(|fields| {
                fields
                    .get("name")
                    .and_then(serde_yaml_ng::Value::as_str)
                    .map(|name| is_builtin_agent(name.trim()))
            })
            .unwrap_or(false)
}

/// The frontmatter of an agent file, when it exists and parses.
fn agent_file_fields(path: &Path) -> Option<BTreeMap<String, serde_yaml_ng::Value>> {
    let content = std::fs::read_to_string(path).ok()?;
    frontmatter::parse(&content).ok().map(|(fields, _)| fields)
}

/// The `tools` value an existing `.agents/agents/<name>.md` holds. conforme
/// cannot translate tool names into Antigravity's (they are undocumented), so
/// the user's own restriction there is kept as written.
fn existing_agent_tools(project_root: &Path, name: &str) -> Option<serde_yaml_ng::Value> {
    agent_file_fields(&agents_dir(project_root).join(format!("{name}.md")))?.remove("tools")
}

impl AiToolAdapter for AntigravityAdapter {
    fn name(&self) -> &str {
        "Antigravity CLI"
    }

    fn id(&self) -> &str {
        "antigravity"
    }

    /// `.agents/skills/` is shared with Codex and Zed, so it alone does not
    /// detect Antigravity.
    fn detect(&self, project_root: &Path) -> bool {
        let agents = project_root.join(".agents");
        agents.join("mcp_config.json").is_file()
            || agents.join("rules").is_dir()
            || agents.join("agents").is_dir()
            || project_root.join(".antigravityignore").exists()
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            activation_modes: true,
            skills: true,
            agents: true,
            mcp: true,
        }
    }

    fn reads_agents_md(&self, _project_root: &Path) -> bool {
        true
    }

    fn source_files(&self, project_root: &Path) -> Vec<PathBuf> {
        crate::adapters::first_existing_file(project_root, &["AGENTS.md", "GEMINI.md"])
    }

    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(".agents/mcp_config.json")
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![
            // A `.md` without a `trigger`, or in a subfolder, is not a rule
            // Antigravity loads (notes, a draft, a team folder), so it is never
            // one of conforme's orphans nor removed by migrate.
            ManagedDir::files_except(rules_dir(project_root), ".md", |path| {
                !is_top_level_rule_path(path) || !is_antigravity_rule_file(path)
            }),
            ManagedDir::subdirs(project_root.join(".agents").join("skills")),
            ManagedDir::files_except(agents_dir(project_root), ".md", is_builtin_agent_file),
        ]
    }

    fn warnings(&self, project_root: &Path, config: &NormalizedConfig) -> Vec<String> {
        let mut warnings = Vec::new();
        for skill in config.skills.iter().filter(|s| s.manual_invocation) {
            warnings.push(format!(
                "Antigravity has no manual-only skills: {} can be used automatically there",
                sanitize_name(&skill.name)
            ));
        }
        for agent in config.agents.iter().filter(|a| is_builtin_agent(&a.name)) {
            warnings.push(format!(
                "agent {} is not written: a file of that name would replace Antigravity's built-in subagent",
                sanitize_name(&agent.name)
            ));
        }
        for agent in config
            .agents
            .iter()
            .filter(|a| !is_builtin_agent(&a.name) && !a.tools.is_empty())
        {
            let name = sanitize_name(&agent.name);
            if existing_agent_tools(project_root, &name).is_none() {
                warnings.push(format!(
                    "agent {name}: Antigravity's tool names are undocumented, so its tools are not \
                     written and it can use every tool; restrict it with `tools:` in \
                     .agents/agents/{name}.md, which sync keeps"
                ));
            }
        }
        for server in &config.mcp_servers {
            let mut strings: Vec<&String> = match &server.transport {
                McpTransport::Stdio { command, args } => {
                    std::iter::once(command).chain(args).collect()
                }
                McpTransport::Http { url, headers } | McpTransport::Sse { url, headers } => {
                    std::iter::once(url).chain(headers.values()).collect()
                }
            };
            strings.extend(server.env.values());
            if strings.iter().any(|s| s.contains("${")) {
                warnings.push(format!(
                    "Antigravity documents no ${{VAR}} expansion; {} may be sent as written",
                    server.name
                ));
            }
        }
        warnings
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut file_rules = Vec::new();
        for path in top_level_rule_files(&rules_dir(project_root))? {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let (fields, body) = frontmatter::parse(&content)?;
            // A file without a `trigger` is not a rule Antigravity loads.
            if !fields.contains_key("trigger") {
                continue;
            }
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            file_rules.push(NormalizedRule {
                name,
                content: body.trim().to_string(),
                activation: parse_devin_activation(&fields),
            });
        }
        let skills =
            crate::skills::read_skills_from_dir(&project_root.join(".agents").join("skills"))?;
        let agents = crate::skills::read_agents_from_dir(&agents_dir(project_root), false)?
            .into_iter()
            .filter(|agent| !is_builtin_agent(&agent.name))
            .collect();
        let mut mcp_servers = Vec::new();
        let mcp_path = mcp_path(project_root);
        if mcp_path.exists() {
            mcp_servers = crate::mcp::parse_mcp_json(&std::fs::read_to_string(&mcp_path)?)?;
        }
        // Antigravity loads `AGENTS.md` (else `GEMINI.md`) as its instructions.
        let mut config = crate::markdown::read_native_agents_md(
            project_root,
            &["AGENTS.md", "GEMINI.md"],
            NormalizedConfig {
                skills,
                agents,
                mcp_servers,
                ..Default::default()
            },
        )?;
        // The `## Rule:` sections of AGENTS.md come first; a rule file replaces
        // the section it is written from (`## Rule: TypeScript` is
        // `typescript.md`), so both are compared in their file-name form.
        config.rules.retain(|r| {
            let section = rule_file_name(&r.name);
            !file_rules
                .iter()
                .any(|f| rule_file_name(&f.name) == section)
        });
        config.rules.extend(file_rules);
        Ok(config)
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        let dir = rules_dir(project_root);
        let mut files = Vec::new();
        for rule in &config.rules {
            let filename = format!("{}.md", rule_file_name(&rule.name));
            let fields = build_devin_fields(rule);
            let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
            files.push((dir.join(filename), content));
        }

        // Skills live in `.agents/skills/`, shared with Codex and Zed.
        if !config.skills.is_empty() {
            files.extend(crate::skills::generate_codex_skills(
                project_root,
                &config.skills,
            )?);
        }

        let agents_dir = agents_dir(project_root);
        for agent in config.agents.iter().filter(|a| !is_builtin_agent(&a.name)) {
            let name = sanitize_name(&agent.name);
            let mut fields = BTreeMap::new();
            fields.insert(
                "name".to_string(),
                serde_yaml_ng::Value::String(name.clone()),
            );
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(description_or_name(&agent.description, &agent.name)),
            );
            if let Some(model) = agent.model.as_deref().and_then(antigravity_model) {
                fields.insert("model".to_string(), serde_yaml_ng::Value::String(model));
            }
            // Tool names are not translated (Antigravity's are undocumented);
            // a `tools` the user wrote in the target file is kept as written.
            if let Some(tools) = existing_agent_tools(project_root, &name) {
                fields.insert("tools".to_string(), tools);
            }
            let content = frontmatter::serialize(&fields, &format!("{}\n", agent.content))?;
            files.push((agents_dir.join(format!("{name}.md")), content));
        }

        files.extend(crate::json_settings::server_settings_file(
            &mcp_path(project_root),
            "mcpServers",
            crate::mcp::generate_antigravity_mcp(&config.mcp_servers),
            crate::mcp::ANTIGRAVITY_OWNED_SERVER_KEYS,
            &[],
        )?);
        Ok(files)
    }
}

/// Whether `path` is a rule Antigravity loads: its frontmatter parses and has
/// a `trigger`.
fn is_antigravity_rule_file(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| frontmatter::parse(&content).ok())
        .is_some_and(|(fields, _)| fields.contains_key("trigger"))
}

/// Whether `path` sits directly in a `.agents/rules/` directory, the only
/// place Antigravity reads rules from (it does not scan subfolders). With no
/// project root at hand, a path holding another `.agents/rules` pair higher up
/// (`.agents/rules/x/.agents/rules/y.md`) counts as nested: when in doubt the
/// file is kept, never deleted.
fn is_top_level_rule_path(path: &Path) -> bool {
    let names: Vec<&std::ffi::OsStr> = path
        .components()
        .map(|component| component.as_os_str())
        .collect();
    let pairs = names
        .windows(2)
        .filter(|pair| pair[0] == ".agents" && pair[1] == "rules")
        .count();
    let [.., agents, rules, _file] = names.as_slice() else {
        return false;
    };
    *agents == ".agents" && *rules == "rules" && pairs == 1
}

/// The top-level `*.md` files of a rules directory, sorted by file name
/// (case-insensitive) then path. Subfolders are not read.
fn top_level_rule_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md")
        })
        .collect();
    files.sort_by(|a, b| {
        let key = |p: &Path| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        };
        key(a).cmp(&key(b)).then_with(|| a.cmp(b))
    });
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ActivationMode, NormalizedAgent, NormalizedSkill};

    #[test]
    fn test_antigravity_detection() {
        let project = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(project.path().join(".agents").join("skills")).unwrap();
        assert!(
            !AntigravityAdapter.detect(project.path()),
            "a .agents/skills/ folder alone must not detect Antigravity"
        );

        type Signal = (&'static str, fn(&Path));
        let signals: [Signal; 4] = [
            ("mcp_config.json", |root: &Path| {
                std::fs::create_dir_all(root.join(".agents")).unwrap();
                std::fs::write(root.join(".agents").join("mcp_config.json"), "{}").unwrap();
            }),
            ("rules directory", |root: &Path| {
                std::fs::create_dir_all(root.join(".agents").join("rules")).unwrap();
            }),
            ("agents directory", |root: &Path| {
                std::fs::create_dir_all(root.join(".agents").join("agents")).unwrap();
            }),
            (".antigravityignore file", |root: &Path| {
                std::fs::write(root.join(".antigravityignore"), "").unwrap();
            }),
        ];
        for (signal, create) in signals {
            let project = tempfile::TempDir::new().unwrap();
            create(project.path());
            assert!(
                AntigravityAdapter.detect(project.path()),
                "{signal} should detect Antigravity"
            );
        }
    }

    #[test]
    fn test_antigravity_rules_round_trip() {
        let project = tempfile::TempDir::new().unwrap();
        let rules = vec![
            NormalizedRule {
                name: "agent-decision".to_string(),
                content: "Apply it to database migrations.".to_string(),
                activation: ActivationMode::AgentDecision {
                    description: "Database migrations".to_string(),
                },
            },
            NormalizedRule {
                name: "always".to_string(),
                content: "Always applied.".to_string(),
                activation: ActivationMode::Always,
            },
            NormalizedRule {
                name: "glob".to_string(),
                content: "Applied to TypeScript files.".to_string(),
                activation: ActivationMode::GlobMatch(vec![
                    "**/*.ts".to_string(),
                    "**/*.tsx".to_string(),
                ]),
            },
            NormalizedRule {
                name: "manual".to_string(),
                content: "Applied only when asked.".to_string(),
                activation: ActivationMode::Manual,
            },
        ];
        let config = NormalizedConfig {
            rules: rules.clone(),
            ..Default::default()
        };

        let adapter = AntigravityAdapter;
        for (path, content) in adapter.generate(project.path(), &config).unwrap() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }

        // None of these files is a rule: one has no `trigger`, one has no
        // frontmatter, and the last sits in a subfolder (only the top level is read).
        let dir = project.path().join(".agents").join("rules");
        std::fs::write(
            dir.join("notes.md"),
            "---\ndescription: Team notes\n---\n\nNot a rule.\n",
        )
        .unwrap();
        std::fs::write(dir.join("plain.md"), "No frontmatter.\n").unwrap();
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(
            dir.join("nested").join("hidden.md"),
            "---\ntrigger: always_on\n---\n\nNested.\n",
        )
        .unwrap();

        let read = adapter.read(project.path()).unwrap();
        assert_eq!(read.rules.len(), rules.len());
        for (actual, expected) in read.rules.iter().zip(&rules) {
            assert_eq!(actual.name, expected.name);
            assert_eq!(actual.content, expected.content);
            assert_eq!(actual.activation, expected.activation);
        }
    }

    #[test]
    fn test_antigravity_rule_files_without_trigger_are_never_orphans() {
        // Antigravity loads only `.md` files with a `trigger` from
        // `.agents/rules/`: anything else there is the user's, so neither sync's
        // orphan cleanup nor migrate may delete it.
        let project = tempfile::TempDir::new().unwrap();
        let dir = project.path().join(".agents").join("rules");
        std::fs::create_dir_all(&dir).unwrap();
        let rule = dir.join("py.md");
        std::fs::write(&rule, "---\ntrigger: always_on\n---\n\nPython rule.\n").unwrap();
        let no_frontmatter = dir.join("notes.md");
        std::fs::write(&no_frontmatter, "Team notes.\n").unwrap();
        let no_trigger = dir.join("draft.md");
        std::fs::write(&no_trigger, "---\ndescription: Draft\n---\n\nDraft.\n").unwrap();
        let unparsable = dir.join("broken.md");
        std::fs::write(&unparsable, "---\ntrigger: [always_on\n---\n\nBroken.\n").unwrap();

        let managed = AntigravityAdapter.managed_directories(project.path());
        let orphans = crate::adapters::find_orphans(&managed, &[]).unwrap();
        assert_eq!(orphans, vec![rule]);
    }

    #[test]
    fn test_antigravity_nested_rule_files_survive_a_migration() {
        // Antigravity loads only the top-level files of `.agents/rules/`, so a
        // file in a subfolder is never read nor migrated: migrate (which walks
        // the rules directory recursively and deletes what `keep` refuses)
        // must leave it in place, its text otherwise lost.
        let project = tempfile::TempDir::new().unwrap();
        let dir = project.path().join(".agents").join("rules");
        std::fs::create_dir_all(dir.join("team")).unwrap();
        let top = dir.join("top.md");
        std::fs::write(&top, "---\ntrigger: always_on\n---\n\nTop rule.\n").unwrap();
        let nested = dir.join("team").join("nested.md");
        std::fs::write(
            &nested,
            "---\ntrigger: always_on\n---\n\nTeam nested rule, precious.\n",
        )
        .unwrap();
        // Even a subfolder named like the rules directory is not read.
        let deeper = dir.join("x").join(".agents").join("rules").join("deep.md");
        std::fs::create_dir_all(deeper.parent().unwrap()).unwrap();
        std::fs::write(&deeper, "---\ntrigger: always_on\n---\n\nDeep.\n").unwrap();

        let keep = AntigravityAdapter
            .managed_directories(project.path())
            .into_iter()
            .find(|d| d.path == dir)
            .and_then(|d| d.keep)
            .unwrap();
        assert!(keep(&nested), "a nested rule file is not Antigravity's");
        assert!(keep(&deeper), "a nested rule file is not Antigravity's");
        assert!(!keep(&top));
    }

    #[test]
    fn test_antigravity_rule_files_survive_an_agents_md() {
        let project = tempfile::TempDir::new().unwrap();
        std::fs::write(
            project.path().join("AGENTS.md"),
            "# Instructions\nBe nice.\n\n## Rule: from-agents\nFrom AGENTS.md.\n\n## Rule: ts\nOverridden by the file.\n",
        )
        .unwrap();
        let dir = project.path().join(".agents").join("rules");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("ts.md"),
            "---\ntrigger: glob\nglobs: \"**/*.ts\"\n---\n\nTypeScript rule.\n",
        )
        .unwrap();

        let read = AntigravityAdapter.read(project.path()).unwrap();
        assert!(read.instructions.contains("Be nice."));
        let names: Vec<&str> = read.rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["from-agents", "ts"]);
        let ts = read.rules.iter().find(|r| r.name == "ts").unwrap();
        assert_eq!(ts.content, "TypeScript rule.");
        assert_eq!(
            ts.activation,
            ActivationMode::GlobMatch(vec!["**/*.ts".to_string()])
        );
    }

    #[test]
    fn test_antigravity_rule_file_replaces_a_section_by_file_name() {
        // A section `## Rule: TypeScript` is written as `typescript.md`: the
        // file must replace the section, not sit beside it under another name.
        let project = tempfile::TempDir::new().unwrap();
        std::fs::write(
            project.path().join("AGENTS.md"),
            "# Instructions\nBe nice.\n\n## Rule: TypeScript\n<!-- activation: glob **/*.ts -->\nFrom AGENTS.md.\n",
        )
        .unwrap();
        let dir = project.path().join(".agents").join("rules");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("typescript.md"),
            "---\ntrigger: glob\nglobs: \"**/*.ts\"\n---\n\nFrom the file.\n",
        )
        .unwrap();

        let read = AntigravityAdapter.read(project.path()).unwrap();
        let names: Vec<&str> = read.rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["typescript"]);
        assert_eq!(read.rules[0].content, "From the file.");
        assert!(crate::validate::validate(&read, false));
    }

    #[test]
    fn test_antigravity_skills_and_agents_round_trip() {
        let project = tempfile::TempDir::new().unwrap();
        let config = NormalizedConfig {
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy the app".to_string(),
                content: "Run the deploy.".to_string(),
                ..Default::default()
            }],
            agents: vec![
                NormalizedAgent {
                    name: "reviewer".to_string(),
                    description: "Code review".to_string(),
                    content: "Review for bugs.".to_string(),
                    model: Some("pro".to_string()),
                    ..Default::default()
                },
                NormalizedAgent {
                    name: "browser".to_string(),
                    description: "Shadows a built-in".to_string(),
                    content: "Never written.".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let adapter = AntigravityAdapter;
        let files = adapter.generate(project.path(), &config).unwrap();
        assert!(files
            .iter()
            .any(|(p, _)| p.ends_with(".agents/skills/deploy/SKILL.md")));
        assert!(files
            .iter()
            .any(|(p, _)| p.ends_with(".agents/agents/reviewer.md")));
        assert!(
            !files.iter().any(|(p, _)| p.ends_with("browser.md")),
            "a built-in subagent name is never written"
        );
        for (path, content) in files {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        // The user's own file named like a built-in is neither read nor cleaned.
        let agents = project.path().join(".agents").join("agents");
        std::fs::write(
            agents.join("research.md"),
            "---\nname: research\ndescription: Mine\n---\n\nMine.\n",
        )
        .unwrap();

        let read = adapter.read(project.path()).unwrap();
        assert_eq!(read.skills.len(), 1);
        assert_eq!(read.skills[0].name, "deploy");
        assert_eq!(read.skills[0].description, "Deploy the app");
        assert_eq!(read.agents.len(), 1);
        assert_eq!(read.agents[0].name, "reviewer");
        assert_eq!(read.agents[0].description, "Code review");
        assert_eq!(read.agents[0].content.trim(), "Review for bugs.");
        assert_eq!(read.agents[0].model.as_deref(), Some("pro"));

        let keep = adapter
            .managed_directories(project.path())
            .into_iter()
            .find(|d| d.path == agents)
            .and_then(|d| d.keep)
            .unwrap();
        assert!(keep(&agents.join("research.md")));
        assert!(!keep(&agents.join("reviewer.md")));

        let manual = NormalizedConfig {
            skills: vec![NormalizedSkill {
                name: "release".to_string(),
                manual_invocation: true,
                ..Default::default()
            }],
            agents: config.agents.clone(),
            ..Default::default()
        };
        let warnings = adapter.warnings(project.path(), &manual);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].contains("no manual-only skills: release"));
        assert!(warnings[1].contains("browser"));
    }

    #[test]
    fn test_antigravity_builtin_override_by_frontmatter_name_is_kept() {
        // Antigravity names an agent by its frontmatter `name`, so a file of
        // another name holding `name: research` overrides the built-in one: it
        // is the user's, never swept nor deleted by migrate.
        let project = tempfile::TempDir::new().unwrap();
        let agents = project.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        let override_file = agents.join("my-research.md");
        std::fs::write(
            &override_file,
            "---\nname: research\ndescription: Mine\n---\n\nCustom research prompt.\n",
        )
        .unwrap();
        let own = agents.join("reviewer.md");
        std::fs::write(
            &own,
            "---\nname: reviewer\ndescription: Code review\n---\n\nReview.\n",
        )
        .unwrap();

        let managed = AntigravityAdapter.managed_directories(project.path());
        let keep = managed
            .iter()
            .find(|d| d.path == agents)
            .and_then(|d| d.keep)
            .unwrap();
        assert!(keep(&override_file));
        assert!(!keep(&own));
        let orphans = crate::adapters::find_orphans(&managed, &[]).unwrap();
        assert_eq!(orphans, vec![own]);
    }

    fn agent_with_tools(tools: &[&str]) -> NormalizedAgent {
        NormalizedAgent {
            name: "reviewer".to_string(),
            description: "Code review".to_string(),
            content: "Review for bugs.".to_string(),
            tools: tools.iter().map(|t| t.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn test_antigravity_agent_keeps_its_own_tools_on_resync() {
        let project = tempfile::TempDir::new().unwrap();
        let agents = project.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(
            agents.join("reviewer.md"),
            "---\nname: reviewer\ndescription: Old\ntools:\n- read_file\n---\n\nOld prompt.\n",
        )
        .unwrap();
        let config = NormalizedConfig {
            agents: vec![agent_with_tools(&["Read", "Grep"])],
            ..Default::default()
        };

        let adapter = AntigravityAdapter;
        let files = adapter.generate(project.path(), &config).unwrap();
        let (path, content) = files
            .iter()
            .find(|(p, _)| p.ends_with(".agents/agents/reviewer.md"))
            .unwrap();
        let (fields, body) = frontmatter::parse(content).unwrap();
        assert_eq!(
            fields.get("tools"),
            Some(&serde_yaml_ng::Value::Sequence(vec![
                serde_yaml_ng::Value::String("read_file".to_string())
            ])),
            "{content}"
        );
        assert_eq!(body.trim(), "Review for bugs.");
        std::fs::write(path, content).unwrap();
        assert_eq!(
            adapter.read(project.path()).unwrap().agents[0].tools,
            ["read_file"]
        );
        assert!(
            adapter.warnings(project.path(), &config).is_empty(),
            "the target file restricts its own tools"
        );

        // A new agent file gets no `tools` (Antigravity's names are undocumented).
        let fresh = tempfile::TempDir::new().unwrap();
        let files = adapter.generate(fresh.path(), &config).unwrap();
        let (_, content) = files
            .iter()
            .find(|(p, _)| p.ends_with(".agents/agents/reviewer.md"))
            .unwrap();
        assert!(!frontmatter::parse(content).unwrap().0.contains_key("tools"));
    }

    #[test]
    fn test_antigravity_warns_about_unrestricted_agent_tools() {
        let project = tempfile::TempDir::new().unwrap();
        let config = NormalizedConfig {
            agents: vec![agent_with_tools(&["Read", "Grep"]), {
                let mut open = agent_with_tools(&[]);
                open.name = "helper".to_string();
                open
            }],
            ..Default::default()
        };
        let warnings = AntigravityAdapter.warnings(project.path(), &config);
        assert_eq!(
            warnings,
            [
                "agent reviewer: Antigravity's tool names are undocumented, so its tools are not \
              written and it can use every tool; restrict it with `tools:` in \
              .agents/agents/reviewer.md, which sync keeps"
            ],
        );

        // An existing target file without `tools` still warns.
        let agents = project.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(
            agents.join("reviewer.md"),
            "---\nname: reviewer\ndescription: Code review\n---\n\nReview.\n",
        )
        .unwrap();
        assert_eq!(
            AntigravityAdapter.warnings(project.path(), &config).len(),
            1
        );
    }

    #[test]
    fn test_antigravity_model_keeps_only_its_values() {
        for kept in ["inherit", "flash", "pro"] {
            assert_eq!(antigravity_model(kept).as_deref(), Some(kept));
        }
        for dropped in [
            "sonnet",
            "opus",
            "gpt-4o",
            "gemini-2.5-pro",
            "flash-lite",
            "",
        ] {
            assert_eq!(antigravity_model(dropped), None, "{dropped}");
        }
    }
}
