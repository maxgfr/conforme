use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, WriteReport};
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
                            let patterns: Vec<String> =
                                apply_to.split(',').map(|s| s.trim().to_string()).collect();
                            ActivationMode::GlobMatch(patterns)
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
        let github_dir = project_root.join(".github");
        let mut files = Vec::new();

        // Write copilot-instructions.md with instructions + always-on rules
        let mut main_content = config.instructions.clone();
        let mut instruction_rules: Vec<&NormalizedRule> = Vec::new();

        for rule in &config.rules {
            match &rule.activation {
                ActivationMode::Always
                | ActivationMode::AgentDecision { .. }
                | ActivationMode::Manual => {
                    // Append to main instructions file
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

        // Write glob-based rules as .github/instructions/{name}.instructions.md
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
