use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::{write_if_changed, AiToolAdapter, WriteReport};
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};

/// Amazon Q Developer adapter.
/// Rules in .amazonq/rules/*.md — plain Markdown files.
/// No YAML frontmatter documented for rules.
pub struct AmazonQAdapter;

impl AiToolAdapter for AmazonQAdapter {
    fn name(&self) -> &str {
        "Amazon Q"
    }

    fn id(&self) -> &str {
        "amazonq"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".amazonq").is_dir()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let rules_dir = project_root.join(".amazonq").join("rules");
        if rules_dir.is_dir() {
            for entry in std::fs::read_dir(&rules_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    let content = std::fs::read_to_string(&path)
                        .with_context(|| format!("failed to read {}", path.display()))?;
                    let name = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    if name == "general" {
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
        let rules_dir = project_root.join(".amazonq").join("rules");
        let mut files = Vec::new();

        if !config.instructions.is_empty() {
            files.push((
                rules_dir.join("general.md"),
                format!("{}\n", config.instructions),
            ));
        }

        for rule in &config.rules {
            let filename = format!("{}.md", sanitize_name(&rule.name));
            files.push((rules_dir.join(filename), format!("{}\n", rule.content)));
        }

        Ok(files)
    }
}
