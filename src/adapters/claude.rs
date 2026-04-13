use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::{
    sanitize_name, ActivationMode, NormalizedAgent, NormalizedConfig, NormalizedRule,
    NormalizedSkill,
};
use crate::frontmatter;

pub struct ClaudeAdapter;

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

    fn managed_directories(&self, project_root: &Path) -> Vec<PathBuf> {
        vec![
            project_root.join(".claude").join("rules"),
            project_root.join(".claude").join("skills"),
            project_root.join(".claude").join("agents"),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let claude_md = project_root.join("CLAUDE.md");
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
        if rules_dir.is_dir() {
            for entry in std::fs::read_dir(&rules_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    let content = std::fs::read_to_string(&path)?;
                    let (fields, body) = frontmatter::parse(&content)?;
                    let name = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    let activation =
                        if let Some(serde_yaml_ng::Value::Sequence(paths)) = fields.get("paths") {
                            let globs: Vec<String> = paths
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .filter(|s| !s.is_empty())
                                .collect();
                            if globs.is_empty() {
                                ActivationMode::Always
                            } else {
                                ActivationMode::GlobMatch(globs)
                            }
                        } else {
                            ActivationMode::Always
                        };

                    rules.push(NormalizedRule {
                        name,
                        content: body.trim().to_string(),
                        activation,
                    });
                }
            }
        }

        // Read skills from .claude/skills/<name>/SKILL.md
        let mut skills = Vec::new();
        let skills_dir = project_root.join(".claude").join("skills");
        if skills_dir.is_dir() {
            for entry in std::fs::read_dir(&skills_dir)? {
                let entry = entry?;
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
                        let allowed_tools = fields
                            .get("allowed-tools")
                            .and_then(|v| v.as_str())
                            .map(|s| {
                                s.split_whitespace()
                                    .flat_map(|t| t.split(','))
                                    .map(|t| t.trim().to_string())
                                    .filter(|t| !t.is_empty())
                                    .collect()
                            })
                            .unwrap_or_default();
                        skills.push(NormalizedSkill {
                            name,
                            description,
                            content: body.trim().to_string(),
                            allowed_tools,
                        });
                    }
                }
            }
        }

        // Read commands from .claude/commands/*.md (mapped to skills for cross-tool sync)
        let commands_dir = project_root.join(".claude").join("commands");
        if commands_dir.is_dir() {
            for entry in std::fs::read_dir(&commands_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    let content = std::fs::read_to_string(&path)?;
                    let (fields, body) = frontmatter::parse(&content)?;
                    let name = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let description = fields
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let allowed_tools = fields
                        .get("allowed-tools")
                        .and_then(|v| v.as_str())
                        .map(|s| {
                            s.split(',')
                                .map(|t| t.trim().to_string())
                                .filter(|t| !t.is_empty())
                                .collect()
                        })
                        .unwrap_or_default();
                    skills.push(NormalizedSkill {
                        name,
                        description,
                        content: body.trim().to_string(),
                        allowed_tools,
                    });
                }
            }
        }

        // Read agents from .claude/agents/*.md
        let mut agents = Vec::new();
        let agents_dir = project_root.join(".claude").join("agents");
        if agents_dir.is_dir() {
            for entry in std::fs::read_dir(&agents_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    let content = std::fs::read_to_string(&path)?;
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
                    let tools = fields
                        .get("tools")
                        .and_then(|v| v.as_str())
                        .map(|s| {
                            s.split_whitespace()
                                .flat_map(|t| t.split(','))
                                .map(|t| t.trim().to_string())
                                .filter(|t| !t.is_empty())
                                .collect()
                        })
                        .unwrap_or_default();
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

        // Read MCP servers from .mcp.json
        let mut mcp_servers = Vec::new();
        let mcp_path = project_root.join(".mcp.json");
        if mcp_path.exists() {
            let mcp_content = std::fs::read_to_string(&mcp_path)?;
            mcp_servers = crate::mcp::parse_mcp_json(&mcp_content)?;
        }

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
        let mut files = Vec::new();

        let claude_md = project_root.join("CLAUDE.md");
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
                    let filename = format!("{}.md", sanitize_name(&rule.name));
                    rule_files.push((rule, filename));
                }
            }
        }

        files.push((claude_md, format!("{}\n", claude_content.trim())));

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

        // Generate MCP config as .mcp.json
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_mcp_json(&config.mcp_servers)?;
            files.push((project_root.join(".mcp.json"), format!("{}\n", mcp_json)));
        }

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
        assert!(agent_file.1.contains("model: gpt-4o"));
        assert!(agent_file.1.contains("Review code."));
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

        // Should still produce CLAUDE.md even if empty
        assert_eq!(files.len(), 1);
        assert!(files[0].0.ends_with("CLAUDE.md"));
    }
}
