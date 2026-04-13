use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
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
            ..Default::default()
        })
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        // Codex reads AGENTS.md natively — no need to re-generate it
        // since AGENTS.md is already our source of truth.
        // Only generate skills as .agents/skills/<name>/SKILL.md (Codex format)
        let files = crate::skills::generate_codex_skills(project_root, &config.skills)?;

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ActivationMode, NormalizedConfig, NormalizedRule, NormalizedSkill};
    use std::path::Path;

    fn make_adapter() -> CodexAdapter {
        CodexAdapter
    }

    #[test]
    fn test_generate_instructions_only() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "General instructions.".to_string(),
            rules: vec![],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        // Codex reads AGENTS.md natively, no files generated without skills
        assert!(files.is_empty());
    }

    #[test]
    fn test_generate_with_rules() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Top-level.".to_string(),
            rules: vec![NormalizedRule {
                name: "TypeScript".to_string(),
                content: "Use strict mode.".to_string(),
                activation: ActivationMode::Always,
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        // No files generated — Codex reads AGENTS.md directly
        assert!(files.is_empty());
    }

    #[test]
    fn test_generate_with_skills() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Main.".to_string(),
            rules: vec![],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec!["Bash".to_string()],
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].0,
            Path::new("/tmp/test/.agents/skills/deploy/SKILL.md")
        );
        assert!(files[0].1.contains("name: deploy"));
        assert!(files[0].1.contains("description: Deploy"));
        assert!(files[0].1.contains("Run deploy."));
    }

    #[test]
    fn test_generate_empty_config() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert!(files.is_empty());
    }
}
