use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
use crate::config::{
    join_flat_globs, rule_file_name, ActivationMode, NormalizedAgent, NormalizedConfig,
    NormalizedRule,
};
use crate::frontmatter;

pub struct CursorAdapter;

impl AiToolAdapter for CursorAdapter {
    fn name(&self) -> &str {
        "Cursor"
    }

    fn id(&self) -> &str {
        "cursor"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".cursor").is_dir() || project_root.join(".cursorrules").exists()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: true,
            skills: true,
            agents: true,
            mcp: true,
        }
    }

    /// `.cursor/mcp.json` is merged, not owned (see `generate`).
    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".cursor/mcp.json"))
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![
            ManagedDir::files(project_root.join(".cursor").join("rules"), ".mdc"),
            ManagedDir::files(project_root.join(".cursor").join("agents"), ".md"),
            ManagedDir::subdirs(project_root.join(".cursor").join("skills")),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let rules_dir = project_root.join(".cursor").join("rules");
        // Cursor documents that `.cursor/rules/` may be organised in
        // subdirectories, so the scan has to recurse.
        for path in crate::adapters::collect_rule_files(&rules_dir, "mdc")? {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let (fields, body) = frontmatter::parse(&content)?;
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let activation = parse_cursor_activation(&fields);

            if name == "general" && activation == ActivationMode::Always {
                instructions = body.trim().to_string();
            } else {
                rules.push(NormalizedRule {
                    name,
                    content: body.trim().to_string(),
                    activation,
                });
            }
        }

        // Read agents from .cursor/agents/*.md (Cursor subagents use .md, not .mdc)
        let mut agents = Vec::new();
        let agents_dir = project_root.join(".cursor").join("agents");
        if agents_dir.is_dir() {
            let mut entries: Vec<_> = std::fs::read_dir(&agents_dir)?
                .filter_map(|e| e.ok())
                .collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    let content = std::fs::read_to_string(&path)
                        .with_context(|| format!("failed to read {}", path.display()))?;
                    let (fields, body) = frontmatter::parse(&content)?;
                    let name = fields
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| path.file_stem().unwrap().to_str().unwrap())
                        .to_string();
                    let description = fields
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let model = fields
                        .get("model")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let tools = crate::skills::parse_frontmatter_tool_list(fields.get("tools"));
                    agents.push(NormalizedAgent {
                        name,
                        description,
                        content: body.trim().to_string(),
                        model,
                        tools,
                        ..Default::default()
                    });
                }
            }
        }

        // Read MCP servers from .cursor/mcp.json
        let mut mcp_servers = Vec::new();
        let mcp_path = project_root.join(".cursor").join("mcp.json");
        if mcp_path.exists() {
            let mcp_content = std::fs::read_to_string(&mcp_path)?;
            mcp_servers = crate::mcp::canonicalize_env_refs(
                crate::mcp::parse_mcp_json(&mcp_content)?,
                crate::mcp::EnvRefStyle::EnvColon,
            );
        }

        // Read skills from .cursor/skills/**/SKILL.md (Cursor walks the
        // skills root recursively).
        let skills = crate::skills::read_skills_recursive(
            &project_root.join(".cursor").join("skills"),
            crate::skills::UNBOUNDED_SKILL_DEPTH,
        )?;

        Ok(NormalizedConfig {
            instructions,
            rules,
            skills,
            agents,
            mcp_servers,
        })
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        let rules_dir = project_root.join(".cursor").join("rules");
        let mut files = Vec::new();

        if !config.instructions.is_empty() {
            let mut fields = BTreeMap::new();
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(true));
            let content = frontmatter::serialize(&fields, &format!("{}\n", config.instructions))?;
            files.push((rules_dir.join("general.mdc"), content));
        }

        for rule in &config.rules {
            let filename = format!("{}.mdc", rule_file_name(&rule.name));
            let fields = build_cursor_fields(rule);
            let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
            files.push((rules_dir.join(filename), content));
        }

        // Generate skills as .cursor/skills/<name>/SKILL.md
        if !config.skills.is_empty() {
            files.extend(crate::skills::generate_cursor_skills(
                project_root,
                &config.skills,
            )?);
        }

        // Generate agents as .cursor/agents/<name>.md (Cursor subagents use .md, not .mdc)
        if !config.agents.is_empty() {
            files.extend(crate::skills::generate_cursor_agents(
                project_root,
                &config.agents,
            )?);
        }

        // Merge MCP servers into .cursor/mcp.json. Per-server keys conforme
        // never writes (`auth`, `envFile`, …) survive a sync.
        files.extend(crate::json_settings::server_settings_file(
            &project_root.join(".cursor").join("mcp.json"),
            "mcpServers",
            crate::mcp::build_cursor_servers_object(&config.mcp_servers),
            crate::mcp::CURSOR_OWNED_SERVER_KEYS,
            &[],
        )?);

        Ok(files)
    }
}

fn parse_cursor_activation(fields: &BTreeMap<String, serde_yaml_ng::Value>) -> ActivationMode {
    let always_apply = fields
        .get("alwaysApply")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    // A string or a list; an empty `globs:` scopes nothing, so the rule falls
    // through to its description (agent-decision) or to manual.
    let globs = crate::config::yaml_globs(fields.get("globs"));
    let description = fields.get("description").and_then(|v| v.as_str());

    if always_apply {
        ActivationMode::Always
    } else if !globs.is_empty() {
        ActivationMode::GlobMatch(globs)
    } else if let Some(desc) = description {
        ActivationMode::AgentDecision {
            description: desc.to_string(),
        }
    } else {
        ActivationMode::Manual
    }
}

fn build_cursor_fields(rule: &NormalizedRule) -> BTreeMap<String, serde_yaml_ng::Value> {
    let mut fields = BTreeMap::new();

    match &rule.activation {
        ActivationMode::Always => {
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(true));
        }
        ActivationMode::GlobMatch(globs) => {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(rule.name.clone()),
            );
            fields.insert(
                "globs".to_string(),
                // "Separate multiple patterns with commas": brace groups are
                // expanded so that split keeps them whole.
                serde_yaml_ng::Value::String(join_flat_globs(globs, ", ")),
            );
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(false));
        }
        ActivationMode::AgentDecision { description } => {
            // Without a description Cursor would read the rule as Manual.
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(crate::skills::description_or_name(
                    description,
                    &rule.name,
                )),
            );
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(false));
        }
        ActivationMode::Manual => {
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(false));
        }
    }

    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ActivationMode, McpTransport, NormalizedAgent, NormalizedConfig, NormalizedMcpServer,
        NormalizedRule,
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
        let adapter = CursorAdapter;
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
        assert!(files[0].0.ends_with("general.mdc"));
        assert!(files[0].1.contains("alwaysApply: true"));
        assert!(files[0].1.contains("Be helpful."));
    }

    #[test]
    fn test_generate_always_rule() {
        let adapter = CursorAdapter;
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![NormalizedRule {
                name: "TypeScript".to_string(),
                content: "Use strict mode.".to_string(),
                activation: ActivationMode::Always,
            }],
            skills: vec![],
            mcp_servers: vec![],
            agents: vec![],
        };
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let ts_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("typescript.mdc"))
            .unwrap();
        assert!(ts_rule.1.contains("alwaysApply: true"));
        assert!(ts_rule.1.contains("Use strict mode."));
    }

    #[test]
    fn test_generate_glob_rule() {
        let adapter = CursorAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let api_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("api-rules.mdc"))
            .unwrap();
        assert!(api_rule.1.contains("globs: "));
        assert!(api_rule.1.contains("src/api/**"));
        assert!(api_rule.1.contains("alwaysApply: false"));
        assert!(api_rule.1.contains("Follow REST."));
    }

    #[test]
    fn test_generate_agent_decision_rule() {
        let adapter = CursorAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let smart_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("smart-rule.mdc"))
            .unwrap();
        assert!(smart_rule.1.contains("description: API context"));
        assert!(smart_rule.1.contains("alwaysApply: false"));
        assert!(smart_rule.1.contains("Decide wisely."));
    }

    #[test]
    fn test_generate_manual_rule() {
        let adapter = CursorAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let manual_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("manual-rule.mdc"))
            .unwrap();
        assert!(manual_rule.1.contains("alwaysApply: false"));
        // Manual rules should not have description or globs
        assert!(!manual_rule.1.contains("description:"));
        assert!(!manual_rule.1.contains("globs:"));
        assert!(manual_rule.1.contains("Only when asked."));
    }

    #[test]
    fn test_generate_with_skills() {
        use crate::config::NormalizedSkill;
        let adapter = CursorAdapter;
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy the app".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec![],
                ..Default::default()
            }],
            mcp_servers: vec![],
            agents: vec![],
        };
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let skill_file = files
            .iter()
            .find(|(p, _)| p.to_string_lossy().contains(".cursor/skills/"))
            .unwrap();
        assert!(skill_file.0.ends_with("SKILL.md"));
        assert!(skill_file.1.contains("name: deploy"));
        assert!(skill_file.1.contains("description: Deploy the app"));
    }

    #[test]
    fn test_generate_with_agents() {
        let adapter = CursorAdapter;
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
            .find(|(p, _)| p.to_string_lossy().contains(".cursor/agents/"))
            .unwrap();
        assert!(agent_file.0.ends_with("reviewer.md"));
        assert!(agent_file.1.contains("name: reviewer"));
        assert!(agent_file.1.contains("description: Code review"));
        assert!(agent_file.1.contains("model: gpt-4o"));
        // Cursor subagents do not use a `tools` frontmatter field
        assert!(!agent_file.1.contains("tools:"));
        assert!(agent_file.1.contains("Review code."));
    }

    #[test]
    fn test_generate_with_mcp() {
        let adapter = CursorAdapter;
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

        let mcp_file = files.iter().find(|(p, _)| p.ends_with("mcp.json")).unwrap();
        assert!(mcp_file.0.to_string_lossy().contains(".cursor/mcp.json"));
        assert!(mcp_file.1.contains("mcpServers"));
        assert!(mcp_file.1.contains("test-server"));
        assert!(mcp_file.1.contains("npx"));
    }

    #[test]
    fn test_generate_empty_config() {
        let adapter = CursorAdapter;
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            skills: vec![],
            mcp_servers: vec![],
            agents: vec![],
        };
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        assert!(files.is_empty());
    }

    fn fields(yaml: &str) -> BTreeMap<String, serde_yaml_ng::Value> {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    #[test]
    fn test_empty_globs_fall_through_to_description() {
        let f = fields("description: API stuff\nglobs: \"\"\nalwaysApply: false\n");
        assert_eq!(
            parse_cursor_activation(&f),
            ActivationMode::AgentDecision {
                description: "API stuff".to_string()
            }
        );
    }

    #[test]
    fn test_empty_globs_without_description_is_manual() {
        let f = fields("globs:\nalwaysApply: false\n");
        assert_eq!(parse_cursor_activation(&f), ActivationMode::Manual);
    }

    #[test]
    fn test_globs_as_yaml_list() {
        let f = fields("globs: [\"**/*.ts\", \"**/*.tsx\"]\nalwaysApply: false\n");
        assert_eq!(
            parse_cursor_activation(&f),
            ActivationMode::GlobMatch(vec!["**/*.ts".to_string(), "**/*.tsx".to_string()])
        );
    }
}
