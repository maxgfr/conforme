use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir, WriteReport};
use crate::config::NormalizedConfig;

/// Mistral Vibe adapter.
///
/// Vibe reads `AGENTS.md` natively (from the working directory up to the
/// trusted root) and has no per-rule files. Skills live in
/// `.vibe/skills/<name>/SKILL.md` (then the shared `.agents/skills`),
/// subagents in `.vibe/agents/<name>.toml`, and MCP servers in the
/// `[[mcp_servers]]` array of the project `.vibe/config.toml`, a file that
/// also holds the user's own settings. Project files load only in a folder
/// the user trusted.
pub struct VibeAdapter;

/// Vibe reads `AGENTS.md` and no other instruction file.
const INSTRUCTION_FILES: &[&str] = &["AGENTS.md"];

fn config_path(project_root: &Path) -> PathBuf {
    project_root.join(".vibe").join("config.toml")
}

/// The skills in `.vibe/skills`; the shared `.agents/skills` root is read
/// only when there are none (as for DeepSeek), so a Codex, Zed or Amp copy
/// there is not taken for the source's own.
fn own_skills(project_root: &Path) -> Result<Vec<crate::config::NormalizedSkill>> {
    crate::skills::read_skills_from_dir(&project_root.join(".vibe").join("skills"))
}

impl AiToolAdapter for VibeAdapter {
    fn name(&self) -> &str {
        "Mistral Vibe"
    }

    fn id(&self) -> &str {
        "vibe"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".vibe").is_dir()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: false,
            skills: true,
            agents: true,
            mcp: true,
        }
    }

    fn reads_agents_md(&self, _project_root: &Path) -> bool {
        true
    }

    fn source_files(&self, project_root: &Path) -> Vec<PathBuf> {
        let mut files = crate::adapters::first_existing_file(project_root, INSTRUCTION_FILES);
        let shared = project_root.join(".agents").join("skills");
        if shared.is_dir() && own_skills(project_root).is_ok_and(|s| s.is_empty()) {
            files.push(shared);
        }
        files
    }

    /// `.vibe/config.toml` holds the user's models, providers, tools and
    /// theme; conforme only merges `[[mcp_servers]]` into it.
    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".vibe/config.toml"))
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![
            ManagedDir::subdirs(project_root.join(".vibe").join("skills")),
            // An agent file that is not a subagent is a mode the user defined.
            ManagedDir::files_except(project_root.join(".vibe").join("agents"), ".toml", |path| {
                !crate::skills::is_vibe_subagent_file(path)
            }),
        ]
    }

    fn write(&self, project_root: &Path, config: &NormalizedConfig) -> Result<WriteReport> {
        let mut report = WriteReport {
            files_written: Vec::new(),
            files_unchanged: Vec::new(),
        };
        for (path, content) in self.generate(project_root, config)? {
            if self.is_shared_file(&path) {
                crate::adapters::write_if_changed_atomic(&path, &content, &mut report)?;
            } else {
                crate::adapters::write_if_changed(&path, &content, &mut report)?;
            }
        }
        Ok(report)
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut skills = own_skills(project_root)?;
        if skills.is_empty() {
            skills =
                crate::skills::read_skills_from_dir(&project_root.join(".agents").join("skills"))?;
        }
        let path = config_path(project_root);
        let mcp_servers = if path.exists() {
            crate::mcp::parse_vibe_mcp_toml(&std::fs::read_to_string(&path)?)?
        } else {
            Vec::new()
        };
        crate::markdown::read_native_agents_md(
            project_root,
            INSTRUCTION_FILES,
            NormalizedConfig {
                skills,
                agents: crate::skills::read_vibe_agents(project_root)?,
                mcp_servers,
                ..Default::default()
            },
        )
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        // Vibe reads AGENTS.md itself: no instruction file is written.
        let mut files = crate::skills::generate_vibe_skills(project_root, &config.skills)?;
        files.extend(crate::skills::generate_vibe_agents(
            project_root,
            &config.agents,
        )?);
        // A source with no MCP server leaves the user's config untouched.
        if !config.mcp_servers.is_empty() {
            let path = config_path(project_root);
            let existing = if path.exists() {
                std::fs::read_to_string(&path)?
            } else {
                String::new()
            };
            files.push((
                path,
                crate::mcp::merge_vibe_mcp_toml(&existing, &config.mcp_servers)?,
            ));
        }
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{NormalizedAgent, NormalizedSkill};

    #[test]
    fn test_generate_empty_config() {
        let files = VibeAdapter
            .generate(Path::new("/tmp/test"), &NormalizedConfig::default())
            .unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_generate_skill_and_subagent() {
        let config = NormalizedConfig {
            skills: vec![NormalizedSkill {
                name: "Deploy App".to_string(),
                description: "Deploy".to_string(),
                content: "Run.".to_string(),
                manual_invocation: true,
                ..Default::default()
            }],
            agents: vec![NormalizedAgent {
                name: "reviewer".to_string(),
                description: "Review".to_string(),
                content: "Review the diff.".to_string(),
                model: Some("sonnet".to_string()),
                tools: vec!["Read".to_string(), "Bash".to_string(), "Edit".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = VibeAdapter
            .generate(Path::new("/tmp/test"), &config)
            .unwrap();

        let skill = files
            .iter()
            .find(|(p, _)| p.ends_with(".vibe/skills/deploy-app/SKILL.md"))
            .unwrap();
        assert!(skill.1.contains("name: deploy-app"), "{}", skill.1);
        assert!(
            skill.1.contains("disable-model-invocation: true"),
            "{}",
            skill.1
        );

        let agent = files
            .iter()
            .find(|(p, _)| p.ends_with(".vibe/agents/reviewer.toml"))
            .unwrap();
        let value: toml::Value = agent.1.parse().unwrap();
        assert_eq!(value["agent_type"].as_str(), Some("subagent"));
        assert_eq!(value["description"].as_str(), Some("Review"));
        assert_eq!(value["instructions"].as_str(), Some("Review the diff."));
        assert_eq!(
            value["enabled_tools"],
            toml::Value::Array(vec!["read_file".into(), "bash".into(), "edit".into()])
        );
        assert!(
            value.get("model").is_none(),
            "a Claude alias is no Vibe model"
        );
    }
}
