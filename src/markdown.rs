use anyhow::{bail, Result};

use crate::config::{ActivationMode, NormalizedConfig, NormalizedRule};

/// Parse an AGENTS.md file into a NormalizedConfig.
///
/// Convention:
/// - Text before the first `## Rule:` heading becomes `instructions`
/// - Each `## Rule: <name>` section becomes a NormalizedRule
/// - HTML comments encode activation: `<!-- activation: always|glob|agent-decision|manual -->`
/// - For glob: `<!-- activation: glob **/*.ts,**/*.tsx -->`
/// - For agent-decision: `<!-- description: ... -->` provides the description
pub fn parse_agents_md(content: &str) -> Result<NormalizedConfig> {
    let mut instructions = String::new();
    let mut rules = Vec::new();
    let mut current_rule: Option<(String, Vec<String>)> = None;

    for line in content.lines() {
        if let Some(name) = line.strip_prefix("## Rule: ") {
            // Flush previous rule
            if let Some((rule_name, rule_lines)) = current_rule.take() {
                rules.push(build_rule(&rule_name, &rule_lines)?);
            }
            current_rule = Some((name.trim().to_string(), Vec::new()));
        } else if let Some((_name, lines)) = current_rule.as_mut() {
            lines.push(line.to_string());
        } else {
            instructions.push_str(line);
            instructions.push('\n');
        }
    }

    // Flush last rule
    if let Some((rule_name, rule_lines)) = current_rule.take() {
        rules.push(build_rule(&rule_name, &rule_lines)?);
    }

    let instructions = instructions.trim().to_string();

    Ok(NormalizedConfig {
        instructions,
        rules,
    })
}

fn build_rule(name: &str, lines: &[String]) -> Result<NormalizedRule> {
    let mut activation = ActivationMode::Always;
    let mut description: Option<String> = None;
    let mut content_lines = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if let Some(inner) = trimmed
            .strip_prefix("<!-- activation:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            let inner = inner.trim();
            activation = parse_activation(inner)?;
        } else if let Some(inner) = trimmed
            .strip_prefix("<!-- description:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            description = Some(inner.trim().to_string());
        } else {
            content_lines.push(line.as_str());
        }
    }

    // If we have a description and activation is AgentDecision with empty description, fill it
    if let Some(desc) = description {
        if matches!(activation, ActivationMode::AgentDecision { .. }) {
            activation = ActivationMode::AgentDecision { description: desc };
        }
    }

    let content = content_lines.join("\n").trim().to_string();

    if name.is_empty() {
        bail!("empty rule name");
    }

    Ok(NormalizedRule {
        name: name.to_string(),
        content,
        activation,
    })
}

fn parse_activation(s: &str) -> Result<ActivationMode> {
    if s == "always" {
        Ok(ActivationMode::Always)
    } else if s == "manual" {
        Ok(ActivationMode::Manual)
    } else if s == "agent-decision" {
        Ok(ActivationMode::AgentDecision {
            description: String::new(),
        })
    } else if let Some(globs) = s.strip_prefix("glob ") {
        let patterns: Vec<String> = globs.split(',').map(|g| g.trim().to_string()).collect();
        if patterns.is_empty() {
            bail!("glob activation requires at least one pattern");
        }
        Ok(ActivationMode::GlobMatch(patterns))
    } else {
        bail!("unknown activation mode: {}", s)
    }
}

/// Generate a template AGENTS.md file.
pub fn template_agents_md() -> String {
    r#"# Project Instructions

<!-- Add your project-wide instructions here. -->
<!-- conforme will sync this file to all detected AI coding tools. -->

## Rule: General Conventions
<!-- activation: always -->

<!-- Add conventions that should always apply. -->
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let config = parse_agents_md("").unwrap();
        assert!(config.instructions.is_empty());
        assert!(config.rules.is_empty());
    }

    #[test]
    fn test_parse_instructions_only() {
        let content = "# My Project\n\nUse TypeScript everywhere.\n";
        let config = parse_agents_md(content).unwrap();
        assert!(config.instructions.contains("Use TypeScript"));
        assert!(config.rules.is_empty());
    }

    #[test]
    fn test_parse_with_rules() {
        let content = r#"# Instructions
General stuff.

## Rule: TypeScript
<!-- activation: glob **/*.ts,**/*.tsx -->

Use strict mode.

## Rule: Security
<!-- activation: agent-decision -->
<!-- description: Apply for security reviews -->

Check for XSS.
"#;
        let config = parse_agents_md(content).unwrap();
        assert!(config.instructions.contains("General stuff"));
        assert_eq!(config.rules.len(), 2);

        assert_eq!(config.rules[0].name, "TypeScript");
        assert!(matches!(
            &config.rules[0].activation,
            ActivationMode::GlobMatch(g) if g.len() == 2
        ));

        assert_eq!(config.rules[1].name, "Security");
        assert!(matches!(
            &config.rules[1].activation,
            ActivationMode::AgentDecision { description } if description == "Apply for security reviews"
        ));
    }

    #[test]
    fn test_parse_always_activation() {
        let content = "## Rule: Always On\n<!-- activation: always -->\n\nContent.\n";
        let config = parse_agents_md(content).unwrap();
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].activation, ActivationMode::Always);
    }

    #[test]
    fn test_parse_manual_activation() {
        let content = "## Rule: Manual Only\n<!-- activation: manual -->\n\nContent.\n";
        let config = parse_agents_md(content).unwrap();
        assert_eq!(config.rules[0].activation, ActivationMode::Manual);
    }

    #[test]
    fn test_parse_default_activation() {
        let content = "## Rule: No Activation\n\nContent without activation comment.\n";
        let config = parse_agents_md(content).unwrap();
        assert_eq!(config.rules[0].activation, ActivationMode::Always);
    }
}
