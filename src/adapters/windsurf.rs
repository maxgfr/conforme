use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};
use crate::frontmatter;

pub struct WindsurfAdapter;

impl AiToolAdapter for WindsurfAdapter {
    fn name(&self) -> &str {
        "Windsurf"
    }

    fn id(&self) -> &str {
        "windsurf"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".windsurf").is_dir() || project_root.join(".windsurfrules").exists()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: true,
            skills: true,
            agents: false,
            mcp: true,
        }
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<PathBuf> {
        vec![
            project_root.join(".windsurf").join("rules"),
            project_root.join(".windsurf").join("skills"),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let rules_dir = project_root.join(".windsurf").join("rules");
        if rules_dir.is_dir() {
            let mut entries: Vec<_> = std::fs::read_dir(&rules_dir)?
                .filter_map(|e| e.ok())
                .collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
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

                    let activation = parse_windsurf_activation(&fields);

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
        let rules_dir = project_root.join(".windsurf").join("rules");
        let mut files = Vec::new();

        if !config.instructions.is_empty() {
            let mut fields = BTreeMap::new();
            fields.insert(
                "trigger".to_string(),
                serde_yaml_ng::Value::String("always_on".to_string()),
            );
            let content = frontmatter::serialize(&fields, &format!("{}\n", config.instructions))?;
            files.push((rules_dir.join("general.md"), content));
        }

        for rule in &config.rules {
            let filename = format!("{}.md", sanitize_name(&rule.name));
            let fields = build_windsurf_fields(rule);
            let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
            files.push((rules_dir.join(filename), content));
        }

        // Generate skills as .windsurf/skills/<name>/SKILL.md
        if !config.skills.is_empty() {
            files.extend(crate::skills::generate_windsurf_skills(
                project_root,
                &config.skills,
            )?);
        }

        // Generate MCP config as .windsurf/mcp.json (best-effort project-level;
        // Windsurf's canonical MCP config is global at ~/.codeium/windsurf/mcp_config.json).
        // Uses Windsurf-specific schema: `serverUrl` for HTTP, no `type` field.
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_windsurf_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".windsurf").join("mcp.json"),
                format!("{}\n", mcp_json),
            ));
        }

        Ok(files)
    }
}

fn parse_windsurf_activation(fields: &BTreeMap<String, serde_yaml_ng::Value>) -> ActivationMode {
    let trigger = fields
        .get("trigger")
        .and_then(|v| v.as_str())
        .unwrap_or("always_on");

    match trigger {
        "always_on" => ActivationMode::Always,
        "glob" => {
            let globs = fields.get("globs").and_then(|v| v.as_str()).unwrap_or("");
            let patterns: Vec<String> = globs
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if patterns.is_empty() {
                ActivationMode::Always
            } else {
                ActivationMode::GlobMatch(patterns)
            }
        }
        "model_decision" => {
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

fn build_windsurf_fields(rule: &NormalizedRule) -> BTreeMap<String, serde_yaml_ng::Value> {
    let mut fields = BTreeMap::new();

    match &rule.activation {
        ActivationMode::Always => {
            fields.insert(
                "trigger".to_string(),
                serde_yaml_ng::Value::String("always_on".to_string()),
            );
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(rule.name.clone()),
            );
        }
        ActivationMode::GlobMatch(globs) => {
            fields.insert(
                "trigger".to_string(),
                serde_yaml_ng::Value::String("glob".to_string()),
            );
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(rule.name.clone()),
            );
            fields.insert(
                "globs".to_string(),
                serde_yaml_ng::Value::String(globs.join(", ")),
            );
        }
        ActivationMode::AgentDecision { description } => {
            fields.insert(
                "trigger".to_string(),
                serde_yaml_ng::Value::String("model_decision".to_string()),
            );
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(description.clone()),
            );
        }
        ActivationMode::Manual => {
            fields.insert(
                "trigger".to_string(),
                serde_yaml_ng::Value::String("manual".to_string()),
            );
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(rule.name.clone()),
            );
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
        let adapter = WindsurfAdapter;
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
            .contains(".windsurf/rules/general.md"));
        assert!(files[0].1.contains("trigger: always_on"));
        assert!(files[0].1.contains("Be helpful."));
    }

    #[test]
    fn test_generate_glob_rule() {
        let adapter = WindsurfAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let api_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("api-rules.md"))
            .unwrap();
        assert!(api_rule.1.contains("trigger: glob"));
        assert!(api_rule.1.contains("globs: "));
        assert!(api_rule.1.contains("src/api/**"));
        assert!(api_rule.1.contains("Follow REST."));
    }

    #[test]
    fn test_generate_agent_decision_rule() {
        let adapter = WindsurfAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let smart_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("smart-rule.md"))
            .unwrap();
        assert!(smart_rule.1.contains("trigger: model_decision"));
        assert!(smart_rule.1.contains("description: API context"));
        assert!(smart_rule.1.contains("Decide wisely."));
    }

    #[test]
    fn test_generate_manual_rule() {
        let adapter = WindsurfAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let manual_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("manual-rule.md"))
            .unwrap();
        assert!(manual_rule.1.contains("trigger: manual"));
        assert!(manual_rule.1.contains("Only when asked."));
    }

    #[test]
    fn test_generate_with_skills() {
        use crate::config::NormalizedSkill;
        let adapter = WindsurfAdapter;
        let config = NormalizedConfig {
            instructions: "".to_string(),
            rules: vec![],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy the app".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec!["Bash".to_string()],
            }],
            mcp_servers: vec![],
            agents: vec![],
        };
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        let skill_file = files
            .iter()
            .find(|(p, _)| p.to_string_lossy().contains(".windsurf/skills/"))
            .unwrap();
        assert!(skill_file.0.ends_with("SKILL.md"));
        assert!(skill_file.1.contains("name: deploy"));
        assert!(skill_file.1.contains("description: Deploy the app"));
        // Windsurf skills don't include allowed-tools
        assert!(!skill_file.1.contains("allowed-tools"));
    }

    #[test]
    fn test_generate_with_mcp() {
        let adapter = WindsurfAdapter;
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
        assert!(mcp_file.0.to_string_lossy().contains(".windsurf/mcp.json"));
        assert!(mcp_file.1.contains("mcpServers"));
        assert!(mcp_file.1.contains("test-server"));
        assert!(mcp_file.1.contains("npx"));
    }

    #[test]
    fn test_generate_empty_config() {
        let adapter = WindsurfAdapter;
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
