use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};
use crate::frontmatter;

/// Continue.dev adapter.
/// Rules in .continue/rules/*.md with optional YAML frontmatter:
/// name, globs (array), alwaysApply, description.
pub struct ContinueDevAdapter;

impl AiToolAdapter for ContinueDevAdapter {
    fn name(&self) -> &str {
        "Continue.dev"
    }

    fn id(&self) -> &str {
        "continue"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".continue").is_dir()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: true,
            skills: false,
            agents: false,
            mcp: true,
        }
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<PathBuf> {
        vec![project_root.join(".continue").join("rules")]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let rules_dir = project_root.join(".continue").join("rules");
        if rules_dir.is_dir() {
            for entry in std::fs::read_dir(&rules_dir)? {
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

                    let activation = parse_continue_activation(&fields);

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
        let rules_dir = project_root.join(".continue").join("rules");
        let mut files = Vec::new();

        if !config.instructions.is_empty() {
            let mut fields = BTreeMap::new();
            fields.insert(
                "name".to_string(),
                serde_yaml_ng::Value::String("General".to_string()),
            );
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(true));
            let content = frontmatter::serialize(&fields, &format!("{}\n", config.instructions))?;
            files.push((rules_dir.join("general.md"), content));
        }

        for rule in &config.rules {
            let filename = format!("{}.md", sanitize_name(&rule.name));
            let fields = build_continue_fields(rule);
            let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
            files.push((rules_dir.join(filename), content));
        }

        // Generate MCP config as .continue/mcp.json
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".continue").join("mcp.json"),
                format!("{}\n", mcp_json),
            ));
        }

        Ok(files)
    }
}

fn parse_continue_activation(fields: &BTreeMap<String, serde_yaml_ng::Value>) -> ActivationMode {
    let always_apply = fields
        .get("alwaysApply")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Continue uses globs as an array, not a comma-separated string
    let globs = fields.get("globs").and_then(|v| {
        if let serde_yaml_ng::Value::Sequence(arr) = v {
            let patterns: Vec<String> = arr
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect();
            if patterns.is_empty() {
                None
            } else {
                Some(patterns)
            }
        } else if let Some(s) = v.as_str() {
            let patterns: Vec<String> = s
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if patterns.is_empty() {
                None
            } else {
                Some(patterns)
            }
        } else {
            None
        }
    });

    let description = fields.get("description").and_then(|v| v.as_str());

    if always_apply {
        ActivationMode::Always
    } else if let Some(g) = globs {
        ActivationMode::GlobMatch(g)
    } else if let Some(desc) = description {
        ActivationMode::AgentDecision {
            description: desc.to_string(),
        }
    } else {
        ActivationMode::Always
    }
}

fn build_continue_fields(rule: &NormalizedRule) -> BTreeMap<String, serde_yaml_ng::Value> {
    let mut fields = BTreeMap::new();

    fields.insert(
        "name".to_string(),
        serde_yaml_ng::Value::String(rule.name.clone()),
    );

    match &rule.activation {
        ActivationMode::Always => {
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(true));
        }
        ActivationMode::GlobMatch(globs) => {
            let yaml_globs: Vec<serde_yaml_ng::Value> = globs
                .iter()
                .map(|g| serde_yaml_ng::Value::String(g.clone()))
                .collect();
            fields.insert(
                "globs".to_string(),
                serde_yaml_ng::Value::Sequence(yaml_globs),
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
        ActivationMode, McpTransport, NormalizedConfig, NormalizedMcpServer, NormalizedRule,
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
        let adapter = ContinueDevAdapter;
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
            .contains(".continue/rules/general.md"));
        assert!(files[0].1.contains("alwaysApply: true"));
        assert!(files[0].1.contains("name: General"));
        assert!(files[0].1.contains("Be helpful."));
    }

    #[test]
    fn test_generate_glob_rule() {
        let adapter = ContinueDevAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let api_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("api-rules.md"))
            .unwrap();
        assert!(api_rule.1.contains("globs:"));
        assert!(api_rule.1.contains("src/api/**"));
        assert!(api_rule.1.contains("alwaysApply: false"));
        assert!(api_rule.1.contains("Follow REST."));
        // globs should be a YAML sequence
        assert!(api_rule.1.contains("- src/api/**"));
    }

    #[test]
    fn test_generate_agent_decision_rule() {
        let adapter = ContinueDevAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let smart_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("smart-rule.md"))
            .unwrap();
        assert!(smart_rule.1.contains("description: API context"));
        assert!(smart_rule.1.contains("alwaysApply: false"));
        assert!(smart_rule.1.contains("name: Smart Rule"));
        assert!(smart_rule.1.contains("Decide wisely."));
    }

    #[test]
    fn test_generate_manual_rule() {
        let adapter = ContinueDevAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let manual_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("manual-rule.md"))
            .unwrap();
        assert!(manual_rule.1.contains("alwaysApply: false"));
        assert!(manual_rule.1.contains("name: Manual Rule"));
        assert!(manual_rule.1.contains("Only when asked."));
    }

    #[test]
    fn test_generate_with_mcp() {
        let adapter = ContinueDevAdapter;
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
        assert!(mcp_file.0.to_string_lossy().contains(".continue/mcp.json"));
        assert!(mcp_file.1.contains("mcpServers"));
        assert!(mcp_file.1.contains("test-server"));
        assert!(mcp_file.1.contains("npx"));
    }

    #[test]
    fn test_generate_empty_config() {
        let adapter = ContinueDevAdapter;
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
