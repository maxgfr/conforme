use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::{write_if_changed, AiToolAdapter, WriteReport};
use crate::config::NormalizedConfig;

/// Zed AI adapter.
/// Uses .rules as primary file.
/// Fallback priority: .rules → .cursorrules → .windsurfrules → .clinerules → AGENTS.md → CLAUDE.md
/// Single file, no per-rule frontmatter.
pub struct ZedAdapter;

impl AiToolAdapter for ZedAdapter {
    fn name(&self) -> &str {
        "Zed AI"
    }

    fn id(&self) -> &str {
        "zed"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".rules").exists()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let rules_file = project_root.join(".rules");
        let instructions = if rules_file.exists() {
            std::fs::read_to_string(&rules_file)
                .with_context(|| format!("failed to read {}", rules_file.display()))?
                .trim()
                .to_string()
        } else {
            String::new()
        };

        Ok(NormalizedConfig {
            instructions,
            rules: Vec::new(),
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
        // Zed uses a single .rules file — no frontmatter, no activation modes.
        let mut content = config.instructions.clone();

        for rule in &config.rules {
            content.push_str("\n\n## ");
            content.push_str(&rule.name);
            content.push_str("\n\n");
            content.push_str(&rule.content);
        }

        Ok(vec![(
            project_root.join(".rules"),
            format!("{}\n", content.trim()),
        )])
    }
}
