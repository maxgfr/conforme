use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::{
    sanitize_name, ActivationMode, NormalizedAgent, NormalizedConfig, NormalizedRule,
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

    fn managed_directories(&self, project_root: &Path) -> Vec<PathBuf> {
        vec![
            project_root.join(".cursor").join("rules"),
            project_root.join(".cursor").join("agents"),
            project_root.join(".cursor").join("skills"),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let rules_dir = project_root.join(".cursor").join("rules");
        if rules_dir.is_dir() {
            for entry in std::fs::read_dir(&rules_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "mdc") {
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
            }
        }

        // Read agents from .cursor/agents/*.mdc
        let mut agents = Vec::new();
        let agents_dir = project_root.join(".cursor").join("agents");
        if agents_dir.is_dir() {
            for entry in std::fs::read_dir(&agents_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "mdc") {
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
                    let tools = match fields.get("tools") {
                        Some(serde_yaml_ng::Value::Sequence(seq)) => seq
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect(),
                        Some(serde_yaml_ng::Value::String(s)) => {
                            s.split_whitespace().map(|t| t.to_string()).collect()
                        }
                        _ => Vec::new(),
                    };
                    agents.push(NormalizedAgent {
                        name,
                        description,
                        content: body.trim().to_string(),
                        model,
                        tools,
                    });
                }
            }
        }

        // Read MCP servers from .cursor/mcp.json
        let mut mcp_servers = Vec::new();
        let mcp_path = project_root.join(".cursor").join("mcp.json");
        if mcp_path.exists() {
            let mcp_content = std::fs::read_to_string(&mcp_path)?;
            mcp_servers = crate::mcp::parse_mcp_json(&mcp_content)?;
        }

        Ok(NormalizedConfig {
            instructions,
            rules,
            skills: Vec::new(),
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
            let filename = format!("{}.mdc", sanitize_name(&rule.name));
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

        // Generate agents as .cursor/agents/<name>.mdc
        if !config.agents.is_empty() {
            files.extend(crate::skills::generate_cursor_agents(
                project_root,
                &config.agents,
            )?);
        }

        // Generate MCP config as .cursor/mcp.json
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".cursor").join("mcp.json"),
                format!("{}\n", mcp_json),
            ));
        }

        Ok(files)
    }
}

fn parse_cursor_activation(fields: &BTreeMap<String, serde_yaml_ng::Value>) -> ActivationMode {
    let always_apply = fields
        .get("alwaysApply")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let globs = fields.get("globs").and_then(|v| v.as_str());
    let description = fields.get("description").and_then(|v| v.as_str());

    if always_apply {
        ActivationMode::Always
    } else if let Some(g) = globs {
        let patterns: Vec<String> = g
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if patterns.is_empty() {
            ActivationMode::Always
        } else {
            ActivationMode::GlobMatch(patterns)
        }
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
                serde_yaml_ng::Value::String(globs.join(", ")),
            );
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(false));
        }
        ActivationMode::AgentDecision { description } => {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(description.clone()),
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
            }],
        };
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let agent_file = files
            .iter()
            .find(|(p, _)| p.to_string_lossy().contains(".cursor/agents/"))
            .unwrap();
        assert!(agent_file.0.ends_with("reviewer.mdc"));
        assert!(agent_file.1.contains("name: reviewer"));
        assert!(agent_file.1.contains("description: Code review"));
        assert!(agent_file.1.contains("model: gpt-4o"));
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
}
