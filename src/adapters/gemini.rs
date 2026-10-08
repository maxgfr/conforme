use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
use crate::config::NormalizedConfig;

/// An agent file in `.gemini/agents/` that is the user's, not a local agent
/// conforme reads and writes: a `_`-prefixed draft (Gemini skips those), a
/// remote (A2A) agent (`kind: remote`, or an agent card without `kind`), or a
/// file whose frontmatter is a YAML list of
/// remote agents. Such files are neither read nor cleaned as orphans.
fn is_gemini_user_agent(path: &Path) -> bool {
    if path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('_'))
    {
        return true;
    }
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    match crate::frontmatter::parse(&content) {
        // `kind` defaults to `remote` on a remote agent, so one that only
        // carries an agent card is remote too.
        Ok((fields, _)) => match fields.get("kind").and_then(|v| v.as_str()) {
            Some(kind) => kind != "local",
            None => ["agent_card_url", "agent_card_json"]
                .iter()
                .any(|key| fields.contains_key(*key)),
        },
        // Not a map: a list of remote agents (or a file Gemini rejects).
        Err(_) => true,
    }
}

/// The context file names Gemini CLI loads in this project: the project
/// `.gemini/settings.json` `context.fileName` (a string or a list), else
/// `GEMINI.md`.
fn context_file_names(project_root: &Path) -> Vec<String> {
    let configured =
        crate::json_settings::load(&project_root.join(".gemini").join("settings.json"))
            .ok()
            .flatten()
            .and_then(|settings| match settings.get("context")?.get("fileName")? {
                serde_json::Value::String(name) => Some(vec![name.clone()]),
                serde_json::Value::Array(names) => Some(
                    names
                        .iter()
                        .filter_map(|n| n.as_str().map(str::to_string))
                        .collect(),
                ),
                _ => None,
            })
            .filter(|names: &Vec<String>| !names.is_empty());
    configured.unwrap_or_else(|| vec!["GEMINI.md".to_string()])
}

/// The context file conforme writes the instructions to: the first name in
/// `context.fileName` other than `AGENTS.md` (`GEMINI.md` by default), or none
/// when Gemini only loads AGENTS.md.
pub fn instructions_file(project_root: &Path) -> Option<String> {
    context_file_names(project_root)
        .into_iter()
        .find(|name| name != "AGENTS.md")
}

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

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: false,
            skills: true,
            agents: true,
            mcp: true,
        }
    }

    /// `.gemini/settings.json` holds the user's general Gemini settings;
    /// conforme only merges `mcpServers` into it, so `remove`/`migrate`
    /// must never delete the file wholesale.
    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".gemini/settings.json"))
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![
            ManagedDir::files_except(
                project_root.join(".gemini").join("agents"),
                ".md",
                is_gemini_user_agent,
            ),
            ManagedDir::subdirs(project_root.join(".gemini").join("skills")),
        ]
    }

    fn reads_agents_md(&self, project_root: &Path) -> bool {
        context_file_names(project_root)
            .iter()
            .any(|name| name == "AGENTS.md")
    }

    fn source_files(&self, project_root: &Path) -> Vec<PathBuf> {
        context_file_names(project_root)
            .iter()
            .map(|name| project_root.join(name))
            .filter(|path| path.is_file())
            .collect()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // The context files `context.fileName` names (GEMINI.md by default);
        // AGENTS.md among them is parsed with the AGENTS.md convention below.
        let context_files = context_file_names(project_root);
        let mut parts = Vec::new();
        for name in context_files.iter().filter(|n| *n != "AGENTS.md") {
            let path = project_root.join(name);
            if path.is_file() {
                let text = std::fs::read_to_string(&path)
                    .with_context(|| format!("failed to read {}", path.display()))?;
                if !text.trim().is_empty() {
                    parts.push(text.trim().to_string());
                }
            }
        }
        let instructions = parts.join("\n\n");

        // Read skills, agents, and MCP so a Gemini project round-trips as a source.
        let skills =
            crate::skills::read_skills_from_dir(&project_root.join(".gemini").join("skills"))?;
        // Drafts and remote agents are not propagated to the other tools.
        let agent_files: Vec<PathBuf> =
            crate::skills::agent_files(&project_root.join(".gemini").join("agents"), false)?
                .into_iter()
                .filter(|p| !is_gemini_user_agent(p))
                .collect();
        let agents = crate::skills::read_agent_files(&agent_files)?;
        let mut mcp_servers = Vec::new();
        let settings_path = project_root.join(".gemini").join("settings.json");
        if settings_path.exists() {
            let settings = std::fs::read_to_string(&settings_path)?;
            mcp_servers = crate::mcp::canonicalize_env_refs(
                crate::mcp::parse_mcp_json(&settings)?,
                crate::mcp::EnvRefStyle::DollarOrBare,
            );
        }

        let config = NormalizedConfig {
            instructions,
            rules: Vec::new(),
            skills,
            agents,
            mcp_servers,
        };
        if !self.reads_agents_md(project_root) {
            return Ok(config);
        }
        let other_files = config.instructions.clone();
        let mut config =
            crate::markdown::read_native_agents_md(project_root, &["AGENTS.md"], config)?;
        if !other_files.is_empty() {
            config.instructions = if config.instructions.is_empty() {
                other_files
            } else {
                format!("{other_files}\n\n{}", config.instructions)
            };
        }
        Ok(config)
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

        let mut files = Vec::new();

        // Only generate the context file if there's actual content, and only
        // one Gemini loads (`context.fileName`); when that names AGENTS.md
        // alone, the AGENTS.md sync generates already carries everything.
        let trimmed = content.trim();
        if let Some(name) = instructions_file(project_root).filter(|_| !trimmed.is_empty()) {
            files.push((project_root.join(name), format!("{}\n", trimmed)));
        }

        // Generate skills as .gemini/skills/<name>/SKILL.md
        if !config.skills.is_empty() {
            files.extend(crate::skills::generate_gemini_skills(
                project_root,
                &config.skills,
            )?);
        }

        // Generate subagents as .gemini/agents/<name>.md
        if !config.agents.is_empty() {
            files.extend(crate::skills::generate_gemini_agents(
                project_root,
                &config.agents,
            )?);
        }

        // Merge MCP config into .gemini/settings.json (Gemini-specific format:
        // no type field, httpUrl for HTTP). `.gemini/settings.json` is the
        // general Gemini settings file (theme, context.fileName, …), so we read
        // any existing file and replace only the managed `mcpServers` key rather
        // than clobbering user-authored settings. Gemini-only server options
        // (`trust`, `timeout`, `includeTools`, …) survive the merge.
        files.extend(crate::json_settings::server_settings_file(
            &project_root.join(".gemini").join("settings.json"),
            "mcpServers",
            crate::mcp::build_gemini_mcp_object(&config.mcp_servers),
            crate::mcp::GEMINI_OWNED_SERVER_KEYS,
            &[],
        )?);

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ActivationMode, NormalizedConfig, NormalizedRule};
    use std::path::Path;

    fn make_adapter() -> GeminiAdapter {
        GeminiAdapter
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
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/GEMINI.md"));
        assert_eq!(files[0].1, "General instructions.\n");
    }

    #[test]
    fn test_generate_with_rules() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Top-level.".to_string(),
            rules: vec![
                NormalizedRule {
                    name: "TypeScript".to_string(),
                    content: "Use strict mode.".to_string(),
                    activation: ActivationMode::Always,
                },
                NormalizedRule {
                    name: "Security".to_string(),
                    content: "No eval.".to_string(),
                    activation: ActivationMode::Always,
                },
            ],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/GEMINI.md"));
        let content = &files[0].1;
        assert!(content.contains("Top-level."));
        assert!(content.contains("## TypeScript"));
        assert!(content.contains("Use strict mode."));
        assert!(content.contains("## Security"));
        assert!(content.contains("No eval."));
    }

    #[test]
    fn test_generate_with_mcp() {
        use crate::config::{McpTransport, NormalizedMcpServer};
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Hello.".to_string(),
            rules: vec![],
            mcp_servers: vec![NormalizedMcpServer {
                name: "fs".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@mcp/fs".to_string()],
                },
                env: std::collections::BTreeMap::new(),
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, Path::new("/tmp/test/GEMINI.md"));
        assert_eq!(files[1].0, Path::new("/tmp/test/.gemini/settings.json"));
        assert!(files[1].1.contains("mcpServers"));
        assert!(files[1].1.contains("\"fs\""));
        // Gemini MCP should NOT have "type" field
        assert!(!files[1].1.contains("\"type\""));
    }

    #[test]
    fn test_generate_with_skills() {
        use crate::config::NormalizedSkill;
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Hello.".to_string(),
            rules: vec![],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy the app".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec!["Bash".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 2);
        let skill_file = files
            .iter()
            .find(|(p, _)| p.to_string_lossy().contains(".gemini/skills/"))
            .unwrap();
        assert!(skill_file.0.ends_with("SKILL.md"));
        assert!(skill_file.1.contains("name: deploy"));
        assert!(skill_file.1.contains("description: Deploy the app"));
        // Gemini skills should NOT include allowed-tools
        assert!(!skill_file.1.contains("allowed-tools"));
    }

    #[test]
    fn test_generate_with_agents() {
        use crate::config::NormalizedAgent;
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "Hello.".to_string(),
            rules: vec![],
            agents: vec![NormalizedAgent {
                name: "reviewer".to_string(),
                description: "Code review".to_string(),
                content: "Review code.".to_string(),
                model: Some("gemini-3-flash".to_string()),
                tools: vec!["read_file".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 2);
        let agent_file = files
            .iter()
            .find(|(p, _)| p.to_string_lossy().contains(".gemini/agents/"))
            .unwrap();
        assert!(agent_file.0.ends_with("reviewer.md"));
        assert!(agent_file.1.contains("kind: local"));
        assert!(agent_file.1.contains("description: Code review"));
        assert!(agent_file.1.contains("model: gemini-3-flash"));
        assert!(agent_file.1.contains("- read_file"));
        assert!(agent_file.1.contains("Review code."));
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
        // Empty config should generate no files
        assert!(files.is_empty());
    }

    #[test]
    fn test_generate_preserves_existing_gemini_settings() {
        use crate::config::{McpTransport, NormalizedMcpServer};
        let adapter = make_adapter();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".gemini")).unwrap();
        std::fs::write(
            tmp.path().join(".gemini").join("settings.json"),
            r#"{"theme":"GitHub","context":{"fileName":"GEMINI.md"}}"#,
        )
        .unwrap();

        let config = NormalizedConfig {
            mcp_servers: vec![NormalizedMcpServer {
                name: "fs".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec![],
                },
                env: std::collections::BTreeMap::new(),
            }],
            ..Default::default()
        };
        let files = adapter.generate(tmp.path(), &config).unwrap();
        let settings = files
            .iter()
            .find(|(p, _)| p.ends_with("settings.json"))
            .unwrap();
        // User-authored keys must be preserved
        assert!(settings.1.contains("\"theme\""));
        assert!(settings.1.contains("GitHub"));
        assert!(settings.1.contains("\"context\""));
        assert!(settings.1.contains("\"fileName\""));
        // And the managed mcpServers key is written
        assert!(settings.1.contains("\"mcpServers\""));
        assert!(settings.1.contains("\"fs\""));
    }
}
