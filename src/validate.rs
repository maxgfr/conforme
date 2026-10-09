use owo_colors::OwoColorize;
use std::collections::HashSet;

use crate::config::{
    rule_file_name, sanitize_name, ActivationMode, McpTransport, NormalizedConfig,
};

/// Longest skill description Zoo Code and Mistral Vibe accept; a longer one
/// makes them skip the skill (Codex 0.162.0 checks only the name length).
const MAX_DESCRIPTION_LEN: usize = 1024;

/// Skill names Claude Code skips (`claude-ai`, reserved in 2.1.282 only,
/// loads again since 2.1.283).
const CLAUDE_RESERVED_SKILL_NAMES: &[&str] = &["synced", "anthropic-skills"];

/// Every rule, skill and agent name becomes a file or folder name: through
/// [`rule_file_name`] for rules, and [`sanitize_name`] (kebab-case ASCII, the
/// `name` field too) for skills and agents. A name that sanitizes to nothing
/// cannot be written, and two names that sanitize alike would overwrite each
/// other's file.
fn check_file_names<'a>(
    kind: &str,
    names: impl Iterator<Item = &'a String>,
    sanitize: fn(&str) -> String,
    errors: &mut Vec<String>,
) {
    let mut seen: std::collections::HashMap<String, &String> = Default::default();
    for name in names {
        if name.trim().is_empty() {
            continue;
        }
        let sanitized = sanitize(name);
        if sanitized.is_empty() {
            errors.push(format!(
                "{kind} name '{name}' has no ASCII letter or digit; tools need a kebab-case name"
            ));
        } else if let Some(other) = seen.insert(sanitized.clone(), name) {
            if other != name {
                errors.push(format!(
                    "{kind} names '{other}' and '{name}' both become '{sanitized}'"
                ));
            }
        }
    }
}

/// Header and environment-variable names that hold credentials.
const SECRET_NAME_PARTS: &[&str] = &[
    "key",
    "token",
    "secret",
    "password",
    "passwd",
    "auth",
    "credential",
];

/// MCP header and `env` values that look like a secret written in clear. The
/// MCP file of every target tool gets the value, and those files are usually
/// committed: a secret belongs in an environment variable (`${VAR}`), which
/// conforme rewrites to each tool's own syntax.
pub fn literal_secrets(config: &NormalizedConfig) -> Vec<String> {
    let mut found = Vec::new();
    for server in &config.mcp_servers {
        let headers = match &server.transport {
            McpTransport::Http { headers, .. } => Some(headers),
            McpTransport::Stdio { .. } => None,
        };
        let entries = headers
            .into_iter()
            .flatten()
            .map(|(k, v)| ("header", k, v))
            .chain(server.env.iter().map(|(k, v)| ("env", k, v)));
        for (kind, name, value) in entries {
            let lower = name.to_ascii_lowercase();
            let secret_name = SECRET_NAME_PARTS.iter().any(|part| lower.contains(part));
            let is_reference = value.contains("${") || value.contains("{env:");
            if secret_name && !is_reference && !value.trim().is_empty() {
                found.push(format!(
                    "MCP server '{}': {kind} '{name}' holds a literal value; it is copied into every \
                     tool's (usually committed) config, reference an environment variable instead (${{VAR}})",
                    server.name
                ));
            }
        }
    }
    found
}

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

    check_file_names(
        "Rule",
        config.rules.iter().map(|r| &r.name),
        rule_file_name,
        &mut errors,
    );
    // Cursor, Devin and Kiro write the instructions as a `general` rule file:
    // a rule of that name would overwrite them on every sync.
    if !config.instructions.trim().is_empty() {
        for rule in config
            .rules
            .iter()
            .filter(|r| rule_file_name(&r.name) == "general")
        {
            errors.push(format!(
                "Rule '{}' becomes 'general', the file the instructions are written to; rename it",
                rule.name
            ));
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
        if CLAUDE_RESERVED_SKILL_NAMES.contains(&sanitize_name(&skill.name).as_str()) {
            warnings.push(format!(
                "Skill '{}' uses a name Claude Code reserves; Claude Code skips it",
                skill.name
            ));
        }
        if skill.description.chars().count() > MAX_DESCRIPTION_LEN {
            warnings.push(format!(
                "Skill '{}' has a description over {MAX_DESCRIPTION_LEN} characters; Zoo Code and Mistral Vibe skip it",
                skill.name
            ));
        }
    }
    check_file_names(
        "Skill",
        config.skills.iter().map(|s| &s.name),
        sanitize_name,
        &mut errors,
    );

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
    check_file_names(
        "Agent",
        config.agents.iter().map(|a| &a.name),
        sanitize_name,
        &mut errors,
    );

    // Check for duplicate MCP server names
    let mut seen_mcp = HashSet::new();
    for mcp in &config.mcp_servers {
        if !seen_mcp.insert(&mcp.name) {
            errors.push(format!("Duplicate MCP server name: '{}'", mcp.name));
        }
        if mcp.name.trim().is_empty() {
            errors.push("MCP server with empty name found".to_string());
        }
        match &mcp.transport {
            McpTransport::Stdio { command, .. } if command.trim().is_empty() => {
                errors.push(format!("MCP server '{}' has an empty command", mcp.name));
            }
            McpTransport::Http { url, .. } if url.trim().is_empty() => {
                errors.push(format!("MCP server '{}' has an empty URL", mcp.name));
            }
            _ => {}
        }
    }

    warnings.extend(literal_secrets(config));

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

    #[test]
    fn test_blank_mcp_identity_and_transport_are_invalid() {
        let config = NormalizedConfig {
            mcp_servers: vec![
                NormalizedMcpServer {
                    name: " ".to_string(),
                    transport: McpTransport::Stdio {
                        command: "".to_string(),
                        args: Vec::new(),
                    },
                    env: Default::default(),
                },
                NormalizedMcpServer {
                    name: "api".to_string(),
                    transport: McpTransport::Http {
                        url: "  ".to_string(),
                        headers: Default::default(),
                    },
                    env: Default::default(),
                },
            ],
            ..Default::default()
        };

        assert!(!validate(&config, false));
    }

    #[test]
    fn test_names_must_sanitize_to_distinct_ascii() {
        let skill = |name: &str| NormalizedSkill {
            name: name.to_string(),
            content: "x".to_string(),
            ..Default::default()
        };
        let config = |skills| NormalizedConfig {
            skills,
            ..Default::default()
        };
        assert!(!validate(&config(vec![skill("部署")]), false));
        assert!(!validate(
            &config(vec![skill("Deploy App"), skill("deploy-app")]),
            false
        ));
        assert!(validate(&config(vec![skill("Déployer")]), false));

        // Rules only become files, which may keep non-ASCII letters.
        let rule = NormalizedConfig {
            rules: vec![NormalizedRule {
                name: "部署".to_string(),
                content: "x".to_string(),
                activation: ActivationMode::Always,
            }],
            ..Default::default()
        };
        assert!(validate(&rule, false));
        assert_eq!(rule_file_name("部署 Rule"), "部署-rule");
        assert_eq!(rule_file_name("Déployer"), "deployer");
    }

    #[test]
    fn test_rule_named_general_collides_with_instructions() {
        let rule = |instructions: &str| NormalizedConfig {
            instructions: instructions.to_string(),
            rules: vec![NormalizedRule {
                name: "General".to_string(),
                content: "Body.".to_string(),
                activation: ActivationMode::Always,
            }],
            ..Default::default()
        };
        assert!(!validate(&rule("Top instructions."), false));
        // Without instructions the `general` file is free.
        assert!(validate(&rule(""), false));
    }

    #[test]
    fn test_literal_secrets_in_mcp_are_flagged() {
        let server = |headers: &[(&str, &str)], env: &[(&str, &str)]| NormalizedMcpServer {
            name: "api".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            },
            env: env
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        };
        let config = |s: NormalizedMcpServer| NormalizedConfig {
            mcp_servers: vec![s],
            ..Default::default()
        };

        // A key written in clear is copied into every tool's committed file.
        assert_eq!(
            literal_secrets(&config(server(&[("x-api-key", "ctx7sk-abc123")], &[]))).len(),
            1
        );
        assert_eq!(
            literal_secrets(&config(server(&[("Authorization", "Bearer abc123")], &[]))).len(),
            1
        );
        assert_eq!(
            literal_secrets(&config(server(&[], &[("GITHUB_TOKEN", "ghp_abc")]))).len(),
            1
        );
        // References and non-secret values are fine.
        assert!(literal_secrets(&config(server(
            &[
                ("Authorization", "Bearer ${GITHUB_TOKEN}"),
                ("X-Region", "eu")
            ],
            &[("API_KEY", "${API_KEY}"), ("TOKEN", "${input:token}")]
        )))
        .is_empty());
    }
}
