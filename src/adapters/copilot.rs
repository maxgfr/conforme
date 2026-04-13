use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};
use crate::frontmatter;

pub struct CopilotAdapter;

impl AiToolAdapter for CopilotAdapter {
    fn name(&self) -> &str {
        "GitHub Copilot"
    }

    fn id(&self) -> &str {
        "copilot"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root
            .join(".github")
            .join("copilot-instructions.md")
            .exists()
            || project_root.join(".github").join("instructions").is_dir()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let instructions_file = project_root.join(".github").join("copilot-instructions.md");
        let instructions = if instructions_file.exists() {
            std::fs::read_to_string(&instructions_file)
                .with_context(|| format!("failed to read {}", instructions_file.display()))?
                .trim()
                .to_string()
        } else {
            String::new()
        };

        let mut rules = Vec::new();
        let instr_dir = project_root.join(".github").join("instructions");
        if instr_dir.is_dir() {
            for entry in std::fs::read_dir(&instr_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(".instructions.md"))
                {
                    let content = std::fs::read_to_string(&path)
                        .with_context(|| format!("failed to read {}", path.display()))?;
                    let (fields, body) = frontmatter::parse(&content)?;

                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .strip_suffix(".instructions.md")
                        .unwrap_or("unknown")
                        .to_string();

                    let activation =
                        if let Some(apply_to) = fields.get("applyTo").and_then(|v| v.as_str()) {
                            let patterns: Vec<String> = apply_to
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            if patterns.is_empty() {
                                ActivationMode::Always
                            } else {
                                ActivationMode::GlobMatch(patterns)
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
        let github_dir = project_root.join(".github");
        let mut files = Vec::new();

        let mut main_content = config.instructions.clone();
        let mut instruction_rules: Vec<&NormalizedRule> = Vec::new();

        for rule in &config.rules {
            match &rule.activation {
                ActivationMode::Always
                | ActivationMode::AgentDecision { .. }
                | ActivationMode::Manual => {
                    main_content.push_str("\n\n## ");
                    main_content.push_str(&rule.name);
                    main_content.push_str("\n\n");
                    main_content.push_str(&rule.content);
                }
                ActivationMode::GlobMatch(_) => {
                    instruction_rules.push(rule);
                }
            }
        }

        if !main_content.is_empty() {
            files.push((
                github_dir.join("copilot-instructions.md"),
                format!("{}\n", main_content.trim()),
            ));
        }

        if !instruction_rules.is_empty() {
            let instr_dir = github_dir.join("instructions");
            for rule in instruction_rules {
                if let ActivationMode::GlobMatch(globs) = &rule.activation {
                    let filename = format!("{}.instructions.md", sanitize_name(&rule.name));
                    let mut fields = BTreeMap::new();
                    fields.insert(
                        "applyTo".to_string(),
                        serde_yaml_ng::Value::String(globs.join(", ")),
                    );
                    let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
                    files.push((instr_dir.join(filename), content));
                }
            }
        }

        // Generate skills as .github/prompts/<name>.prompt.md
        files.extend(crate::skills::generate_copilot_prompts(
            project_root,
            &config.skills,
        )?);

        // Generate agents as .github/agents/<name>.agent.md
        files.extend(crate::skills::generate_copilot_agents(
            project_root,
            &config.agents,
        )?);

        // Generate MCP config as .vscode/mcp.json (Copilot uses `servers` key)
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_copilot_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".vscode").join("mcp.json"),
                format!("{}\n", mcp_json),
            ));
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
        let adapter = CopilotAdapter;
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
        assert!(files[0].0.ends_with("copilot-instructions.md"));
        assert!(files[0]
            .0
            .to_string_lossy()
            .contains(".github/copilot-instructions.md"));
        assert!(files[0].1.contains("Be helpful."));
    }

    #[test]
    fn test_generate_with_glob_rules() {
        let adapter = CopilotAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        // Glob rules go to .github/instructions/<name>.instructions.md with applyTo:
        let api_rule = files
            .iter()
            .find(|(p, _)| p.ends_with("api-rules.instructions.md"))
            .unwrap();
        assert!(api_rule
            .0
            .to_string_lossy()
            .contains(".github/instructions/"));
        assert!(api_rule.1.contains("applyTo:"));
        assert!(api_rule.1.contains("src/api/**"));
        assert!(api_rule.1.contains("Follow REST."));
    }

    #[test]
    fn test_generate_always_rules_inlined() {
        let adapter = CopilotAdapter;
        let config = test_config();
        let root = Path::new("/tmp/test");
        let files = adapter.generate(root, &config).unwrap();

        // Always, AgentDecision, and Manual rules get inlined into copilot-instructions.md
        let main_file = files
            .iter()
            .find(|(p, _)| p.ends_with("copilot-instructions.md"))
            .unwrap();
        assert!(main_file.1.contains("## TypeScript"));
        assert!(main_file.1.contains("Use strict mode."));
        assert!(main_file.1.contains("## Smart Rule"));
        assert!(main_file.1.contains("Decide wisely."));
        assert!(main_file.1.contains("## Manual Rule"));
        assert!(main_file.1.contains("Only when asked."));
    }

    #[test]
    fn test_generate_with_skills() {
        let adapter = CopilotAdapter;
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

        let prompt_file = files
            .iter()
            .find(|(p, _)| p.ends_with("deploy.prompt.md"))
            .unwrap();
        assert!(prompt_file.0.to_string_lossy().contains(".github/prompts/"));
        assert!(prompt_file.1.contains("description: Deploy app"));
        assert!(prompt_file.1.contains("Run deploy."));
    }

    #[test]
    fn test_generate_with_agents() {
        let adapter = CopilotAdapter;
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
            .find(|(p, _)| p.ends_with("reviewer.agent.md"))
            .unwrap();
        assert!(agent_file.0.to_string_lossy().contains(".github/agents/"));
        assert!(agent_file.1.contains("name: reviewer"));
        assert!(agent_file.1.contains("description: Code review"));
        assert!(agent_file.1.contains("model: gpt-4o"));
        assert!(agent_file.1.contains("Review code."));
    }

    #[test]
    fn test_generate_with_mcp() {
        let adapter = CopilotAdapter;
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
        // Copilot uses .vscode/mcp.json with "servers" key (NOT mcpServers)
        assert!(mcp_file.0.to_string_lossy().contains(".vscode/mcp.json"));
        assert!(mcp_file.1.contains("\"servers\""));
        assert!(!mcp_file.1.contains("mcpServers"));
        assert!(mcp_file.1.contains("test-server"));
        assert!(mcp_file.1.contains("npx"));
    }

    #[test]
    fn test_generate_empty_config() {
        let adapter = CopilotAdapter;
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
