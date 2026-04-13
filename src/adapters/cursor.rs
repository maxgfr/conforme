use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, WriteReport};
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};
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
        let rules_dir = project_root.join(".cursor").join("rules");
        let mut files = Vec::new();

        // Write instructions as general.mdc
        if !config.instructions.is_empty() {
            let mut fields = BTreeMap::new();
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(true));
            let content = frontmatter::serialize(&fields, &format!("{}\n", config.instructions))?;
            files.push((rules_dir.join("general.mdc"), content));
        }

        // Write each rule
        for rule in &config.rules {
            let filename = format!("{}.mdc", sanitize_name(&rule.name));
            let fields = build_cursor_fields(rule);
            let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
            files.push((rules_dir.join(filename), content));
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
        let patterns: Vec<String> = g.split(',').map(|s| s.trim().to_string()).collect();
        ActivationMode::GlobMatch(patterns)
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
