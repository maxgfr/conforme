use anyhow::{bail, Result};

use crate::config::{
    ActivationMode, McpTransport, NormalizedAgent, NormalizedConfig, NormalizedMcpServer,
    NormalizedRule, NormalizedSkill,
};

/// Parse an AGENTS.md file into a NormalizedConfig.
///
/// Convention:
/// - Text before the first `##` heading becomes `instructions`
/// - `## Rule: <name>` → NormalizedRule with activation comments
/// - `## Skill: <name>` → NormalizedSkill with description/tools comments
/// - `## Agent: <name>` → NormalizedAgent with model/tools comments
/// - `## MCP: <name>` → NormalizedMcpServer with transport comments
pub fn parse_agents_md(content: &str) -> Result<NormalizedConfig> {
    let mut config = NormalizedConfig::new();
    let mut current_section: Option<Section> = None;

    for line in content.lines() {
        if let Some(name) = line.strip_prefix("## Rule: ") {
            flush_section(&mut config, current_section.take())?;
            current_section = Some(Section::Rule(name.trim().to_string(), Vec::new()));
        } else if let Some(name) = line.strip_prefix("## Skill: ") {
            flush_section(&mut config, current_section.take())?;
            current_section = Some(Section::Skill(name.trim().to_string(), Vec::new()));
        } else if let Some(name) = line.strip_prefix("## Agent: ") {
            flush_section(&mut config, current_section.take())?;
            current_section = Some(Section::Agent(name.trim().to_string(), Vec::new()));
        } else if let Some(name) = line.strip_prefix("## MCP: ") {
            flush_section(&mut config, current_section.take())?;
            current_section = Some(Section::Mcp(name.trim().to_string(), Vec::new()));
        } else if let Some((_name, lines)) = match &mut current_section {
            Some(Section::Rule(_, l)) => Some(("", l)),
            Some(Section::Skill(_, l)) => Some(("", l)),
            Some(Section::Agent(_, l)) => Some(("", l)),
            Some(Section::Mcp(_, l)) => Some(("", l)),
            None => None,
        } {
            lines.push(line.to_string());
        } else {
            config.instructions.push_str(line);
            config.instructions.push('\n');
        }
    }

    flush_section(&mut config, current_section.take())?;
    config.instructions = config.instructions.trim().to_string();

    Ok(config)
}

enum Section {
    Rule(String, Vec<String>),
    Skill(String, Vec<String>),
    Agent(String, Vec<String>),
    Mcp(String, Vec<String>),
}

fn flush_section(config: &mut NormalizedConfig, section: Option<Section>) -> Result<()> {
    match section {
        Some(Section::Rule(name, lines)) => {
            config.rules.push(build_rule(&name, &lines)?);
        }
        Some(Section::Skill(name, lines)) => {
            config.skills.push(build_skill(&name, &lines));
        }
        Some(Section::Agent(name, lines)) => {
            config.agents.push(build_agent(&name, &lines));
        }
        Some(Section::Mcp(name, lines)) => {
            if let Some(mcp) = build_mcp(&name, &lines) {
                config.mcp_servers.push(mcp);
            }
        }
        None => {}
    }
    Ok(())
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
            activation = parse_activation(inner.trim())?;
        } else if let Some(inner) = trimmed
            .strip_prefix("<!-- description:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            description = Some(inner.trim().to_string());
        } else {
            content_lines.push(line.as_str());
        }
    }

    if let Some(desc) = description {
        if matches!(activation, ActivationMode::AgentDecision { .. }) {
            activation = ActivationMode::AgentDecision { description: desc };
        }
    }

    if name.is_empty() {
        bail!("empty rule name");
    }

    Ok(NormalizedRule {
        name: name.to_string(),
        content: content_lines.join("\n").trim().to_string(),
        activation,
    })
}

fn build_skill(name: &str, lines: &[String]) -> NormalizedSkill {
    let mut description = String::new();
    let mut allowed_tools = Vec::new();
    let mut content_lines = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if let Some(inner) = trimmed
            .strip_prefix("<!-- description:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            description = inner.trim().to_string();
        } else if let Some(inner) = trimmed
            .strip_prefix("<!-- tools:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            allowed_tools = inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        } else {
            content_lines.push(line.as_str());
        }
    }

    NormalizedSkill {
        name: name.to_string(),
        description,
        content: content_lines.join("\n").trim().to_string(),
        allowed_tools,
    }
}

fn build_agent(name: &str, lines: &[String]) -> NormalizedAgent {
    let mut description = String::new();
    let mut model = None;
    let mut tools = Vec::new();
    let mut content_lines = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if let Some(inner) = trimmed
            .strip_prefix("<!-- description:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            description = inner.trim().to_string();
        } else if let Some(inner) = trimmed
            .strip_prefix("<!-- model:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            model = Some(inner.trim().to_string());
        } else if let Some(inner) = trimmed
            .strip_prefix("<!-- tools:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            tools = inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        } else {
            content_lines.push(line.as_str());
        }
    }

    NormalizedAgent {
        name: name.to_string(),
        description,
        content: content_lines.join("\n").trim().to_string(),
        model,
        tools,
    }
}

fn build_mcp(name: &str, lines: &[String]) -> Option<NormalizedMcpServer> {
    let mut command = None;
    let mut args = Vec::new();
    let mut url = None;
    let mut env = std::collections::BTreeMap::new();

    for line in lines {
        let trimmed = line.trim();
        if let Some(inner) = trimmed
            .strip_prefix("<!-- command:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            command = Some(inner.trim().to_string());
        } else if let Some(inner) = trimmed
            .strip_prefix("<!-- args:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            args = inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        } else if let Some(inner) = trimmed
            .strip_prefix("<!-- url:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            url = Some(inner.trim().to_string());
        } else if let Some(inner) = trimmed
            .strip_prefix("<!-- env:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            // Format: KEY=VALUE
            for pair in inner.split(',') {
                if let Some((k, v)) = pair.trim().split_once('=') {
                    env.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }

    let transport = if let Some(u) = url {
        McpTransport::Http {
            url: u,
            headers: std::collections::BTreeMap::new(),
        }
    } else if let Some(cmd) = command {
        McpTransport::Stdio { command: cmd, args }
    } else {
        return None;
    };

    Some(NormalizedMcpServer {
        name: name.to_string(),
        transport,
        env,
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

/// Export a NormalizedConfig back to AGENTS.md format.
pub fn export_as_agents_md(config: &NormalizedConfig) -> String {
    let mut out = String::new();

    if !config.instructions.is_empty() {
        out.push_str("# Project Instructions\n\n");
        out.push_str(&config.instructions);
        out.push('\n');
    }

    for rule in &config.rules {
        out.push_str(&format!("\n## Rule: {}\n", rule.name));
        match &rule.activation {
            ActivationMode::Always => {
                out.push_str("<!-- activation: always -->\n");
            }
            ActivationMode::GlobMatch(globs) => {
                out.push_str(&format!("<!-- activation: glob {} -->\n", globs.join(",")));
            }
            ActivationMode::AgentDecision { description } => {
                out.push_str("<!-- activation: agent-decision -->\n");
                if !description.is_empty() {
                    out.push_str(&format!("<!-- description: {} -->\n", description));
                }
            }
            ActivationMode::Manual => {
                out.push_str("<!-- activation: manual -->\n");
            }
        }
        out.push('\n');
        out.push_str(&rule.content);
        out.push('\n');
    }

    for skill in &config.skills {
        out.push_str(&format!("\n## Skill: {}\n", skill.name));
        if !skill.description.is_empty() {
            out.push_str(&format!("<!-- description: {} -->\n", skill.description));
        }
        if !skill.allowed_tools.is_empty() {
            out.push_str(&format!(
                "<!-- tools: {} -->\n",
                skill.allowed_tools.join(", ")
            ));
        }
        out.push('\n');
        out.push_str(&skill.content);
        out.push('\n');
    }

    for agent in &config.agents {
        out.push_str(&format!("\n## Agent: {}\n", agent.name));
        if !agent.description.is_empty() {
            out.push_str(&format!("<!-- description: {} -->\n", agent.description));
        }
        if let Some(model) = &agent.model {
            out.push_str(&format!("<!-- model: {} -->\n", model));
        }
        if !agent.tools.is_empty() {
            out.push_str(&format!("<!-- tools: {} -->\n", agent.tools.join(", ")));
        }
        out.push('\n');
        out.push_str(&agent.content);
        out.push('\n');
    }

    for mcp in &config.mcp_servers {
        out.push_str(&format!("\n## MCP: {}\n", mcp.name));
        match &mcp.transport {
            McpTransport::Stdio { command, args } => {
                out.push_str(&format!("<!-- command: {} -->\n", command));
                if !args.is_empty() {
                    out.push_str(&format!("<!-- args: {} -->\n", args.join(", ")));
                }
            }
            McpTransport::Http { url, .. } => {
                out.push_str(&format!("<!-- url: {} -->\n", url));
            }
        }
        if !mcp.env.is_empty() {
            let env_str: Vec<String> = mcp.env.iter().map(|(k, v)| format!("{k}={v}")).collect();
            out.push_str(&format!("<!-- env: {} -->\n", env_str.join(", ")));
        }
        out.push('\n');
    }

    out
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

    #[test]
    fn test_parse_skills() {
        let content = r#"## Skill: deploy
<!-- description: Deploy the application -->
<!-- tools: Bash, Read -->

Run `npm run deploy` to deploy.
"#;
        let config = parse_agents_md(content).unwrap();
        assert_eq!(config.skills.len(), 1);
        assert_eq!(config.skills[0].name, "deploy");
        assert_eq!(config.skills[0].description, "Deploy the application");
        assert_eq!(config.skills[0].allowed_tools, vec!["Bash", "Read"]);
        assert!(config.skills[0].content.contains("npm run deploy"));
    }

    #[test]
    fn test_parse_agents() {
        let content = r#"## Agent: reviewer
<!-- description: Find bugs and security issues -->
<!-- model: claude-sonnet-4-5 -->
<!-- tools: Read, Grep, Bash -->

Review code for correctness.
"#;
        let config = parse_agents_md(content).unwrap();
        assert_eq!(config.agents.len(), 1);
        assert_eq!(config.agents[0].name, "reviewer");
        assert_eq!(
            config.agents[0].model,
            Some("claude-sonnet-4-5".to_string())
        );
        assert_eq!(config.agents[0].tools.len(), 3);
    }

    #[test]
    fn test_parse_mcp_stdio() {
        let content = r#"## MCP: filesystem
<!-- command: npx -->
<!-- args: -y, @modelcontextprotocol/server-filesystem, /tmp -->

"#;
        let config = parse_agents_md(content).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        assert_eq!(config.mcp_servers[0].name, "filesystem");
        assert!(matches!(
            &config.mcp_servers[0].transport,
            McpTransport::Stdio { command, args } if command == "npx" && args.len() == 3
        ));
    }

    #[test]
    fn test_parse_mcp_http() {
        let content = "## MCP: github\n<!-- url: https://api.github.com/mcp -->\n\n";
        let config = parse_agents_md(content).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        assert!(matches!(
            &config.mcp_servers[0].transport,
            McpTransport::Http { url, .. } if url == "https://api.github.com/mcp"
        ));
    }
}
