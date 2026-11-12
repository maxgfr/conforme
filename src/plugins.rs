//! Plugin bundles: a named package of rules, skills, agents and MCP servers
//! (see [`NormalizedPlugin`]).
//!
//! A tool that cannot hold a plugin bundle gets the plugin's content as its
//! own rules, skills, agents and MCP servers ([`flatten`]).

use anyhow::{bail, Result};
use std::collections::HashSet;

use crate::config::{
    rule_file_name, sanitize_name, ActivationMode, NormalizedConfig, NormalizedPlugin,
    NormalizedRule,
};

/// The variables a plugin's files use to reach the bundle's own folder
/// (`${CLAUDE_PLUGIN_ROOT}/scripts/check.sh`) or its data folder. They mean
/// nothing outside a loaded bundle.
const ROOT_VARIABLES: &[&str] = &[
    "${PLUGIN_ROOT}",
    "${PLUGIN_DATA}",
    "${CLAUDE_PLUGIN_ROOT}",
    "${CLAUDE_PLUGIN_DATA}",
    "${CURSOR_PLUGIN_ROOT}",
    "${COPILOT_PLUGIN_ROOT}",
];

/// Whether `text` reaches into the plugin's own folder.
pub fn uses_plugin_root(text: &str) -> bool {
    ROOT_VARIABLES.iter().any(|var| text.contains(var))
}

/// The names already taken in the flattened config, per kind, compared the
/// way each kind is written to disk.
struct Taken {
    rules: HashSet<String>,
    skills: HashSet<String>,
    agents: HashSet<String>,
    servers: HashSet<String>,
}

/// A free name for `name` from plugin `plugin`: the name itself, else
/// `<plugin>-<name>`; `None` when both are taken.
fn free_name(
    taken: &mut HashSet<String>,
    key: fn(&str) -> String,
    plugin: &str,
    name: &str,
) -> Option<String> {
    let prefixed = format!("{plugin}-{name}");
    [name.to_string(), prefixed]
        .into_iter()
        .find(|candidate| !taken.contains(&key(candidate)))
        .inspect(|candidate| {
            taken.insert(key(candidate));
        })
}

fn same(name: &str) -> String {
    name.to_string()
}

/// Place one item of `plugin` under a free name, or report it skipped.
fn place(
    taken: &mut HashSet<String>,
    key: fn(&str) -> String,
    plugin: &NormalizedPlugin,
    kind: &str,
    name: &str,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let placed = free_name(taken, key, &plugin.name, name);
    match &placed {
        None => warnings.push(format!(
            "plugin {}: {kind} {name} is left out: the project already has a {kind} named \
             {name} and one named {}-{name}",
            plugin.name, plugin.name
        )),
        Some(placed) if placed != name => warnings.push(format!(
            "plugin {}: {kind} {name} is written as {placed}, since the project already has \
             a {kind} named {name}",
            plugin.name
        )),
        Some(_) => {}
    }
    placed
}

/// The config with every plugin's content moved into the top-level rules,
/// skills, agents and MCP servers, for a tool that cannot hold a plugin
/// bundle, and the warnings about what that loses:
///
/// - a plugin's instructions become an always-on rule named after it;
/// - an item keeps its name, or becomes `<plugin>-<name>` when the project
///   (or an earlier plugin) already has one of that name, and is left out
///   when both are taken;
/// - an item that uses `${PLUGIN_ROOT}`-style variables (see
///   [`uses_plugin_root`]) only works inside the bundle and is left out;
/// - a disabled plugin is left out whole: a tool without bundles has no
///   switch to keep it off;
/// - the bundle's loose files have no place outside it.
///
/// A plugin with an empty name, or two plugins of the same name, are an
/// error.
pub fn flatten(config: &NormalizedConfig) -> Result<(NormalizedConfig, Vec<String>)> {
    let mut out = config.clone();
    if config.plugins.is_empty() {
        return Ok((out, Vec::new()));
    }
    out.plugins.clear();
    let mut warnings = Vec::new();
    let mut taken = Taken {
        rules: config
            .rules
            .iter()
            .map(|r| rule_file_name(&r.name))
            .collect(),
        skills: config
            .skills
            .iter()
            .map(|s| sanitize_name(&s.name))
            .collect(),
        agents: config
            .agents
            .iter()
            .map(|a| sanitize_name(&a.name))
            .collect(),
        servers: config.mcp_servers.iter().map(|m| m.name.clone()).collect(),
    };
    let mut plugin_names = HashSet::new();
    for plugin in &config.plugins {
        let key = sanitize_name(&plugin.name);
        if key.is_empty() {
            bail!(
                "plugin {:?} has no usable name: a plugin name needs at least one letter or digit",
                plugin.name
            );
        }
        if !plugin_names.insert(key) {
            bail!("plugin {} is defined twice", plugin.name);
        }
        if plugin.disabled {
            warnings.push(format!(
                "plugin {} is disabled, so tools that cannot hold a plugin bundle do not get \
                 its content",
                plugin.name
            ));
            continue;
        }
        if !plugin.files.is_empty() {
            let files: Vec<&str> = plugin.files.keys().map(String::as_str).collect();
            warnings.push(format!(
                "plugin {}: its bundled files ({}) only exist inside a plugin bundle",
                plugin.name,
                files.join(", ")
            ));
        }
        let skipped = |kind: &str, name: &str, warnings: &mut Vec<String>| {
            warnings.push(format!(
                "plugin {}: {kind} {name} is left out: it uses the plugin's own folder \
                 (${{PLUGIN_ROOT}}), which only exists inside a plugin bundle",
                plugin.name
            ));
        };

        let mut rules = Vec::new();
        if !plugin.instructions.trim().is_empty() {
            rules.push(NormalizedRule {
                name: plugin.name.clone(),
                content: plugin.instructions.clone(),
                activation: ActivationMode::Always,
            });
        }
        rules.extend(plugin.rules.iter().cloned());
        for mut rule in rules {
            if uses_plugin_root(&rule.content) {
                skipped("rule", &rule.name, &mut warnings);
                continue;
            }
            let Some(name) = place(
                &mut taken.rules,
                rule_file_name,
                plugin,
                "rule",
                &rule.name,
                &mut warnings,
            ) else {
                continue;
            };
            rule.name = name;
            out.rules.push(rule);
        }

        for skill in &plugin.skills {
            if uses_plugin_root(&skill.content) || skill.files.values().any(|f| uses_plugin_root(f))
            {
                skipped("skill", &skill.name, &mut warnings);
                continue;
            }
            let Some(name) = place(
                &mut taken.skills,
                sanitize_name,
                plugin,
                "skill",
                &skill.name,
                &mut warnings,
            ) else {
                continue;
            };
            out.skills.push(crate::config::NormalizedSkill {
                name,
                ..skill.clone()
            });
        }

        for agent in &plugin.agents {
            if uses_plugin_root(&agent.content) {
                skipped("agent", &agent.name, &mut warnings);
                continue;
            }
            let Some(name) = place(
                &mut taken.agents,
                sanitize_name,
                plugin,
                "agent",
                &agent.name,
                &mut warnings,
            ) else {
                continue;
            };
            out.agents.push(crate::config::NormalizedAgent {
                name,
                ..agent.clone()
            });
        }

        for server in &plugin.mcp_servers {
            if crate::mcp::server_strings(server)
                .iter()
                .any(|(_, value)| uses_plugin_root(value))
            {
                skipped("MCP server", &server.name, &mut warnings);
                continue;
            }
            let Some(name) = place(
                &mut taken.servers,
                same,
                plugin,
                "MCP server",
                &server.name,
                &mut warnings,
            ) else {
                continue;
            };
            out.mcp_servers.push(crate::config::NormalizedMcpServer {
                name,
                ..server.clone()
            });
        }
    }
    Ok((out, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpTransport, NormalizedMcpServer, NormalizedSkill};

    fn skill(name: &str, content: &str) -> NormalizedSkill {
        NormalizedSkill {
            name: name.to_string(),
            description: format!("{name} skill"),
            content: content.to_string(),
            ..Default::default()
        }
    }

    fn plugin(name: &str) -> NormalizedPlugin {
        NormalizedPlugin {
            name: name.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_flatten_moves_every_component_to_the_top_level() {
        let mut demo = plugin("demo");
        demo.instructions = "Always be kind.".to_string();
        demo.skills.push(skill("lint", "Run the linter."));
        demo.agents.push(crate::config::NormalizedAgent {
            name: "reviewer".to_string(),
            content: "Review.".to_string(),
            ..Default::default()
        });
        demo.mcp_servers.push(NormalizedMcpServer {
            name: "docs".to_string(),
            transport: McpTransport::Http {
                url: "https://example.com/mcp".to_string(),
                headers: Default::default(),
            },
            env: Default::default(),
        });
        let config = NormalizedConfig {
            plugins: vec![demo],
            ..Default::default()
        };

        let (flat, warnings) = flatten(&config).unwrap();

        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(flat.plugins.is_empty());
        assert_eq!(flat.rules[0].name, "demo");
        assert_eq!(flat.rules[0].activation, ActivationMode::Always);
        assert_eq!(flat.skills[0].name, "lint");
        assert_eq!(flat.agents[0].name, "reviewer");
        assert_eq!(flat.mcp_servers[0].name, "docs");
    }

    #[test]
    fn test_flatten_prefixes_a_taken_name_then_leaves_it_out() {
        let mut demo = plugin("demo");
        demo.skills.push(skill("lint", "Plugin lint."));
        let config = NormalizedConfig {
            skills: vec![skill("lint", "Project lint.")],
            plugins: vec![demo.clone()],
            ..Default::default()
        };
        let (flat, warnings) = flatten(&config).unwrap();
        let names: Vec<_> = flat.skills.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["lint", "demo-lint"]);
        assert!(warnings[0].contains("written as demo-lint"), "{warnings:?}");

        let config = NormalizedConfig {
            skills: vec![skill("lint", "A."), skill("demo-lint", "B.")],
            plugins: vec![demo],
            ..Default::default()
        };
        let (flat, warnings) = flatten(&config).unwrap();
        assert_eq!(flat.skills.len(), 2);
        assert!(warnings[0].contains("left out"), "{warnings:?}");
    }

    #[test]
    fn test_flatten_leaves_out_items_using_the_plugin_root() {
        let mut demo = plugin("demo");
        demo.skills.push(skill(
            "check",
            "Run ${CLAUDE_PLUGIN_ROOT}/scripts/check.sh.",
        ));
        demo.mcp_servers.push(NormalizedMcpServer {
            name: "local".to_string(),
            transport: McpTransport::Stdio {
                command: "node".to_string(),
                args: vec!["${PLUGIN_ROOT}/server.js".to_string()],
            },
            env: Default::default(),
        });
        let config = NormalizedConfig {
            plugins: vec![demo],
            ..Default::default()
        };
        let (flat, warnings) = flatten(&config).unwrap();
        assert!(flat.skills.is_empty() && flat.mcp_servers.is_empty());
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }

    #[test]
    fn test_flatten_skips_a_disabled_plugin_and_refuses_duplicates() {
        let mut off = plugin("off");
        off.disabled = true;
        off.skills.push(skill("x", "X."));
        let config = NormalizedConfig {
            plugins: vec![off],
            ..Default::default()
        };
        let (flat, warnings) = flatten(&config).unwrap();
        assert!(flat.skills.is_empty());
        assert!(warnings[0].contains("disabled"));

        let config = NormalizedConfig {
            plugins: vec![plugin("Demo"), plugin("demo")],
            ..Default::default()
        };
        assert!(flatten(&config).is_err());
        let config = NormalizedConfig {
            plugins: vec![plugin("!!")],
            ..Default::default()
        };
        assert!(flatten(&config).is_err());
    }
}
