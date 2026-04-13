use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};
use crate::frontmatter;

/// Kiro (AWS) adapter.
/// Rules in .kiro/steering/*.md with YAML frontmatter:
/// - inclusion: always | fileMatch | manual | auto
/// - fileMatchPattern: glob patterns (string or array)
/// - name: required for auto inclusion
/// - description: required for auto inclusion
pub struct KiroAdapter;

impl AiToolAdapter for KiroAdapter {
    fn name(&self) -> &str {
        "Kiro"
    }

    fn id(&self) -> &str {
        "kiro"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".kiro").is_dir()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let steering_dir = project_root.join(".kiro").join("steering");
        if steering_dir.is_dir() {
            for entry in std::fs::read_dir(&steering_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    let content = std::fs::read_to_string(&path)
                        .with_context(|| format!("failed to read {}", path.display()))?;
                    let (fields, body) = frontmatter::parse(&content)?;
                    let name = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    let activation = parse_kiro_activation(&fields);

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

        Ok(NormalizedConfig {
            instructions,
            rules,
            ..Default::default()
        })
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        let steering_dir = project_root.join(".kiro").join("steering");
        let mut files = Vec::new();

        if !config.instructions.is_empty() {
            let mut fields = BTreeMap::new();
            fields.insert(
                "inclusion".to_string(),
                serde_yaml_ng::Value::String("always".to_string()),
            );
            let content = frontmatter::serialize(&fields, &format!("{}\n", config.instructions))?;
            files.push((steering_dir.join("general.md"), content));
        }

        for rule in &config.rules {
            let filename = format!("{}.md", sanitize_name(&rule.name));
            let fields = build_kiro_fields(rule);
            let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
            files.push((steering_dir.join(filename), content));
        }

        // Generate skills as .kiro/skills/<name>/SKILL.md
        files.extend(crate::skills::generate_kiro_skills(
            project_root,
            &config.skills,
        )?);

        // Generate agents as .kiro/agents/<name>.md
        if !config.agents.is_empty() {
            files.extend(crate::skills::generate_kiro_agents(
                project_root,
                &config.agents,
            )?);
        }

        // Generate MCP config as .kiro/settings/mcp.json
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".kiro").join("settings").join("mcp.json"),
                format!("{}\n", mcp_json),
            ));
        }

        Ok(files)
    }
}

fn parse_kiro_activation(fields: &BTreeMap<String, serde_yaml_ng::Value>) -> ActivationMode {
    let inclusion = fields
        .get("inclusion")
        .and_then(|v| v.as_str())
        .unwrap_or("always");

    match inclusion {
        "always" => ActivationMode::Always,
        "fileMatch" => {
            let patterns = fields.get("fileMatchPattern").and_then(|v| match v {
                serde_yaml_ng::Value::String(s) => Some(vec![s.clone()]),
                serde_yaml_ng::Value::Sequence(arr) => {
                    let p: Vec<String> = arr
                        .iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .filter(|s| !s.is_empty())
                        .collect();
                    if p.is_empty() {
                        None
                    } else {
                        Some(p)
                    }
                }
                _ => None,
            });
            match patterns {
                Some(p) => ActivationMode::GlobMatch(p),
                None => ActivationMode::Always,
            }
        }
        "auto" => {
            let desc = fields
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ActivationMode::AgentDecision { description: desc }
        }
        "manual" => ActivationMode::Manual,
        _ => ActivationMode::Always,
    }
}

fn build_kiro_fields(rule: &NormalizedRule) -> BTreeMap<String, serde_yaml_ng::Value> {
    let mut fields = BTreeMap::new();

    match &rule.activation {
        ActivationMode::Always => {
            fields.insert(
                "inclusion".to_string(),
                serde_yaml_ng::Value::String("always".to_string()),
            );
        }
        ActivationMode::GlobMatch(globs) => {
            fields.insert(
                "inclusion".to_string(),
                serde_yaml_ng::Value::String("fileMatch".to_string()),
            );
            let yaml_globs: Vec<serde_yaml_ng::Value> = globs
                .iter()
                .map(|g| serde_yaml_ng::Value::String(g.clone()))
                .collect();
            fields.insert(
                "fileMatchPattern".to_string(),
                serde_yaml_ng::Value::Sequence(yaml_globs),
            );
        }
        ActivationMode::AgentDecision { description } => {
            fields.insert(
                "inclusion".to_string(),
                serde_yaml_ng::Value::String("auto".to_string()),
            );
            fields.insert(
                "name".to_string(),
                serde_yaml_ng::Value::String(rule.name.clone()),
            );
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(description.clone()),
            );
        }
        ActivationMode::Manual => {
            fields.insert(
                "inclusion".to_string(),
                serde_yaml_ng::Value::String("manual".to_string()),
            );
        }
    }

    fields
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
        let adapter = KiroAdapter;
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
        assert!(files[0].0.ends_with("general.md"));
        assert!(files[0]
            .0
            .to_string_lossy()
            .contains(".kiro/steering/general.md"));
        assert!(files[0].1.contains("inclusion: always"));
        assert!(files[0].1.contains("Be helpful."));
    }

    #[test]
    fn test_generate_glob_rule() {
        let adapter = KiroAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let api_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("api-rules.md"))
            .unwrap();
        assert!(api_rule.1.contains("inclusion: fileMatch"));
        assert!(api_rule.1.contains("fileMatchPattern:"));
        assert!(api_rule.1.contains("src/api/**"));
        assert!(api_rule.1.contains("Follow REST."));
    }

    #[test]
    fn test_generate_agent_decision_rule() {
        let adapter = KiroAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let smart_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("smart-rule.md"))
            .unwrap();
        assert!(smart_rule.1.contains("inclusion: auto"));
        assert!(smart_rule.1.contains("name: Smart Rule"));
        assert!(smart_rule.1.contains("description: API context"));
        assert!(smart_rule.1.contains("Decide wisely."));
    }

    #[test]
    fn test_generate_manual_rule() {
        let adapter = KiroAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let manual_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("manual-rule.md"))
            .unwrap();
        assert!(manual_rule.1.contains("inclusion: manual"));
        assert!(manual_rule.1.contains("Only when asked."));
    }

    #[test]
    fn test_generate_with_skills() {
        let adapter = KiroAdapter;
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy app".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec!["Bash".to_string()],
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
            .contains(".kiro/skills/deploy/SKILL.md"));
        assert!(skill_file.1.contains("name: deploy"));
        assert!(skill_file.1.contains("description: Deploy app"));
        assert!(skill_file.1.contains("Run deploy."));
    }

    #[test]
    fn test_generate_with_agents() {
        let adapter = KiroAdapter;
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
            .find(|(p, _)| p.to_string_lossy().contains(".kiro/agents/"))
            .unwrap();
        assert!(agent_file.0.ends_with("reviewer.md"));
        assert!(agent_file.1.contains("description: Code review"));
        assert!(agent_file.1.contains("model: gpt-4o"));
        assert!(agent_file.1.contains("Review code."));
    }

    #[test]
    fn test_generate_with_mcp() {
        let adapter = KiroAdapter;
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
        assert!(mcp_file
            .0
            .to_string_lossy()
            .contains(".kiro/settings/mcp.json"));
        assert!(mcp_file.1.contains("mcpServers"));
        assert!(mcp_file.1.contains("test-server"));
        assert!(mcp_file.1.contains("npx"));
    }

    #[test]
    fn test_generate_empty_config() {
        let adapter = KiroAdapter;
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
