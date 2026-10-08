use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::adapters::{AiToolAdapter, ManagedDir};
use crate::config::{rule_file_name, ActivationMode, NormalizedConfig, NormalizedRule};

/// The comment `generate` puts above a glob rule, since Zoo Code has no
/// activation modes.
const SCOPE_COMMENT: &str = "<!-- Intended scope: ";

/// `01-security` → `security`: the numeric prefix only orders the files.
/// Only the two- or three-digit prefixes conforme writes (`{:02}`) are
/// stripped: `2024-plan` is a name, not an order.
fn strip_order_prefix(stem: &str) -> &str {
    match stem.split_once('-') {
        Some((digits, rest))
            if (2..=3).contains(&digits.len())
                && digits.bytes().all(|b| b.is_ascii_digit())
                && !rest.is_empty() =>
        {
            rest
        }
        _ => stem,
    }
}

/// Read back the scope comment `generate` writes above a glob rule, so the
/// rule keeps its globs when Zoo Code is the source.
fn parse_scope_comment(content: &str) -> (ActivationMode, String) {
    let scoped = content.strip_prefix(SCOPE_COMMENT).and_then(|rest| {
        let (line, body) = rest.split_once('\n').unwrap_or((rest, ""));
        let globs = crate::config::split_globs(line.trim_end().strip_suffix("-->")?);
        (!globs.is_empty()).then(|| (ActivationMode::GlobMatch(globs), body.trim().to_string()))
    });
    scoped.unwrap_or_else(|| (ActivationMode::Always, content.to_string()))
}

/// Zoo Code (community fork of Roo Code) adapter.
/// Rules in .roo/rules/*.md — plain Markdown, NO YAML frontmatter.
/// Files loaded in alphabetical order. Mode-specific rules in .roo/rules-{mode}/.
pub struct ZooCodeAdapter;

impl AiToolAdapter for ZooCodeAdapter {
    fn name(&self) -> &str {
        "Zoo Code"
    }

    fn id(&self) -> &str {
        "zoocode"
    }

    // `.clinerules` is Cline's own file: Zoo Code reads it only as a legacy
    // fallback, so it does not mean Zoo Code is set up.
    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".roo").is_dir() || project_root.join(".roorules").exists()
    }

    fn capabilities(&self) -> crate::adapters::AdapterCapabilities {
        crate::adapters::AdapterCapabilities {
            activation_modes: false,
            skills: true,
            agents: false,
            mcp: true,
        }
    }

    /// Zoo Code writes its own per-server state (`alwaysAllow`,
    /// `disabledTools`) into `.roo/mcp.json`, so `remove`/`migrate` must
    /// never delete the file wholesale.
    fn is_shared_file(&self, path: &Path) -> bool {
        path.ends_with(Path::new(".roo/mcp.json"))
    }

    fn source_files(&self, project_root: &Path) -> Vec<PathBuf> {
        // `.roorules` is only read when `.roo/rules/` holds no rule.
        let rules_dir = project_root.join(".roo").join("rules");
        if crate::adapters::collect_rule_files(&rules_dir, "md").is_ok_and(|f| f.is_empty()) {
            crate::adapters::first_existing_file(project_root, &[".roorules"])
        } else {
            Vec::new()
        }
    }

    fn managed_directories(&self, project_root: &Path) -> Vec<ManagedDir> {
        vec![
            ManagedDir::files(project_root.join(".roo").join("rules"), ".md"),
            ManagedDir::subdirs(project_root.join(".roo").join("skills")),
        ]
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        let mut instructions = String::new();
        let mut rules = Vec::new();

        let rules_dir = project_root.join(".roo").join("rules");
        // Zoo Code reads `.roo/rules/` recursively, sorting by base name only.
        let rule_files = crate::adapters::collect_rule_files(&rules_dir, "md")?;
        for path in &rule_files {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            // `NN-` only orders the files; it is not part of the rule's name.
            let name = strip_order_prefix(&stem).to_string();
            let (activation, body) = parse_scope_comment(content.trim());

            if name == "general" && activation == ActivationMode::Always {
                instructions = body;
            } else {
                rules.push(NormalizedRule {
                    name,
                    content: body,
                    activation,
                });
            }
        }
        // Zoo Code falls back to `.roorules` when `.roo/rules/` is missing or
        // empty.
        let roorules = project_root.join(".roorules");
        if rule_files.is_empty() && roorules.is_file() {
            instructions = std::fs::read_to_string(&roorules)
                .with_context(|| format!("failed to read {}", roorules.display()))?
                .trim()
                .to_string();
        }

        // Read skills and MCP back so a Zoo Code project round-trips as a source.
        let skills =
            crate::skills::read_skills_from_dir(&project_root.join(".roo").join("skills"))?;
        let mut mcp_servers = Vec::new();
        let mcp_path = project_root.join(".roo").join("mcp.json");
        if mcp_path.exists() {
            let mcp_content = std::fs::read_to_string(&mcp_path)?;
            mcp_servers = crate::mcp::canonicalize_env_refs(
                crate::mcp::parse_mcp_json(&mcp_content)?,
                crate::mcp::EnvRefStyle::EnvColon,
            );
        }

        Ok(NormalizedConfig {
            instructions,
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
        let rules_dir = project_root.join(".roo").join("rules");
        let mut files = Vec::new();
        let mut idx = 0u32;

        // Zoo Code has no frontmatter — files are plain Markdown, loaded alphabetically.
        // Use numeric prefix for ordering: 00-general, 01-rule-name, etc.

        if !config.instructions.is_empty() {
            files.push((
                rules_dir.join("00-general.md"),
                format!("{}\n", config.instructions),
            ));
            idx += 1;
        }

        for rule in &config.rules {
            let filename = format!("{:02}-{}.md", idx, rule_file_name(&rule.name));
            // Zoo Code doesn't support activation modes — all rules are always-on.
            // For glob/agent-decision rules, we include a comment noting the intended scope.
            let mut content = String::new();
            match &rule.activation {
                ActivationMode::GlobMatch(globs) => {
                    content.push_str(&format!("{SCOPE_COMMENT}{} -->\n\n", globs.join(", ")));
                }
                ActivationMode::AgentDecision { description } if !description.is_empty() => {
                    content.push_str(&format!("<!-- {description} -->\n\n"));
                }
                _ => {}
            }
            content.push_str(&rule.content);
            files.push((rules_dir.join(filename), format!("{}\n", content.trim())));
            idx += 1;
        }

        // Generate skills as .roo/skills/<name>/SKILL.md
        if !config.skills.is_empty() {
            files.extend(crate::skills::generate_zoocode_skills(
                project_root,
                &config.skills,
            )?);
        }

        // Merge MCP config into .roo/mcp.json. Zoo Code uses
        // `type: "streamable-http"` for HTTP servers (not bare "http"), and
        // writes its own per-server state (`alwaysAllow`, `disabledTools`)
        // into this file, so existing entries keep the keys conforme does
        // not own.
        files.extend(crate::json_settings::server_settings_file(
            &project_root.join(".roo").join("mcp.json"),
            "mcpServers",
            crate::mcp::build_zoocode_servers_object(&config.mcp_servers),
            crate::mcp::ZOOCODE_OWNED_SERVER_KEYS,
            &[],
        )?);

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpTransport, NormalizedConfig, NormalizedMcpServer};
    use std::path::Path;

    fn make_adapter() -> ZooCodeAdapter {
        ZooCodeAdapter
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
        assert_eq!(files[0].0, Path::new("/tmp/test/.roo/rules/00-general.md"));
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
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].0, Path::new("/tmp/test/.roo/rules/00-general.md"));
        assert_eq!(
            files[1].0,
            Path::new("/tmp/test/.roo/rules/01-typescript.md")
        );
        assert_eq!(files[2].0, Path::new("/tmp/test/.roo/rules/02-security.md"));
        assert!(files[1].1.contains("Use strict mode."));
        assert!(files[2].1.contains("No eval."));
    }

    #[test]
    fn test_generate_glob_rule_has_comment() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![NormalizedRule {
                name: "TypeScript".to_string(),
                content: "Use strict mode.".to_string(),
                activation: ActivationMode::GlobMatch(vec![
                    "**/*.ts".to_string(),
                    "**/*.tsx".to_string(),
                ]),
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0]
            .1
            .contains("<!-- Intended scope: **/*.ts, **/*.tsx -->"));
        assert!(files[0].1.contains("Use strict mode."));
    }

    #[test]
    fn test_generate_with_skills() {
        use crate::config::NormalizedSkill;
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy the app".to_string(),
                content: "Run deploy.".to_string(),
                allowed_tools: vec![],
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0]
            .0
            .to_string_lossy()
            .contains(".roo/skills/deploy/SKILL.md"));
        assert!(files[0].1.contains("name: deploy"));
        assert!(files[0].1.contains("description: Deploy the app"));
    }

    #[test]
    fn test_generate_with_mcp() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: String::new(),
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
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, Path::new("/tmp/test/.roo/mcp.json"));
        assert!(files[0].1.contains("mcpServers"));
        assert!(files[0].1.contains("\"fs\""));
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

    #[test]
    fn test_only_order_prefixes_are_stripped() {
        assert_eq!(strip_order_prefix("01-security"), "security");
        assert_eq!(strip_order_prefix("100-late"), "late");
        assert_eq!(strip_order_prefix("2024-plan"), "2024-plan");
        assert_eq!(strip_order_prefix("1-intro"), "1-intro");
        assert_eq!(strip_order_prefix("general"), "general");
    }
}
