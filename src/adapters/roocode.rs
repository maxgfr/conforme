use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};

/// Roo Code / Cline adapter.
/// Rules in .roo/rules/*.md — plain Markdown, NO YAML frontmatter.
/// Files loaded in alphabetical order. Mode-specific rules in .roo/rules-{mode}/.
pub struct RooCodeAdapter;

impl AiToolAdapter for RooCodeAdapter {
    fn name(&self) -> &str {
        "Roo Code"
    }

    fn id(&self) -> &str {
        "roocode"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".roo").is_dir()
            || project_root.join(".roorules").exists()
            || project_root.join(".clinerules").exists()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let rules_dir = project_root.join(".roo").join("rules");
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
                    let name = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    if name == "00-general" || name == "general" {
                        instructions = content.trim().to_string();
                    } else {
                        rules.push(NormalizedRule {
                            name,
                            content: content.trim().to_string(),
                            activation: ActivationMode::Always,
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
        let rules_dir = project_root.join(".roo").join("rules");
        let mut files = Vec::new();
        let mut idx = 0u32;

        // Roo Code has no frontmatter — files are plain Markdown, loaded alphabetically.
        // Use numeric prefix for ordering: 00-general, 01-rule-name, etc.

        if !config.instructions.is_empty() {
            files.push((
                rules_dir.join("00-general.md"),
                format!("{}\n", config.instructions),
            ));
            idx += 1;
        }

        for rule in &config.rules {
            let filename = format!("{:02}-{}.md", idx, sanitize_name(&rule.name));
            // Roo Code doesn't support activation modes — all rules are always-on.
            // For glob/agent-decision rules, we include a comment noting the intended scope.
            let mut content = String::new();
            match &rule.activation {
                ActivationMode::GlobMatch(globs) => {
                    content.push_str(&format!(
                        "<!-- Intended scope: {} -->\n\n",
                        globs.join(", ")
                    ));
                }
                ActivationMode::AgentDecision { description } if !description.is_empty() => {
                    content.push_str(&format!("<!-- {description} -->\n\n"));
                }
                _ => {}
            }
            content.push_str(&rule.content);
            files.push((rules_dir.join(filename), format!("{}\n", content.trim())));
            idx += 1;
        }

        // Generate MCP config as .roo/mcp.json
        if !config.mcp_servers.is_empty() {
            let mcp_json = crate::mcp::generate_mcp_json(&config.mcp_servers)?;
            files.push((
                project_root.join(".roo").join("mcp.json"),
                format!("{}\n", mcp_json),
            ));
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpTransport, NormalizedConfig, NormalizedMcpServer};
    use std::path::Path;

    fn make_adapter() -> RooCodeAdapter {
        RooCodeAdapter
    }

    #[test]
    fn test_generate_instructions_only() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "General instructions.".to_string(),
            rules: vec![],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.roo/rules/00-general.md"));
        assert_eq!(files[0].1, "General instructions.\n");
    }

    #[test]
    fn test_generate_with_rules() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Top-level.".to_string(),
            rules: vec![
                NormalizedRule {
                    name: "TypeScript".to_string(),
                    content: "Use strict mode.".to_string(),
                    activation: ActivationMode::Always,
                },
                NormalizedRule {
                    name: "Security".to_string(),
                    content: "No eval.".to_string(),
                    activation: ActivationMode::Always,
                },
            ],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].0, Path::new("/tmp/test/.roo/rules/00-general.md"));
        assert_eq!(
            files[1].0,
            Path::new("/tmp/test/.roo/rules/01-typescript.md")
        );
        assert_eq!(files[2].0, Path::new("/tmp/test/.roo/rules/02-security.md"));
        assert!(files[1].1.contains("Use strict mode."));
        assert!(files[2].1.contains("No eval."));
    }

    #[test]
    fn test_generate_glob_rule_has_comment() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![NormalizedRule {
                name: "TypeScript".to_string(),
                content: "Use strict mode.".to_string(),
                activation: ActivationMode::GlobMatch(vec![
                    "**/*.ts".to_string(),
                    "**/*.tsx".to_string(),
                ]),
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0]
            .1
            .contains("<!-- Intended scope: **/*.ts, **/*.tsx -->"));
        assert!(files[0].1.contains("Use strict mode."));
    }

    #[test]
    fn test_generate_with_mcp() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![],
            mcp_servers: vec![NormalizedMcpServer {
                name: "fs".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/fs".to_string()],
                },
                env: std::collections::BTreeMap::new(),
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.roo/mcp.json"));
        assert!(files[0].1.contains("mcpServers"));
        assert!(files[0].1.contains("\"fs\""));
    }

    #[test]
    fn test_generate_empty_config() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert!(files.is_empty());
    }
}
