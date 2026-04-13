use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::{write_if_changed, AiToolAdapter, WriteReport};
use crate::config::NormalizedConfig;

/// OpenCode adapter.
/// OpenCode reads AGENTS.md as primary, falls back to CLAUDE.md.
/// It also scans skills from .opencode/skills/, .claude/skills/, .agents/skills/.
/// Since AGENTS.md is our source of truth and OpenCode reads it natively,
/// this adapter is mostly a pass-through.
pub struct OpenCodeAdapter;

impl AiToolAdapter for OpenCodeAdapter {
    fn name(&self) -> &str {
        "OpenCode"
    }

    fn id(&self) -> &str {
        "opencode"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("opencode.json").exists() || project_root.join(".opencode").is_dir()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // OpenCode reads AGENTS.md natively
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
        // OpenCode reads AGENTS.md natively — same re-export as Codex
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
