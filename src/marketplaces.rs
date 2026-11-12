//! Plugins enabled from a marketplace ([`NormalizedPluginRef`]) and the
//! marketplaces a project declares ([`NormalizedMarketplace`]).
//!
//! Claude Code and GitHub Copilot keep both in a settings file with the same
//! keys, `enabledPlugins` (`"<plugin>@<marketplace>": true|false`) and
//! `extraKnownMarketplaces` (`"<name>": {"source": {...}}`): Claude Code in
//! `.claude/settings.json`
//! (https://code.claude.com/docs/en/settings-reference.md), Copilot CLI in
//! `.github/copilot/settings.json`, which also reads Claude Code's keys
//! (https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference).
//! Codex enables one with `[plugins."<plugin>@<marketplace>"] enabled` in
//! `.codex/config.toml`; the marketplace itself each user adds
//! (`codex plugin marketplace add`), since Codex clones no marketplace a
//! project names.

use anyhow::Result;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

use crate::config::{NormalizedConfig, NormalizedMarketplace, NormalizedPluginRef};

/// The marketplace Claude Code gives the bundles of `.claude/skills/`: their
/// switches are bundles' (see [`crate::plugins`]), not marketplace plugins.
pub const SKILLS_DIR_MARKETPLACE: &str = "skills-dir";

/// `<plugin>@<marketplace>` split, when it is one.
pub fn parse_id(id: &str) -> Option<(&str, &str)> {
    let (plugin, marketplace) = id.rsplit_once('@')?;
    (!plugin.is_empty() && !marketplace.is_empty()).then_some((plugin, marketplace))
}

/// The marketplace plugins and the marketplaces of a Claude Code or Copilot
/// settings document; `@skills-dir` switches are left out.
pub fn read_settings(settings: &Value) -> (Vec<NormalizedPluginRef>, Vec<NormalizedMarketplace>) {
    let refs = settings
        .get("enabledPlugins")
        .and_then(Value::as_object)
        .map(|enabled| {
            enabled
                .iter()
                .filter_map(|(id, on)| {
                    let (plugin, marketplace) = parse_id(id)?;
                    let enabled = on.as_bool()?;
                    (marketplace != SKILLS_DIR_MARKETPLACE).then(|| NormalizedPluginRef {
                        plugin: plugin.to_string(),
                        marketplace: marketplace.to_string(),
                        enabled,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let marketplaces = settings
        .get("extraKnownMarketplaces")
        .and_then(Value::as_object)
        .map(|known| {
            known
                .iter()
                .filter_map(|(name, entry)| {
                    Some(NormalizedMarketplace {
                        name: name.clone(),
                        source: entry.get("source").filter(|s| s.is_object())?.clone(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    (refs, marketplaces)
}

/// [`read_settings`] of a settings file; an unreadable or unparsable file
/// reads as none.
pub fn read_settings_file(path: &Path) -> (Vec<NormalizedPluginRef>, Vec<NormalizedMarketplace>) {
    crate::config::read_text(path)
        .ok()
        .and_then(|text| crate::json_settings::parse_jsonc(&text).ok())
        .map(|settings| read_settings(&settings))
        .unwrap_or_default()
}

/// The settings file with `enabledPlugins` and `extraKnownMarketplaces` as
/// the source has them, merged in place: the `@skills-dir` switches (Claude
/// Code's bundles) and every other key stay, and a marketplace entry keeps
/// its own keys besides `source` (`autoUpdate`). A source with no marketplace
/// plugin leaves the file untouched, and one with plugins but no marketplace
/// (Codex keeps them per user) leaves `extraKnownMarketplaces` alone.
pub fn settings_file(path: &Path, config: &NormalizedConfig) -> Result<Option<(PathBuf, String)>> {
    if config.plugin_refs.is_empty() && config.marketplaces.is_empty() {
        return Ok(None);
    }
    let existing = crate::json_settings::load(path)?;
    let key = |name: &str| existing.as_ref().and_then(|f| f.get(name));
    let mut enabled: Map<String, Value> = key("enabledPlugins")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter(|(id, _)| {
                    parse_id(id).is_some_and(|(_, market)| market == SKILLS_DIR_MARKETPLACE)
                })
                .map(|(id, on)| (id.clone(), on.clone()))
                .collect()
        })
        .unwrap_or_default();
    for plugin in &config.plugin_refs {
        enabled.insert(plugin.id(), Value::Bool(plugin.enabled));
    }
    let mut set = vec![("enabledPlugins", Value::Object(enabled))];
    if !config.marketplaces.is_empty() {
        let old = key("extraKnownMarketplaces").and_then(Value::as_object);
        let mut known = Map::new();
        for marketplace in &config.marketplaces {
            let mut entry = old
                .and_then(|o| o.get(&marketplace.name))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            entry.insert("source".to_string(), marketplace.source.clone());
            known.insert(marketplace.name.clone(), Value::Object(entry));
        }
        set.push(("extraKnownMarketplaces", Value::Object(known)));
    }
    let json = crate::json_settings::render(existing.as_ref(), &set, &[])?;
    Ok(Some((path.to_path_buf(), json)))
}

/// The marketplace plugins `[plugins."<id>"] enabled` lists in a Codex
/// `config.toml`, except those of `repo_marketplace` (conforme's bundles,
/// see the Codex adapter).
pub fn read_codex_refs(config_text: &str, repo_marketplace: &str) -> Vec<NormalizedPluginRef> {
    config_text
        .parse::<toml::Table>()
        .ok()
        .and_then(|table| {
            table
                .get("plugins")
                .and_then(toml::Value::as_table)
                .cloned()
        })
        .map(|plugins| {
            plugins
                .iter()
                .filter_map(|(id, entry)| {
                    let (plugin, marketplace) = parse_id(id)?;
                    let enabled = entry.get("enabled").and_then(toml::Value::as_bool)?;
                    (marketplace != repo_marketplace).then(|| NormalizedPluginRef {
                        plugin: plugin.to_string(),
                        marketplace: marketplace.to_string(),
                        enabled,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `config_text` whose `[plugins]` tables of marketplace plugins are the
/// source's `refs` (`enabled` set, the table's other keys kept); the tables of
/// `repo_marketplace` (conforme's bundles) and of plugins without a
/// marketplace are left alone. No ref: unchanged.
pub fn merge_codex_refs_toml(
    config_text: &str,
    refs: &[NormalizedPluginRef],
    repo_marketplace: &str,
) -> Result<String> {
    if refs.is_empty() {
        return Ok(config_text.to_string());
    }
    let mut document: toml_edit::DocumentMut = config_text.parse().map_err(|e| {
        anyhow::anyhow!("failed to parse .codex/config.toml, nothing was changed: {e}")
    })?;
    if document.get("plugins").is_none() {
        let mut table = toml_edit::Table::new();
        table.set_implicit(true);
        document.insert("plugins", toml_edit::Item::Table(table));
    }
    let Some(table) = document
        .get_mut("plugins")
        .and_then(|item| item.as_table_like_mut())
    else {
        anyhow::bail!(".codex/config.toml: `plugins` is not a table, nothing was changed");
    };
    let wanted: Vec<String> = refs.iter().map(NormalizedPluginRef::id).collect();
    let stale: Vec<String> = table
        .iter()
        .map(|(key, _)| key.to_string())
        .filter(|key| {
            parse_id(key).is_some_and(|(_, market)| market != repo_marketplace)
                && !wanted.contains(key)
        })
        .collect();
    for key in stale {
        table.remove(&key);
    }
    for plugin in refs {
        let key = plugin.id();
        if table.get(&key).is_none() {
            table.insert(&key, toml_edit::Item::Table(toml_edit::Table::new()));
        }
        if let Some(entry) = table.get_mut(&key).and_then(|i| i.as_table_like_mut()) {
            if entry.get("enabled").and_then(|v| v.as_bool()) != Some(plugin.enabled) {
                entry.insert("enabled", toml_edit::value(plugin.enabled));
            }
        }
    }
    Ok(document.to_string())
}

/// The `codex plugin marketplace add` argument for a marketplace source, when
/// Codex can add it: a GitHub repository or a Git URL.
pub fn codex_add_argument(source: &Value) -> Option<String> {
    match source.get("source").and_then(Value::as_str)? {
        "github" => source
            .get("repo")
            .and_then(Value::as_str)
            .map(str::to_string),
        "git" | "url" => source
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs() -> Vec<NormalizedPluginRef> {
        vec![NormalizedPluginRef {
            plugin: "formatter".to_string(),
            marketplace: "team".to_string(),
            enabled: true,
        }]
    }

    #[test]
    fn test_settings_keep_bundle_switches_and_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            "{\n  // mine\n  \"model\": \"opus\",\n  \"enabledPlugins\": {\"kit@skills-dir\": false, \"old@team\": true},\n  \"extraKnownMarketplaces\": {\"team\": {\"source\": {\"source\": \"github\", \"repo\": \"a/b\"}, \"autoUpdate\": true}}\n}\n",
        )
        .unwrap();
        let config = NormalizedConfig {
            plugin_refs: refs(),
            marketplaces: vec![NormalizedMarketplace {
                name: "team".to_string(),
                source: serde_json::json!({"source": "github", "repo": "acme/tools"}),
            }],
            ..Default::default()
        };
        let (_, text) = settings_file(&path, &config).unwrap().unwrap();
        assert!(
            text.contains("// mine") && text.contains("\"model\": \"opus\""),
            "{text}"
        );
        let value = crate::json_settings::parse_jsonc(&text).unwrap();
        assert_eq!(value["enabledPlugins"]["kit@skills-dir"], false);
        assert_eq!(value["enabledPlugins"]["formatter@team"], true);
        assert!(value["enabledPlugins"].get("old@team").is_none());
        assert_eq!(value["extraKnownMarketplaces"]["team"]["autoUpdate"], true);
        assert_eq!(
            value["extraKnownMarketplaces"]["team"]["source"]["repo"],
            "acme/tools"
        );
        let (read_refs, read_markets) = read_settings(&value);
        assert_eq!(read_refs, refs());
        assert_eq!(read_markets.len(), 1);

        let empty = NormalizedConfig::default();
        assert!(settings_file(&path, &empty).unwrap().is_none());
    }

    #[test]
    fn test_codex_refs_leave_conformes_bundles_alone() {
        let text = "[plugins.\"kit@conforme\"]\nenabled = true\n\n[plugins.\"old@team\"]\nenabled = true\n\n[plugins.\"formatter@team\"]\nenabled = false\nmcp_servers = {}\n";
        let merged = merge_codex_refs_toml(text, &refs(), "conforme").unwrap();
        assert!(merged.contains("[plugins.\"kit@conforme\"]"), "{merged}");
        assert!(!merged.contains("old@team"), "{merged}");
        assert!(
            merged.contains("[plugins.\"formatter@team\"]\nenabled = true"),
            "{merged}"
        );
        assert!(merged.contains("mcp_servers"), "{merged}");
        assert_eq!(read_codex_refs(&merged, "conforme"), refs());
        assert_eq!(merge_codex_refs_toml(text, &[], "conforme").unwrap(), text);
    }
}
