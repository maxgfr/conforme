use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, WriteReport};
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};
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
                                .collect();
                            ActivationMode::GlobMatch(globs)
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
        })
    }

    fn write(&self, project_root: &Path, config: &NormalizedConfig) -> Result<WriteReport> {
        let generated = self.generate(project_root, config)?;
        let mut report = WriteReport {
            files_written: Vec::new(),
            files_unchanged: Vec::new(),
        };

        for (path, content) in generated {
            write_if_changed(&path, &content, &mut report)?;
        }

        Ok(report)
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        let mut files = Vec::new();

        // Write CLAUDE.md with instructions
        let claude_md = project_root.join("CLAUDE.md");
        let mut claude_content = config.instructions.clone();

        // Append always-on rules directly to CLAUDE.md
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

        // Write rules with specific activation to .claude/rules/
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
                // AgentDecision and Manual: no frontmatter (Claude doesn't support these modes)

                let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
                files.push((rules_dir.join(filename), content));
            }
        }

        Ok(files)
    }
}

fn write_if_changed(path: &Path, content: &str, report: &mut WriteReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if path.exists() {
        let existing = std::fs::read_to_string(path)?;
        if crate::hash::contents_match(&existing, content) {
            report.files_unchanged.push(path.to_path_buf());
            return Ok(());
        }
    }

    std::fs::write(path, content)?;
    report.files_written.push(path.to_path_buf());
    Ok(())
}
