use std::collections::BTreeMap;

/// Activation mode for a rule — determines when/where the rule applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationMode {
    /// Always active in every session
    Always,
    /// Active when files matching these glob patterns are in context
    GlobMatch(Vec<String>),
    /// Agent decides based on description
    AgentDecision { description: String },
    /// Only active when explicitly mentioned
    Manual,
}

/// A normalized rule extracted from AGENTS.md or a tool-specific config.
#[derive(Debug, Clone)]
pub struct NormalizedRule {
    pub name: String,
    pub content: String,
    pub activation: ActivationMode,
}

/// A normalized skill (reusable prompt template).
#[derive(Debug, Clone)]
pub struct NormalizedSkill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub allowed_tools: Vec<String>,
}

/// A normalized MCP server definition.
#[derive(Debug, Clone)]
pub struct NormalizedMcpServer {
    pub name: String,
    pub transport: McpTransport,
    pub env: BTreeMap<String, String>,
}

/// MCP server transport type.
#[derive(Debug, Clone)]
pub enum McpTransport {
    Stdio {
        command: String,
        args: Vec<String>,
    },
    Http {
        url: String,
        headers: BTreeMap<String, String>,
    },
}

/// A normalized custom agent definition.
#[derive(Debug, Clone)]
pub struct NormalizedAgent {
    pub name: String,
    pub description: String,
    pub content: String,
    pub model: Option<String>,
    pub tools: Vec<String>,
}

/// Full normalized configuration: instructions + rules + skills + MCP + agents.
#[derive(Debug, Clone)]
pub struct NormalizedConfig {
    /// Main instruction content (text before any ## headings)
    pub instructions: String,
    /// Individual rules with activation modes
    pub rules: Vec<NormalizedRule>,
    /// Reusable skills (SKILL.md files)
    pub skills: Vec<NormalizedSkill>,
    /// MCP server definitions
    pub mcp_servers: Vec<NormalizedMcpServer>,
    /// Custom agent definitions
    pub agents: Vec<NormalizedAgent>,
}

impl NormalizedConfig {
    pub fn new() -> Self {
        Self {
            instructions: String::new(),
            rules: Vec::new(),
            skills: Vec::new(),
            mcp_servers: Vec::new(),
            agents: Vec::new(),
        }
    }
}

impl Default for NormalizedConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Sanitize a rule name into a filesystem-safe identifier.
/// "TypeScript Conventions" → "typescript-conventions"
pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name() {
        assert_eq!(
            sanitize_name("TypeScript Conventions"),
            "typescript-conventions"
        );
        assert_eq!(sanitize_name("Security Review"), "security-review");
        assert_eq!(sanitize_name("my_rule"), "my-rule");
        assert_eq!(sanitize_name("  spaces  "), "spaces");
        assert_eq!(sanitize_name("CamelCase"), "camelcase");
    }
}
