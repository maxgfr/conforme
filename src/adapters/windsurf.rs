use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, WriteReport};
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

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let rules_dir = project_root.join(".windsurf").join("rules");
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
        let rules_dir = project_root.join(".windsurf").join("rules");
        let mut files = Vec::new();

        // Write instructions as general.md
        if !config.instructions.is_empty() {
            let mut fields = BTreeMap::new();
            fields.insert(
                "trigger".to_string(),
                serde_yaml_ng::Value::String("always_on".to_string()),
            );
            let content = frontmatter::serialize(&fields, &format!("{}\n", config.instructions))?;
            files.push((rules_dir.join("general.md"), content));
        }

        // Write each rule
        for rule in &config.rules {
            let filename = format!("{}.md", sanitize_name(&rule.name));
            let fields = build_windsurf_fields(rule);
            let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
            files.push((rules_dir.join(filename), content));
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
            let patterns: Vec<String> = globs.split(',').map(|s| s.trim().to_string()).collect();
            ActivationMode::GlobMatch(patterns)
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
