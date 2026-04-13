use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::{write_if_changed, AiToolAdapter, WriteReport};
use crate::config::NormalizedConfig;

/// Gemini CLI adapter.
/// Uses GEMINI.md discovered hierarchically.
/// Supports @path/to/file.md imports. No per-rule files — single GEMINI.md.
pub struct GeminiAdapter;

impl AiToolAdapter for GeminiAdapter {
    fn name(&self) -> &str {
        "Gemini CLI"
    }

    fn id(&self) -> &str {
        "gemini"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("GEMINI.md").exists() || project_root.join(".gemini").is_dir()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let gemini_md = project_root.join("GEMINI.md");
        let instructions = if gemini_md.exists() {
            std::fs::read_to_string(&gemini_md)
                .with_context(|| format!("failed to read {}", gemini_md.display()))?
                .trim()
                .to_string()
        } else {
            String::new()
        };

        Ok(NormalizedConfig {
            instructions,
            rules: Vec::new(),
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
        // Gemini uses a single GEMINI.md — no per-rule frontmatter, no activation modes.
        // All content is merged into one file.
        let mut content = config.instructions.clone();

        for rule in &config.rules {
            content.push_str("\n\n## ");
            content.push_str(&rule.name);
            content.push_str("\n\n");
            content.push_str(&rule.content);
        }

        Ok(vec![(
            project_root.join("GEMINI.md"),
            format!("{}\n", content.trim()),
        )])
    }
}
