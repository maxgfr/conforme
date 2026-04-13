use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{write_if_changed, AiToolAdapter, WriteReport};
use crate::config::{sanitize_name, ActivationMode, NormalizedConfig, NormalizedRule};
use crate::frontmatter;

/// Continue.dev adapter.
/// Rules in .continue/rules/*.md with optional YAML frontmatter:
/// name, globs (array), alwaysApply, description.
pub struct ContinueDevAdapter;

impl AiToolAdapter for ContinueDevAdapter {
    fn name(&self) -> &str {
        "Continue.dev"
    }

    fn id(&self) -> &str {
        "continue"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".continue").is_dir()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let rules_dir = project_root.join(".continue").join("rules");
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

                    let activation = parse_continue_activation(&fields);

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
        let rules_dir = project_root.join(".continue").join("rules");
        let mut files = Vec::new();

        if !config.instructions.is_empty() {
            let mut fields = BTreeMap::new();
            fields.insert(
                "name".to_string(),
                serde_yaml_ng::Value::String("General".to_string()),
            );
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(true));
            let content = frontmatter::serialize(&fields, &format!("{}\n", config.instructions))?;
            files.push((rules_dir.join("general.md"), content));
        }

        for rule in &config.rules {
            let filename = format!("{}.md", sanitize_name(&rule.name));
            let fields = build_continue_fields(rule);
            let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
            files.push((rules_dir.join(filename), content));
        }

        Ok(files)
    }
}

fn parse_continue_activation(fields: &BTreeMap<String, serde_yaml_ng::Value>) -> ActivationMode {
    let always_apply = fields
        .get("alwaysApply")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Continue uses globs as an array, not a comma-separated string
    let globs = fields.get("globs").and_then(|v| {
        if let serde_yaml_ng::Value::Sequence(arr) = v {
            let patterns: Vec<String> = arr
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect();
            if patterns.is_empty() {
                None
            } else {
                Some(patterns)
            }
        } else if let Some(s) = v.as_str() {
            let patterns: Vec<String> = s
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if patterns.is_empty() {
                None
            } else {
                Some(patterns)
            }
        } else {
            None
        }
    });

    let description = fields.get("description").and_then(|v| v.as_str());

    if always_apply {
        ActivationMode::Always
    } else if let Some(g) = globs {
        ActivationMode::GlobMatch(g)
    } else if let Some(desc) = description {
        ActivationMode::AgentDecision {
            description: desc.to_string(),
        }
    } else {
        ActivationMode::Always
    }
}

fn build_continue_fields(rule: &NormalizedRule) -> BTreeMap<String, serde_yaml_ng::Value> {
    let mut fields = BTreeMap::new();

    fields.insert(
        "name".to_string(),
        serde_yaml_ng::Value::String(rule.name.clone()),
    );

    match &rule.activation {
        ActivationMode::Always => {
            fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(true));
        }
        ActivationMode::GlobMatch(globs) => {
            let yaml_globs: Vec<serde_yaml_ng::Value> = globs
                .iter()
                .map(|g| serde_yaml_ng::Value::String(g.clone()))
                .collect();
            fields.insert(
                "globs".to_string(),
                serde_yaml_ng::Value::Sequence(yaml_globs),
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
