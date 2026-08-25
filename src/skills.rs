use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::config::{sanitize_name, NormalizedAgent, NormalizedSkill};
use crate::frontmatter;

/// Parse a frontmatter tool list that may be a scalar string (space- or
/// comma-separated) or a YAML sequence. Shared by the per-tool skill/agent readers.
pub(crate) fn parse_frontmatter_tool_list(value: Option<&serde_yaml_ng::Value>) -> Vec<String> {
    match value {
        Some(serde_yaml_ng::Value::String(s)) => s
            .split_whitespace()
            .flat_map(|t| t.split(','))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        Some(serde_yaml_ng::Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// Read `<dir>/<name>/SKILL.md` skill files into `NormalizedSkill`s.
/// Mirrors the SKILL.md folder layout every per-rule adapter writes, so `read()`
/// can round-trip the skills that `generate()` produced.
pub(crate) fn read_skills_from_dir(skills_dir: &Path) -> Result<Vec<NormalizedSkill>> {
    let mut skills = Vec::new();
    if !skills_dir.is_dir() {
        return Ok(skills);
    }
    let mut entries: Vec<_> = std::fs::read_dir(skills_dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let skill_dir = entry.path();
        if !skill_dir.is_dir() {
            continue;
        }
        let skill_file = skill_dir.join("SKILL.md");
        if !skill_file.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&skill_file)?;
        let (fields, body) = frontmatter::parse(&content)?;
        let name = fields
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                skill_dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });
        let description = fields
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let allowed_tools = parse_frontmatter_tool_list(fields.get("allowed-tools"));
        skills.push(NormalizedSkill {
            name,
            description,
            content: body.trim().to_string(),
            allowed_tools,
        });
    }
    Ok(skills)
}

/// Read `<dir>/*.md` agent files into `NormalizedAgent`s (the common frontmatter
/// subset: `name`, `description`, `model`, `tools`). Used by adapters whose
/// subagents live one-per-markdown-file.
pub(crate) fn read_agents_from_dir(agents_dir: &Path) -> Result<Vec<NormalizedAgent>> {
    let mut agents = Vec::new();
    if !agents_dir.is_dir() {
        return Ok(agents);
    }
    let mut entries: Vec<_> = std::fs::read_dir(agents_dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            let content = std::fs::read_to_string(&path)?;
            let (fields, body) = frontmatter::parse(&content)?;
            let name = fields
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                });
            let description = fields
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let model = fields
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let tools = parse_frontmatter_tool_list(fields.get("tools"));
            agents.push(NormalizedAgent {
                name,
                description,
                content: body.trim().to_string(),
                model,
                tools,
                ..Default::default()
            });
        }
    }
    Ok(agents)
}

/// Generate Claude Code skill files in `.claude/skills/<name>/SKILL.md`.
pub fn generate_claude_skills(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let skills_dir = project_root.join(".claude").join("skills");
    let mut files = Vec::new();

    for skill in skills {
        let skill_name = sanitize_name(&skill.name);
        let skill_dir = skills_dir.join(&skill_name);
        let skill_path = skill_dir.join("SKILL.md");

        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), serde_yaml_ng::Value::String(skill_name));
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }
        if !skill.allowed_tools.is_empty() {
            fields.insert(
                "allowed-tools".to_string(),
                serde_yaml_ng::Value::String(skill.allowed_tools.join(" ")),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((skill_path, content));
    }

    Ok(files)
}

/// Generate Cursor skill files in `.cursor/skills/<name>/SKILL.md`.
/// Cursor skills use `name` and `description` in frontmatter.
pub fn generate_cursor_skills(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let skills_dir = project_root.join(".cursor").join("skills");
    let mut files = Vec::new();

    for skill in skills {
        let skill_name = sanitize_name(&skill.name);
        let skill_dir = skills_dir.join(&skill_name);
        let skill_path = skill_dir.join("SKILL.md");

        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), serde_yaml_ng::Value::String(skill_name));
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((skill_path, content));
    }

    Ok(files)
}

/// Generate Codex/OpenCode skill files in `.agents/skills/<name>/SKILL.md`.
pub fn generate_codex_skills(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let skills_dir = project_root.join(".agents").join("skills");
    let mut files = Vec::new();

    for skill in skills {
        let skill_name = sanitize_name(&skill.name);
        let skill_dir = skills_dir.join(&skill_name);
        let skill_path = skill_dir.join("SKILL.md");

        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), serde_yaml_ng::Value::String(skill_name));
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((skill_path, content));
    }

    Ok(files)
}

/// Generate GitHub Copilot skill files in `.github/skills/<name>/SKILL.md`.
///
/// Copilot documents skills at `.github/skills/<skill-name>/SKILL.md` (the same
/// location for Copilot CLI and the cloud agent), with `name` and `description`
/// required and `allowed-tools` available to pre-approve tools. This supersedes
/// the older `.github/prompts/<name>.prompt.md` layout conforme used to emit —
/// prompt files are a separate VS Code feature, not Copilot's skills format.
pub fn generate_copilot_skills(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let skills_dir = project_root.join(".github").join("skills");
    let mut files = Vec::new();

    for skill in skills {
        let skill_name = sanitize_name(&skill.name);
        let skill_path = skills_dir.join(&skill_name).join("SKILL.md");

        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), serde_yaml_ng::Value::String(skill_name));
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }
        if !skill.allowed_tools.is_empty() {
            fields.insert(
                "allowed-tools".to_string(),
                serde_yaml_ng::Value::String(skill.allowed_tools.join(" ")),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((skill_path, content));
    }

    Ok(files)
}

/// Generate Copilot agent files in `.github/agents/<name>.agent.md`.
pub fn generate_copilot_agents(
    project_root: &Path,
    agents: &[crate::config::NormalizedAgent],
) -> Result<Vec<(PathBuf, String)>> {
    let agents_dir = project_root.join(".github").join("agents");
    let mut files = Vec::new();

    for agent in agents {
        let filename = format!("{}.agent.md", sanitize_name(&agent.name));
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".to_string(),
            serde_yaml_ng::Value::String(agent.name.clone()),
        );
        if !agent.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(agent.description.clone()),
            );
        }
        if let Some(model) = &agent.model {
            fields.insert(
                "model".to_string(),
                serde_yaml_ng::Value::String(model.clone()),
            );
        }
        if !agent.tools.is_empty() {
            let yaml_tools: Vec<serde_yaml_ng::Value> = agent
                .tools
                .iter()
                .map(|t| serde_yaml_ng::Value::String(t.clone()))
                .collect();
            fields.insert(
                "tools".to_string(),
                serde_yaml_ng::Value::Sequence(yaml_tools),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", agent.content))?;
        files.push((agents_dir.join(filename), content));
    }

    Ok(files)
}

/// Generate Claude Code subagent files in `.claude/agents/<name>.md`.
pub fn generate_claude_agents(
    project_root: &Path,
    agents: &[crate::config::NormalizedAgent],
) -> Result<Vec<(PathBuf, String)>> {
    let agents_dir = project_root.join(".claude").join("agents");
    let mut files = Vec::new();

    for agent in agents {
        let filename = format!("{}.md", sanitize_name(&agent.name));
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".to_string(),
            serde_yaml_ng::Value::String(sanitize_name(&agent.name)),
        );
        fields.insert(
            "description".to_string(),
            serde_yaml_ng::Value::String(agent.description.clone()),
        );
        if let Some(model) = &agent.model {
            fields.insert(
                "model".to_string(),
                serde_yaml_ng::Value::String(model.clone()),
            );
        }
        if !agent.tools.is_empty() {
            fields.insert(
                "tools".to_string(),
                serde_yaml_ng::Value::String(agent.tools.join(", ")),
            );
        }
        if let Some(color) = &agent.color {
            fields.insert(
                "color".to_string(),
                serde_yaml_ng::Value::String(color.clone()),
            );
        }
        if let Some(permission_mode) = &agent.permission_mode {
            fields.insert(
                "permissionMode".to_string(),
                serde_yaml_ng::Value::String(permission_mode.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", agent.content))?;
        files.push((agents_dir.join(filename), content));
    }

    Ok(files)
}

/// Generate Cursor agent files in `.cursor/agents/<name>.md`.
/// Cursor subagents use `.md` (not `.mdc`) and support only `name`, `description`,
/// `model`, `readonly`, `is_background`. The `tools` field is not recognized;
/// tool access is inherited from the parent agent.
pub fn generate_cursor_agents(
    project_root: &Path,
    agents: &[NormalizedAgent],
) -> Result<Vec<(PathBuf, String)>> {
    let agents_dir = project_root.join(".cursor").join("agents");
    let mut files = Vec::new();

    for agent in agents {
        let filename = format!("{}.md", sanitize_name(&agent.name));
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".to_string(),
            serde_yaml_ng::Value::String(agent.name.clone()),
        );
        if !agent.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(agent.description.clone()),
            );
        }
        if let Some(model) = &agent.model {
            fields.insert(
                "model".to_string(),
                serde_yaml_ng::Value::String(model.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", agent.content))?;
        files.push((agents_dir.join(filename), content));
    }

    Ok(files)
}

/// Generate Kiro agent files in `.kiro/agents/<name>.md`.
pub fn generate_kiro_agents(
    project_root: &Path,
    agents: &[NormalizedAgent],
) -> Result<Vec<(PathBuf, String)>> {
    let agents_dir = project_root.join(".kiro").join("agents");
    let mut files = Vec::new();

    for agent in agents {
        let filename = format!("{}.md", sanitize_name(&agent.name));
        // Kiro derives the agent name from the file path — the custom-agent
        // frontmatter has no `name` field, so we do not emit one.
        let mut fields = BTreeMap::new();
        if !agent.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(agent.description.clone()),
            );
        }
        if let Some(model) = &agent.model {
            fields.insert(
                "model".to_string(),
                serde_yaml_ng::Value::String(model.clone()),
            );
        }
        if !agent.tools.is_empty() {
            let yaml_tools: Vec<serde_yaml_ng::Value> = agent
                .tools
                .iter()
                .map(|t| serde_yaml_ng::Value::String(t.clone()))
                .collect();
            fields.insert(
                "tools".to_string(),
                serde_yaml_ng::Value::Sequence(yaml_tools),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", agent.content))?;
        files.push((agents_dir.join(filename), content));
    }

    Ok(files)
}

/// Generate Gemini CLI subagent files in `.gemini/agents/<name>.md`.
/// Gemini format: name, description, kind (local), tools, model in YAML frontmatter.
pub fn generate_gemini_agents(
    project_root: &Path,
    agents: &[NormalizedAgent],
) -> Result<Vec<(PathBuf, String)>> {
    let agents_dir = project_root.join(".gemini").join("agents");
    let mut files = Vec::new();

    for agent in agents {
        let filename = format!("{}.md", sanitize_name(&agent.name));
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".to_string(),
            serde_yaml_ng::Value::String(sanitize_name(&agent.name)),
        );
        if !agent.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(agent.description.clone()),
            );
        }
        fields.insert(
            "kind".to_string(),
            serde_yaml_ng::Value::String("local".to_string()),
        );
        if !agent.tools.is_empty() {
            let yaml_tools: Vec<serde_yaml_ng::Value> = agent
                .tools
                .iter()
                .map(|t| serde_yaml_ng::Value::String(t.clone()))
                .collect();
            fields.insert(
                "tools".to_string(),
                serde_yaml_ng::Value::Sequence(yaml_tools),
            );
        }
        if let Some(model) = &agent.model {
            fields.insert(
                "model".to_string(),
                serde_yaml_ng::Value::String(model.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", agent.content))?;
        files.push((agents_dir.join(filename), content));
    }

    Ok(files)
}

/// Generate Kiro skill files in `.kiro/skills/<name>/SKILL.md`.
pub fn generate_kiro_skills(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let skills_dir = project_root.join(".kiro").join("skills");
    let mut files = Vec::new();

    for skill in skills {
        let skill_name = sanitize_name(&skill.name);
        let skill_dir = skills_dir.join(&skill_name);
        let skill_path = skill_dir.join("SKILL.md");

        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), serde_yaml_ng::Value::String(skill_name));
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((skill_path, content));
    }

    Ok(files)
}

/// Generate Windsurf skill files in `.windsurf/skills/<name>/SKILL.md`.
/// Windsurf skills use only `name` and `description` in frontmatter.
pub fn generate_windsurf_skills(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let skills_dir = project_root.join(".windsurf").join("skills");
    let mut files = Vec::new();

    for skill in skills {
        let skill_name = sanitize_name(&skill.name);
        let skill_dir = skills_dir.join(&skill_name);
        let skill_path = skill_dir.join("SKILL.md");

        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), serde_yaml_ng::Value::String(skill_name));
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((skill_path, content));
    }

    Ok(files)
}

/// Generate Roo Code skill files in `.roo/skills/<name>/SKILL.md`.
/// Roo Code skills use `name` and `description` in frontmatter.
pub fn generate_roocode_skills(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let skills_dir = project_root.join(".roo").join("skills");
    let mut files = Vec::new();

    for skill in skills {
        let skill_name = sanitize_name(&skill.name);
        let skill_dir = skills_dir.join(&skill_name);
        let skill_path = skill_dir.join("SKILL.md");

        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), serde_yaml_ng::Value::String(skill_name));
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((skill_path, content));
    }

    Ok(files)
}

/// Generate OpenCode skill files in `.opencode/skills/<name>/SKILL.md`.
/// OpenCode skills recognize only `name`, `description`, `license`, `compatibility`,
/// and `metadata`. `allowed-tools` is not a recognized field.
pub fn generate_opencode_skills(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let skills_dir = project_root.join(".opencode").join("skills");
    let mut files = Vec::new();

    for skill in skills {
        let skill_name = sanitize_name(&skill.name);
        let skill_dir = skills_dir.join(&skill_name);
        let skill_path = skill_dir.join("SKILL.md");

        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), serde_yaml_ng::Value::String(skill_name));
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((skill_path, content));
    }

    Ok(files)
}

/// Generate OpenCode subagent markdown files in `.opencode/agents/<name>.md`.
/// OpenCode reads per-project agent markdown files from `.opencode/agents/`.
pub fn generate_opencode_agents_md(
    project_root: &Path,
    agents: &[NormalizedAgent],
) -> Result<Vec<(PathBuf, String)>> {
    let agents_dir = project_root.join(".opencode").join("agents");
    let mut files = Vec::new();

    for agent in agents {
        let filename = format!("{}.md", sanitize_name(&agent.name));
        let mut fields = BTreeMap::new();
        if !agent.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(agent.description.clone()),
            );
        }
        fields.insert(
            "mode".to_string(),
            serde_yaml_ng::Value::String("subagent".to_string()),
        );
        if let Some(model) = &agent.model {
            fields.insert(
                "model".to_string(),
                serde_yaml_ng::Value::String(model.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", agent.content))?;
        files.push((agents_dir.join(filename), content));
    }

    Ok(files)
}

/// Generate DeepSeek Harness skill files in `.dsh/skills/<name>/SKILL.md`.
/// The harness filesystem skill provider scans `<projectRoot>/.dsh/skills` first
/// and interprets required `name` and `description`, plus optional `whenToUse`,
/// `metadata`, `disable-model-invocation` and `user-invocable`. `allowed-tools`
/// is not a recognized field, and names must be kebab-case.
pub fn generate_deepseek_skills(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let skills_dir = project_root.join(".dsh").join("skills");
    let mut files = Vec::new();

    for skill in skills {
        let skill_name = sanitize_name(&skill.name);
        let skill_dir = skills_dir.join(&skill_name);
        let skill_path = skill_dir.join("SKILL.md");

        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), serde_yaml_ng::Value::String(skill_name));
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((skill_path, content));
    }

    Ok(files)
}

/// Generate Gemini CLI skill files in `.gemini/skills/<name>/SKILL.md`.
/// Gemini skills use ONLY `name` and `description` (no other fields allowed).
pub fn generate_gemini_skills(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let skills_dir = project_root.join(".gemini").join("skills");
    let mut files = Vec::new();

    for skill in skills {
        let skill_name = sanitize_name(&skill.name);
        let skill_dir = skills_dir.join(&skill_name);
        let skill_path = skill_dir.join("SKILL.md");

        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), serde_yaml_ng::Value::String(skill_name));
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }
        // Gemini docs: "do not include any other fields" — no allowed-tools

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((skill_path, content));
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_claude_skill() {
        let skills = vec![NormalizedSkill {
            name: "deploy".to_string(),
            description: "Deploy the app".to_string(),
            content: "Run npm run deploy".to_string(),
            allowed_tools: vec!["Bash".to_string()],
        }];
        let files = generate_claude_skills(Path::new("/tmp/test"), &skills).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].0.to_string_lossy().ends_with("SKILL.md"));
        assert!(files[0].1.contains("name: deploy"));
        assert!(files[0].1.contains("description: Deploy the app"));
        assert!(files[0].1.contains("allowed-tools: Bash"));
    }

    #[test]
    fn test_generate_copilot_skill() {
        // Copilot documents skills at `.github/skills/<name>/SKILL.md`.
        let skills = vec![NormalizedSkill {
            name: "Deploy App".to_string(),
            description: "Deploy the app".to_string(),
            content: "Run npm run deploy".to_string(),
            allowed_tools: vec!["shell".to_string(), "bash".to_string()],
        }];
        let files = generate_copilot_skills(Path::new("/tmp/test"), &skills).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0]
            .0
            .to_string_lossy()
            .ends_with(".github/skills/deploy-app/SKILL.md"));
        assert!(files[0].1.contains("name: deploy-app"));
        assert!(files[0].1.contains("description: Deploy the app"));
        assert!(files[0].1.contains("allowed-tools: shell bash"));
        assert!(files[0].1.contains("Run npm run deploy"));
    }

    #[test]
    fn test_generate_copilot_agent() {
        let agents = vec![crate::config::NormalizedAgent {
            name: "reviewer".to_string(),
            description: "Code reviewer".to_string(),
            content: "Review for bugs.".to_string(),
            model: Some("gpt-4o".to_string()),
            tools: vec!["codebase".to_string()],
            ..Default::default()
        }];
        let files = generate_copilot_agents(Path::new("/tmp/test"), &agents).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].0.to_string_lossy().ends_with(".agent.md"));
        assert!(files[0].1.contains("name: reviewer"));
        assert!(files[0].1.contains("model: gpt-4o"));
    }
}
