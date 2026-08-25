use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::NormalizedConfig;

/// DeepSeek Harness (`dsh`) adapter.
///
/// The harness loads workspace instructions from the project root through its
/// `@deepseek-ai/dsh-agent-instructions` plugin, whose default candidate list is
/// `AGENTS.md` then `CLAUDE.md` (with `AGENTS.local.md` / `CLAUDE.local.md` as
/// the per-directory overlay). Skills are discovered by the filesystem provider
/// at `<projectRoot>/.dsh/skills` (rank 100) and `<projectRoot>/.agents/skills`
/// (rank 200).
///
/// MCP servers are declared as `@deepseek-ai/dsh-mcp-client` plugin entries in
/// the user-level `cordis.patch.yml` under `$DSH_HOME` (or a profile), so there
/// is no project-scoped MCP file for conforme to generate.
pub struct DeepSeekAdapter;

impl AiToolAdapter for DeepSeekAdapter {
    fn name(&self) -> &str {
        "DeepSeek Harness"
    }

    fn id(&self) -> &str {
        "deepseek"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".dsh").is_dir()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: false,
            skills: true,
            agents: false,
            mcp: false,
        }
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<PathBuf> {
        vec![project_root.join(".dsh").join("skills")]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // Mirrors the harness default `instructionFileCandidates`: AGENTS.md
        // first, CLAUDE.md second.
        let instructions = ["AGENTS.md", "CLAUDE.md"]
            .iter()
            .map(|name| project_root.join(name))
            .find(|path| path.exists())
            .map(std::fs::read_to_string)
            .transpose()?
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        // `.dsh/skills` is the harness-native project root; `.agents/skills` is
        // the shared root it also scans, used as a fallback when a project only
        // carries the shared layout.
        let mut skills =
            crate::skills::read_skills_from_dir(&project_root.join(".dsh").join("skills"))?;
        if skills.is_empty() {
            skills =
                crate::skills::read_skills_from_dir(&project_root.join(".agents").join("skills"))?;
        }

        Ok(NormalizedConfig {
            instructions,
            rules: Vec::new(),
            skills,
            ..Default::default()
        })
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        // The harness reads AGENTS.md natively — only skills need generating.
        crate::skills::generate_deepseek_skills(project_root, &config.skills)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ActivationMode, NormalizedConfig, NormalizedRule, NormalizedSkill};

    fn make_adapter() -> DeepSeekAdapter {
        DeepSeekAdapter
    }

    #[test]
    fn test_generate_instructions_only() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "General instructions.".to_string(),
            rules: vec![NormalizedRule {
                name: "TypeScript".to_string(),
                content: "Use strict mode.".to_string(),
                activation: ActivationMode::Always,
            }],
            ..Default::default()
        };
        // AGENTS.md is read natively, so nothing is generated for it.
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_generate_with_skills() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            skills: vec![NormalizedSkill {
                name: "Deploy App".to_string(),
                description: "Deploy the app".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec!["Bash".to_string()],
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].0,
            Path::new("/tmp/test/.dsh/skills/deploy-app/SKILL.md")
        );
        assert!(files[0].1.contains("name: deploy-app"));
        assert!(files[0].1.contains("description: Deploy the app"));
        // `allowed-tools` is not a field the harness skill provider interprets.
        assert!(!files[0].1.contains("allowed-tools"));
    }

    #[test]
    fn test_detect() {
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        assert!(!adapter.detect(tmp.path()));
        std::fs::create_dir_all(tmp.path().join(".dsh")).unwrap();
        assert!(adapter.detect(tmp.path()));
    }

    #[test]
    fn test_read_falls_back_to_claude_md() {
        let adapter = make_adapter();

        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), "From CLAUDE.md").unwrap();
        assert_eq!(
            adapter.read(tmp.path()).unwrap().instructions,
            "From CLAUDE.md"
        );

        let tmp2 = tempfile::tempdir().unwrap();
        std::fs::write(tmp2.path().join("AGENTS.md"), "Primary").unwrap();
        std::fs::write(tmp2.path().join("CLAUDE.md"), "Fallback").unwrap();
        assert_eq!(adapter.read(tmp2.path()).unwrap().instructions, "Primary");
    }

    #[test]
    fn test_read_skills_prefers_dsh_root() {
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        let dsh = tmp.path().join(".dsh/skills/deploy");
        std::fs::create_dir_all(&dsh).unwrap();
        std::fs::write(
            dsh.join("SKILL.md"),
            "---\nname: deploy\ndescription: Deploy\n---\n\nRun deploy.\n",
        )
        .unwrap();

        let shared = tmp.path().join(".agents/skills/other");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(
            shared.join("SKILL.md"),
            "---\nname: other\ndescription: Other\n---\n\nBody.\n",
        )
        .unwrap();

        let config = adapter.read(tmp.path()).unwrap();
        assert_eq!(config.skills.len(), 1);
        assert_eq!(config.skills[0].name, "deploy");
    }

    #[test]
    fn test_read_falls_back_to_shared_skills_root() {
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        let shared = tmp.path().join(".agents/skills/other");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(
            shared.join("SKILL.md"),
            "---\nname: other\ndescription: Other\n---\n\nBody.\n",
        )
        .unwrap();

        let config = adapter.read(tmp.path()).unwrap();
        assert_eq!(config.skills.len(), 1);
        assert_eq!(config.skills[0].name, "other");
    }
}
