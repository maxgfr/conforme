use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::config::{sanitize_name, NormalizedAgent, NormalizedSkill};
use crate::frontmatter;

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

/// Generate Copilot prompt files in `.github/prompts/<name>.prompt.md`.
pub fn generate_copilot_prompts(
    project_root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<(PathBuf, String)>> {
    let prompts_dir = project_root.join(".github").join("prompts");
    let mut files = Vec::new();

    for skill in skills {
        let filename = format!("{}.prompt.md", sanitize_name(&skill.name));
        // Copilot prompts use simple frontmatter with description
        let mut fields = BTreeMap::new();
        if !skill.description.is_empty() {
            fields.insert(
                "description".to_string(),
                serde_yaml_ng::Value::String(skill.description.clone()),
            );
        }

        let content = frontmatter::serialize(&fields, &format!("{}\n", skill.content))?;
        files.push((prompts_dir.join(filename), content));
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

        let content = frontmatter::serialize(&fields, &format!("{}\n", agent.content))?;
        files.push((agents_dir.join(filename), content));
    }

    Ok(files)
}

/// Generate Cursor agent files in `.cursor/agents/<name>.mdc`.
pub fn generate_cursor_agents(
    project_root: &Path,
    agents: &[NormalizedAgent],
) -> Result<Vec<(PathBuf, String)>> {
    let agents_dir = project_root.join(".cursor").join("agents");
    let mut files = Vec::new();

    for agent in agents {
        let filename = format!("{}.mdc", sanitize_name(&agent.name));
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

/// Generate Kiro agent files in `.kiro/agents/<name>.md`.
pub fn generate_kiro_agents(
    project_root: &Path,
    agents: &[NormalizedAgent],
) -> Result<Vec<(PathBuf, String)>> {
    let agents_dir = project_root.join(".kiro").join("agents");
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
    fn test_generate_copilot_agent() {
        let agents = vec![crate::config::NormalizedAgent {
            name: "reviewer".to_string(),
            description: "Code reviewer".to_string(),
            content: "Review for bugs.".to_string(),
            model: Some("gpt-4o".to_string()),
            tools: vec!["codebase".to_string()],
        }];
        let files = generate_copilot_agents(Path::new("/tmp/test"), &agents).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].0.to_string_lossy().ends_with(".agent.md"));
        assert!(files[0].1.contains("name: reviewer"));
        assert!(files[0].1.contains("model: gpt-4o"));
    }
}
