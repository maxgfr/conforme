use owo_colors::OwoColorize;
use std::collections::HashSet;

use crate::config::{ActivationMode, NormalizedConfig};

/// Validate a NormalizedConfig and print warnings. Returns true if valid (no errors).
pub fn validate(config: &NormalizedConfig, verbose: bool) -> bool {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Check for duplicate rule names
    let mut seen_rules = HashSet::new();
    for rule in &config.rules {
        if !seen_rules.insert(&rule.name) {
            errors.push(format!("Duplicate rule name: '{}'", rule.name));
        }
        if rule.content.trim().is_empty() {
            warnings.push(format!("Rule '{}' has empty content", rule.name));
        }
        if rule.name.trim().is_empty() {
            errors.push("Rule with empty name found".to_string());
        }
        // Validate glob patterns
        if let ActivationMode::GlobMatch(globs) = &rule.activation {
            for glob in globs {
                if let Err(e) = globset::Glob::new(glob) {
                    errors.push(format!(
                        "Invalid glob pattern '{}' in rule '{}': {}",
                        glob, rule.name, e
                    ));
                }
            }
        }
    }

    // Check for duplicate skill names
    let mut seen_skills = HashSet::new();
    for skill in &config.skills {
        if !seen_skills.insert(&skill.name) {
            errors.push(format!("Duplicate skill name: '{}'", skill.name));
        }
        if skill.content.trim().is_empty() {
            warnings.push(format!("Skill '{}' has empty content", skill.name));
        }
    }

    // Check for duplicate agent names
    let mut seen_agents = HashSet::new();
    for agent in &config.agents {
        if !seen_agents.insert(&agent.name) {
            errors.push(format!("Duplicate agent name: '{}'", agent.name));
        }
        if agent.content.trim().is_empty() {
            warnings.push(format!("Agent '{}' has empty content", agent.name));
        }
    }

    // Check for duplicate MCP server names
    let mut seen_mcp = HashSet::new();
    for mcp in &config.mcp_servers {
        if !seen_mcp.insert(&mcp.name) {
            errors.push(format!("Duplicate MCP server name: '{}'", mcp.name));
        }
    }

    // Print warnings
    if verbose || !warnings.is_empty() {
        for w in &warnings {
            eprintln!("  {} {}", "warning:".yellow(), w);
        }
    }

    // Print errors
    for e in &errors {
        eprintln!("  {} {}", "error:".red(), e);
    }

    errors.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    #[test]
    fn test_valid_config() {
        let config = NormalizedConfig {
            instructions: "Hello".to_string(),
            rules: vec![NormalizedRule {
                name: "TypeScript".to_string(),
                content: "Use strict.".to_string(),
                activation: ActivationMode::Always,
            }],
            ..Default::default()
        };
        assert!(validate(&config, false));
    }

    #[test]
    fn test_duplicate_rule_names() {
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![
                NormalizedRule {
                    name: "TypeScript".to_string(),
                    content: "A".to_string(),
                    activation: ActivationMode::Always,
                },
                NormalizedRule {
                    name: "TypeScript".to_string(),
                    content: "B".to_string(),
                    activation: ActivationMode::Always,
                },
            ],
            ..Default::default()
        };
        assert!(!validate(&config, false));
    }

    #[test]
    fn test_invalid_glob() {
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![NormalizedRule {
                name: "Bad".to_string(),
                content: "Content".to_string(),
                activation: ActivationMode::GlobMatch(vec!["[invalid".to_string()]),
            }],
            ..Default::default()
        };
        assert!(!validate(&config, false));
    }

    #[test]
    fn test_empty_content_is_warning_not_error() {
        let config = NormalizedConfig {
            instructions: String::new(),
            rules: vec![NormalizedRule {
                name: "Empty".to_string(),
                content: "  ".to_string(),
                activation: ActivationMode::Always,
            }],
            ..Default::default()
        };
        // Empty content is a warning, not an error
        assert!(validate(&config, false));
    }
}
