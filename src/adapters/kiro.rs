use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{write_if_changed, AiToolAdapter, WriteReport};
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};
use crate::frontmatter;

/// Kiro (AWS) adapter.
/// Rules in .kiro/steering/*.md with YAML frontmatter:
/// - inclusion: always | fileMatch | manual | auto
/// - fileMatchPattern: glob patterns (string or array)
/// - name: required for auto inclusion
/// - description: required for auto inclusion
pub struct KiroAdapter;

impl AiToolAdapter for KiroAdapter {
    fn name(&self) -> &str {
        "Kiro"
    }

    fn id(&self) -> &str {
        "kiro"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".kiro").is_dir()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let steering_dir = project_root.join(".kiro").join("steering");
        if steering_dir.is_dir() {
            for entry in std::fs::read_dir(&steering_dir)? {
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

                    let activation = parse_kiro_activation(&fields);

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
        let steering_dir = project_root.join(".kiro").join("steering");
        let mut files = Vec::new();

        if !config.instructions.is_empty() {
            let mut fields = BTreeMap::new();
            fields.insert(
                "inclusion".to_string(),
                serde_yaml_ng::Value::String("always".to_string()),
            );
            let content = frontmatter::serialize(&fields, &format!("{}\n", config.instructions))?;
            files.push((steering_dir.join("general.md"), content));
        }

        for rule in &config.rules {
            let filename = format!("{}.md", sanitize_name(&rule.name));
            let fields = build_kiro_fields(rule);
            let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
            files.push((steering_dir.join(filename), content));
        }

        Ok(files)
    }
}

fn parse_kiro_activation(fields: &BTreeMap<String, serde_yaml_ng::Value>) -> ActivationMode {
    let inclusion = fields
        .get("inclusion")
        .and_then(|v| v.as_str())
        .unwrap_or("always");

    match inclusion {
        "always" => ActivationMode::Always,
        "fileMatch" => {
            let patterns = fields.get("fileMatchPattern").and_then(|v| match v {
                serde_yaml_ng::Value::String(s) => Some(vec![s.clone()]),
                serde_yaml_ng::Value::Sequence(arr) => {
                    let p: Vec<String> = arr
                        .iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .filter(|s| !s.is_empty())
                        .collect();
                    if p.is_empty() {
                        None
                    } else {
                        Some(p)
                    }
                }
                _ => None,
            });
            match patterns {
                Some(p) => ActivationMode::GlobMatch(p),
                None => ActivationMode::Always,
            }
        }
        "auto" => {
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

fn build_kiro_fields(rule: &NormalizedRule) -> BTreeMap<String, serde_yaml_ng::Value> {
    let mut fields = BTreeMap::new();

    match &rule.activation {
        ActivationMode::Always => {
            fields.insert(
                "inclusion".to_string(),
                serde_yaml_ng::Value::String("always".to_string()),
            );
        }
        ActivationMode::GlobMatch(globs) => {
            fields.insert(
                "inclusion".to_string(),
                serde_yaml_ng::Value::String("fileMatch".to_string()),
            );
            let yaml_globs: Vec<serde_yaml_ng::Value> = globs
                .iter()
                .map(|g| serde_yaml_ng::Value::String(g.clone()))
                .collect();
            fields.insert(
                "fileMatchPattern".to_string(),
                serde_yaml_ng::Value::Sequence(yaml_globs),
            );
        }
        ActivationMode::AgentDecision { description } => {
            fields.insert(
                "inclusion".to_string(),
                serde_yaml_ng::Value::String("auto".to_string()),
            );
            fields.insert(
                "name".to_string(),
                serde_yaml_ng::Value::String(rule.name.clone()),
            );
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(description.clone()),
            );
        }
        ActivationMode::Manual => {
            fields.insert(
                "inclusion".to_string(),
                serde_yaml_ng::Value::String("manual".to_string()),
            );
        }
    }

    fields
}
