use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::{write_if_changed, AiToolAdapter, WriteReport};
use crate::config::NormalizedConfig;

/// OpenAI Codex CLI adapter.
/// Codex reads AGENTS.md natively as its primary instruction file.
/// It also supports AGENTS.override.md and config.toml for settings.
/// Since AGENTS.md is already our source of truth, this adapter simply
/// ensures AGENTS.md exists (a no-op write essentially).
pub struct CodexAdapter;

impl AiToolAdapter for CodexAdapter {
    fn name(&self) -> &str {
        "Codex CLI"
    }

    fn id(&self) -> &str {
        "codex"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".codex").is_dir()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // Codex reads AGENTS.md directly — same as our source of truth
        let agents_md = project_root.join("AGENTS.md");
        let instructions = if agents_md.exists() {
            std::fs::read_to_string(&agents_md)?.trim().to_string()
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
        // Codex reads AGENTS.md natively. We re-export the full content
        // as AGENTS.md to ensure it stays in sync with our parsed version.
        // Rules are embedded as sections since Codex doesn't have per-rule files.
        let mut content = config.instructions.clone();

        for rule in &config.rules {
            content.push_str("\n\n## ");
            content.push_str(&rule.name);
            content.push_str("\n\n");
            content.push_str(&rule.content);
        }

        Ok(vec![(
            project_root.join("AGENTS.md"),
            format!("{}\n", content.trim()),
        )])
    }
}
