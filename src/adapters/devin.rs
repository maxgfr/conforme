use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
use crate::config::{
    join_flat_globs, rule_file_name, split_globs, ActivationMode, NormalizedConfig, NormalizedRule,
};
use crate::frontmatter;

/// Devin Desktop (formerly Windsurf) and the Devin CLI harness its Devin Local
/// agent runs on. Cascade, Windsurf's original agent, was removed in Devin
/// Desktop v3.9.19, so the Devin CLI documentation describes what is read.
///
/// conforme writes the preferred `.devin/` layout. Devin also still loads the
/// legacy `.windsurf/rules/` and `.windsurf/skills/` (both, not one or the
/// other), so `read()` merges them and sync cleans conforme's old copies
/// there rather than leaving the agent two of everything.
pub struct DevinAdapter;

fn devin_dir(project_root: &Path) -> PathBuf {
    project_root.join(".devin")
}

fn legacy_dir(project_root: &Path) -> PathBuf {
    project_root.join(".windsurf")
}

fn mcp_path(project_root: &Path) -> PathBuf {
    devin_dir(project_root).join("mcp_config.json")
}

impl AiToolAdapter for DevinAdapter {
    fn name(&self) -> &str {
        "Devin Desktop"
    }

    fn id(&self) -> &str {
        "devin"
    }

    fn detect(&self, project_root: &Path) -> bool {
        devin_dir(project_root).is_dir()
            || legacy_dir(project_root).is_dir()
            || project_root.join(".windsurfrules").exists()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: true,
            skills: true,
            // Devin subagents (`.devin/agents/`) are experimental upstream;
            // conforme does not generate them yet.
            agents: false,
            mcp: true,
        }
    }

    /// `.devin/mcp_config.json` also holds per-server OAuth settings and
    /// `disabled` flags, so it is merged and never deleted wholesale.
    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".devin/mcp_config.json"))
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        let mut dirs = vec![
            ManagedDir::files(devin_dir(project_root).join("rules"), ".md"),
            ManagedDir::subdirs(devin_dir(project_root).join("skills")),
        ];
        // Devin loads `.windsurf/rules/` next to `.devin/rules/`, so a rule an
        // earlier sync wrote there would be applied twice: a legacy file is
        // removed once the same file is generated in `.devin/rules/`. Other
        // legacy rules are the user's and stay.
        let legacy_rules = legacy_dir(project_root).join("rules");
        if legacy_rules.is_dir() {
            dirs.push(ManagedDir::legacy_files(
                legacy_rules,
                devin_dir(project_root).join("rules"),
                ".md",
            ));
        }
        let legacy_skills = legacy_dir(project_root).join("skills");
        if legacy_skills.is_dir() {
            dirs.push(ManagedDir::legacy_skills(
                legacy_skills,
                devin_dir(project_root).join("skills"),
            ));
        }
        dirs
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = None;
        let mut rules: Vec<NormalizedRule> = Vec::new();

        // `.devin/rules/` first: on a name clash it is the copy that wins.
        for dir in [devin_dir(project_root), legacy_dir(project_root)] {
            for path in crate::adapters::collect_rule_files(&dir.join("rules"), "md")? {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("failed to read {}", path.display()))?;
                let (fields, body) = frontmatter::parse(&content)?;
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let activation = parse_devin_activation(&fields);

                if name == "general" && activation == ActivationMode::Always {
                    instructions.get_or_insert_with(|| body.trim().to_string());
                } else if !rules.iter().any(|r| r.name == name) {
                    rules.push(NormalizedRule {
                        name,
                        content: body.trim().to_string(),
                        activation,
                    });
                }
            }
        }

        // The legacy root `.windsurfrules` is still read as workspace rules.
        if instructions.is_none() {
            let windsurfrules = project_root.join(".windsurfrules");
            if windsurfrules.is_file() {
                instructions = Some(std::fs::read_to_string(&windsurfrules)?.trim().to_string());
            }
        }

        let mut skills =
            crate::skills::read_skills_from_dir(&devin_dir(project_root).join("skills"))?;
        for skill in crate::skills::read_skills_from_dir(&legacy_dir(project_root).join("skills"))?
        {
            if !skills.iter().any(|s| s.name == skill.name) {
                skills.push(skill);
            }
        }

        let mut mcp_servers = Vec::new();
        let mcp_path = mcp_path(project_root);
        if mcp_path.exists() {
            mcp_servers = crate::mcp::canonicalize_env_refs(
                crate::mcp::parse_mcp_json(&std::fs::read_to_string(&mcp_path)?)?,
                crate::mcp::EnvRefStyle::EnvColon,
            );
        }

        Ok(NormalizedConfig {
            instructions: instructions.unwrap_or_default(),
            rules,
            skills,
            mcp_servers,
            ..Default::default()
        })
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        let rules_dir = devin_dir(project_root).join("rules");
        let mut files = Vec::new();

        if !config.instructions.is_empty() {
            let mut fields = BTreeMap::new();
            fields.insert(
                "trigger".to_string(),
                serde_yaml_ng::Value::String("always_on".to_string()),
            );
            let content = frontmatter::serialize(&fields, &format!("{}\n", config.instructions))?;
            files.push((rules_dir.join("general.md"), content));
        }

        for rule in &config.rules {
            let filename = format!("{}.md", rule_file_name(&rule.name));
            let fields = build_devin_fields(rule);
            let content = frontmatter::serialize(&fields, &format!("{}\n", rule.content))?;
            files.push((rules_dir.join(filename), content));
        }

        files.extend(crate::skills::generate_devin_skills(
            &devin_dir(project_root).join("skills"),
            &config.skills,
        )?);

        // Devin Local reads project MCP servers from `.devin/mcp_config.json`
        // (since Devin CLI v3000.3; earlier versions kept them in the main
        // config file).
        files.extend(crate::json_settings::server_settings_file(
            &mcp_path(project_root),
            "mcpServers",
            crate::mcp::build_devin_servers_object(&config.mcp_servers),
            crate::mcp::DEVIN_OWNED_SERVER_KEYS,
            &[],
        )?);

        Ok(files)
    }
}

fn parse_devin_activation(fields: &BTreeMap<String, serde_yaml_ng::Value>) -> ActivationMode {
    let trigger = fields
        .get("trigger")
        .and_then(|v| v.as_str())
        .unwrap_or("always_on");

    match trigger {
        "always_on" => ActivationMode::Always,
        "glob" => {
            let patterns = match fields.get("globs") {
                Some(serde_yaml_ng::Value::Sequence(seq)) => seq
                    .iter()
                    .filter_map(|v| v.as_str())
                    .flat_map(split_globs)
                    .collect(),
                Some(value) => split_globs(value.as_str().unwrap_or("")),
                None => Vec::new(),
            };
            if patterns.is_empty() {
                ActivationMode::Always
            } else {
                ActivationMode::GlobMatch(patterns)
            }
        }
        // `agent` is Devin's newer spelling: "the model independently decides
        // whether to apply the rule based on context and the rule's
        // description".
        "model_decision" | "agent" => {
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

fn build_devin_fields(rule: &NormalizedRule) -> BTreeMap<String, serde_yaml_ng::Value> {
    let mut fields = BTreeMap::new();
    let mut set = |key: &str, value: String| {
        fields.insert(key.to_string(), serde_yaml_ng::Value::String(value));
    };

    match &rule.activation {
        ActivationMode::Always => {
            set("trigger", "always_on".to_string());
            set("description", rule.name.clone());
        }
        ActivationMode::GlobMatch(globs) => {
            set("trigger", "glob".to_string());
            set("description", rule.name.clone());
            // One comma-separated string, the historical Windsurf form; brace
            // groups are expanded so the comma split keeps them whole.
            set("globs", join_flat_globs(globs, ", "));
        }
        ActivationMode::AgentDecision { description } => {
            set("trigger", "model_decision".to_string());
            set(
                "description",
                crate::skills::description_or_name(description, &rule.name),
            );
        }
        ActivationMode::Manual => {
            set("trigger", "manual".to_string());
            set("description", rule.name.clone());
        }
    }

    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpTransport, NormalizedMcpServer, NormalizedSkill};
    use std::path::Path;

    fn test_config() -> NormalizedConfig {
        NormalizedConfig {
            instructions: "Be helpful.".to_string(),
            rules: vec![
                NormalizedRule {
                    name: "TypeScript".to_string(),
                    content: "Use strict mode.".to_string(),
                    activation: ActivationMode::Always,
                },
                NormalizedRule {
                    name: "API Rules".to_string(),
                    content: "Follow REST.".to_string(),
                    activation: ActivationMode::GlobMatch(vec!["src/api/**".to_string()]),
                },
                NormalizedRule {
                    name: "Smart Rule".to_string(),
                    content: "Decide wisely.".to_string(),
                    activation: ActivationMode::AgentDecision {
                        description: "API context".to_string(),
                    },
                },
                NormalizedRule {
                    name: "Manual Rule".to_string(),
                    content: "Only when asked.".to_string(),
                    activation: ActivationMode::Manual,
                },
            ],
            ..Default::default()
        }
    }

    fn skill(name: &str) -> NormalizedSkill {
        NormalizedSkill {
            name: name.to_string(),
            description: "Deploy the app".to_string(),
            content: "Run deploy.".to_string(),
            allowed_tools: vec!["Bash".to_string()],
            ..Default::default()
        }
    }

    #[test]
    fn test_generate_instructions_only() {
        let config = NormalizedConfig {
            instructions: "Be helpful.".to_string(),
            ..Default::default()
        };
        let files = DevinAdapter
            .generate(Path::new("/tmp/test"), &config)
            .unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.devin/rules/general.md"));
        assert!(files[0].1.contains("trigger: always_on"));
        assert!(files[0].1.contains("Be helpful."));
    }

    #[test]
    fn test_generate_rules_per_activation() {
        let files = DevinAdapter
            .generate(Path::new("/tmp/test"), &test_config())
            .unwrap();
        let find = |suffix: &str| &files.iter().find(|(p, _)| p.ends_with(suffix)).unwrap().1;

        let glob = find("api-rules.md");
        assert!(glob.contains("trigger: glob"));
        assert!(glob.contains("src/api/**"));
        let smart = find("smart-rule.md");
        assert!(smart.contains("trigger: model_decision"));
        assert!(smart.contains("description: API context"));
        assert!(find("manual-rule.md").contains("trigger: manual"));
    }

    #[test]
    fn test_writes_devin_even_for_a_legacy_windsurf_project() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".windsurf")).unwrap();
        assert!(DevinAdapter.detect(tmp.path()));

        let files = DevinAdapter.generate(tmp.path(), &test_config()).unwrap();
        assert!(files
            .iter()
            .all(|(p, _)| p.starts_with(tmp.path().join(".devin"))));
    }

    #[test]
    fn test_read_merges_devin_and_legacy_rules_recursively() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // `.devin/` exists for other reasons (MCP config) and holds no rules:
        // the legacy rules must still be read, not an empty config.
        std::fs::create_dir_all(root.join(".devin")).unwrap();
        std::fs::write(root.join(".devin/mcp_config.json"), "{}").unwrap();
        std::fs::create_dir_all(root.join(".windsurf/rules/backend")).unwrap();
        std::fs::write(
            root.join(".windsurf/rules/general.md"),
            "---\ntrigger: always_on\n---\nLegacy general.\n",
        )
        .unwrap();
        std::fs::write(
            root.join(".windsurf/rules/backend/api.md"),
            "---\ntrigger: agent\ndescription: API work\n---\nAPI rule.\n",
        )
        .unwrap();

        let config = DevinAdapter.read(root).unwrap();
        assert_eq!(config.instructions, "Legacy general.");
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].name, "api");
        assert_eq!(
            config.rules[0].activation,
            ActivationMode::AgentDecision {
                description: "API work".to_string()
            }
        );

        // A `.devin/rules/` copy of the same rule wins.
        std::fs::create_dir_all(root.join(".devin/rules")).unwrap();
        std::fs::write(
            root.join(".devin/rules/api.md"),
            "---\ntrigger: manual\n---\nNew API rule.\n",
        )
        .unwrap();
        let config = DevinAdapter.read(root).unwrap();
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].content, "New API rule.");
    }

    #[test]
    fn test_read_falls_back_to_windsurfrules() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".windsurfrules"), "Old rules.\n").unwrap();
        assert_eq!(
            DevinAdapter.read(tmp.path()).unwrap().instructions,
            "Old rules."
        );
    }

    #[test]
    fn test_read_globs_keeps_brace_groups() {
        let mut fields = BTreeMap::new();
        fields.insert("trigger".to_string(), "glob".into());
        fields.insert("globs".to_string(), "src/*.{ts,tsx}, docs/**".into());
        assert_eq!(
            parse_devin_activation(&fields),
            ActivationMode::GlobMatch(vec!["src/*.{ts,tsx}".into(), "docs/**".into()])
        );
    }

    #[test]
    fn test_glob_with_braces_is_expanded_on_write() {
        let config = NormalizedConfig {
            rules: vec![NormalizedRule {
                name: "TS".to_string(),
                content: "x".to_string(),
                activation: ActivationMode::GlobMatch(vec!["src/*.{ts,tsx}".to_string()]),
            }],
            ..Default::default()
        };
        let files = DevinAdapter.generate(Path::new("/r"), &config).unwrap();
        assert!(
            files[0].1.contains("globs: src/*.ts, src/*.tsx"),
            "{}",
            files[0].1
        );
    }

    #[test]
    fn test_manual_skill_uses_triggers_user() {
        let config = NormalizedConfig {
            skills: vec![NormalizedSkill {
                manual_invocation: true,
                ..skill("deploy")
            }],
            ..Default::default()
        };
        let tmp = tempfile::tempdir().unwrap();
        DevinAdapter.write(tmp.path(), &config).unwrap();
        let written =
            std::fs::read_to_string(tmp.path().join(".devin/skills/deploy/SKILL.md")).unwrap();
        assert!(written.contains("triggers:\n- user\n"), "{written}");
        assert!(!written.contains("disable-model-invocation"), "{written}");
        let back = DevinAdapter.read(tmp.path()).unwrap();
        assert!(back.skills[0].manual_invocation);
        // Skills are not restricted by `allowed-tools` in Devin, and the
        // field only takes Devin tool names, so it is not copied.
        assert!(!written.contains("allowed-tools"));
    }

    #[test]
    fn test_mcp_written_to_devin_mcp_config() {
        let config = NormalizedConfig {
            mcp_servers: vec![
                NormalizedMcpServer {
                    name: "fs".to_string(),
                    transport: McpTransport::Stdio {
                        command: "npx".to_string(),
                        args: vec!["-y".to_string(), "@mcp/fs".to_string()],
                    },
                    env: BTreeMap::from([("TOKEN".to_string(), "${GH_TOKEN}".to_string())]),
                },
                NormalizedMcpServer {
                    name: "api".to_string(),
                    transport: McpTransport::Http {
                        url: "https://example.com/mcp".to_string(),
                        headers: BTreeMap::new(),
                    },
                    env: BTreeMap::from([("X".to_string(), "y".to_string())]),
                },
            ],
            ..Default::default()
        };
        let tmp = tempfile::tempdir().unwrap();
        let files = DevinAdapter.generate(tmp.path(), &config).unwrap();
        let (path, json) = files.last().unwrap();
        assert!(path.ends_with(".devin/mcp_config.json"));
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        let fs = &value["mcpServers"]["fs"];
        assert!(fs.get("type").is_none());
        assert_eq!(fs["env"]["TOKEN"], "${env:GH_TOKEN}");
        let api = &value["mcpServers"]["api"];
        assert_eq!(api["url"], "https://example.com/mcp");
        assert_eq!(api["transport"], "http");
        assert!(api.get("env").is_none());
        assert!(DevinAdapter.is_shared_file(path));

        DevinAdapter.write(tmp.path(), &config).unwrap();
        let back = DevinAdapter.read(tmp.path()).unwrap();
        assert_eq!(back.mcp_servers.len(), 2);
        let fs = back.mcp_servers.iter().find(|s| s.name == "fs").unwrap();
        assert_eq!(fs.env["TOKEN"], "${GH_TOKEN}");
    }

    #[test]
    fn test_managed_dirs_cover_legacy_layout() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".windsurf/rules")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".windsurf/skills")).unwrap();
        let dirs = DevinAdapter.managed_directories(tmp.path());
        let paths: Vec<_> = dirs.iter().map(|d| d.path.clone()).collect();
        assert!(paths.contains(&tmp.path().join(".devin/rules")));
        assert!(paths.contains(&tmp.path().join(".devin/skills")));
        assert!(paths.contains(&tmp.path().join(".windsurf/rules")));
        let legacy_skills = dirs
            .iter()
            .find(|d| d.path == tmp.path().join(".windsurf/skills"))
            .unwrap();
        assert_eq!(
            legacy_skills.superseded_by,
            Some(tmp.path().join(".devin/skills"))
        );
    }

    #[test]
    fn test_clean_removes_legacy_skill_duplicates_only() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for name in ["deploy", "bundled", "hand-made"] {
            let dir = root.join(".windsurf/skills").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("SKILL.md"), "---\nname: x\n---\nOld.\n").unwrap();
        }
        std::fs::write(root.join(".windsurf/skills/bundled/run.sh"), "echo").unwrap();
        std::fs::create_dir_all(root.join(".windsurf/rules")).unwrap();
        std::fs::write(root.join(".windsurf/rules/general.md"), "Old.\n").unwrap();
        std::fs::write(root.join(".windsurf/rules/mine.md"), "Mine.\n").unwrap();

        let config = NormalizedConfig {
            instructions: "Be helpful.".to_string(),
            skills: vec![skill("deploy"), skill("bundled")],
            ..Default::default()
        };
        let generated = DevinAdapter.generate(root, &config).unwrap();
        let cleaned =
            crate::adapters::clean_orphans(&DevinAdapter.managed_directories(root), &generated)
                .unwrap();

        // Only conforme's old copies go: the legacy general.md and the plain
        // deploy skill, now generated in `.devin/`.
        assert_eq!(
            cleaned,
            vec![
                root.join(".windsurf/rules/general.md"),
                root.join(".windsurf/skills/deploy/SKILL.md"),
            ]
        );
        assert!(!root.join(".windsurf/skills/deploy").exists());
        // A rule the source does not generate, a skill folder bundling a
        // script, and a skill the source does not have are the user's.
        assert!(root.join(".windsurf/rules/mine.md").exists());
        assert!(root.join(".windsurf/skills/bundled/SKILL.md").exists());
        assert!(root.join(".windsurf/skills/bundled/run.sh").exists());
        assert!(root.join(".windsurf/skills/hand-made/SKILL.md").exists());
    }

    #[test]
    fn test_generate_empty_config() {
        let files = DevinAdapter
            .generate(Path::new("/tmp/test"), &NormalizedConfig::default())
            .unwrap();
        assert!(files.is_empty());
    }
}
