//! Merging conforme-managed keys into JSON settings files the user also owns.
//!
//! `.zed/settings.json`, `.gemini/settings.json`, `.amp/settings.json`,
//! `opencode.json`, `.vscode/mcp.json` and `.roo/mcp.json` all hold settings
//! conforme does not generate. Most of those tools parse them as JSONC, so a
//! file may carry comments and trailing commas that `serde_json` rejects.
//!
//! The merge therefore:
//! - parses the file as JSONC, and refuses to write when even that fails,
//!   instead of replacing an unreadable file with conforme's keys alone;
//! - edits the existing text in place, so comments and formatting outside the
//!   managed keys survive (comments *inside* a managed key are regenerated);
//! - keeps, inside each managed server entry, the keys conforme never emits
//!   (Zoo Code's `alwaysAllow`, Gemini's `trust`, …).

use anyhow::{bail, Context, Result};
use jsonc_parser::cst::{CstInputValue, CstRootNode};
use jsonc_parser::ParseOptions;
use serde_json::{Map, Value};
use std::path::Path;

/// An existing settings file: its raw text and its parsed JSON value.
pub struct SettingsFile {
    text: String,
    value: Value,
}

impl SettingsFile {
    /// The parsed value of a top-level key, if present.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.value.get(key)
    }
}

/// Parse JSON or JSONC text (comments, trailing commas) into a JSON value.
pub fn parse_jsonc(content: &str) -> Result<Value> {
    let value: Option<Value> =
        jsonc_parser::parse_to_serde_value(content, &ParseOptions::default())
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(value.unwrap_or(Value::Null))
}

/// Load a settings file. Returns `None` when it does not exist, and an error
/// when it exists but is not a JSON(C) object — callers must then leave the
/// file untouched rather than overwrite it.
pub fn load(path: &Path) -> Result<Option<SettingsFile>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if text.trim().is_empty() {
        return Ok(Some(SettingsFile {
            text,
            value: Value::Object(Map::new()),
        }));
    }
    let value = parse_jsonc(&text).with_context(|| {
        format!(
            "failed to parse {}; conforme will not overwrite it",
            path.display()
        )
    })?;
    if !value.is_object() {
        bail!(
            "{} is not a JSON object; conforme will not overwrite it",
            path.display()
        );
    }
    Ok(Some(SettingsFile { text, value }))
}

/// Render the settings file with each `(key, value)` in `set` replaced or
/// added, and each `(key, value)` in `defaults` added only when missing.
pub fn render(
    existing: Option<&SettingsFile>,
    set: &[(&str, Value)],
    defaults: &[(&str, Value)],
) -> Result<String> {
    // Plain JSON (including every file conforme created) is merged and
    // pretty-printed with serde, so output stays byte-stable across syncs.
    // Only a file that needs JSONC (comments, trailing commas) is edited in
    // place, which is what keeps those comments.
    let file = match existing {
        Some(f) if !f.text.trim().is_empty() => f,
        _ => return render_plain(Map::new(), set, defaults),
    };
    if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&file.text) {
        return render_plain(map, set, defaults);
    }

    let root = CstRootNode::parse(&file.text, &ParseOptions::default())
        .map_err(|e| anyhow::anyhow!("failed to parse settings JSON: {e}"))?;
    let object = root.object_value_or_set();
    for (key, value) in defaults {
        if object.get(key).is_none() {
            object.append(key, to_cst(value));
        }
    }
    for (key, value) in set {
        match object.get(key) {
            Some(prop) => prop.set_value(to_cst(value)),
            None => {
                object.append(key, to_cst(value));
            }
        }
    }
    let mut out = root.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn render_plain(
    mut map: Map<String, Value>,
    set: &[(&str, Value)],
    defaults: &[(&str, Value)],
) -> Result<String> {
    for (key, value) in defaults {
        map.entry((*key).to_string())
            .or_insert_with(|| value.clone());
    }
    for (key, value) in set {
        map.insert((*key).to_string(), value.clone());
    }
    let json = serde_json::to_string_pretty(&Value::Object(map))
        .context("failed to serialize settings JSON")?;
    Ok(format!("{json}\n"))
}

/// Merge freshly generated server entries with the ones already on disk.
///
/// The generated set is authoritative for which servers exist and for every
/// key in `owned_keys` (the keys conforme may emit for this tool, across all
/// transports, plus any enable/disable flag it resets). Any other key an
/// existing entry carries is tool-specific configuration and is kept.
pub fn merge_server_entries(
    existing: Option<&Value>,
    generated: Map<String, Value>,
    owned_keys: &[&str],
) -> Map<String, Value> {
    let existing = existing.and_then(Value::as_object);
    generated
        .into_iter()
        .map(|(name, entry)| {
            let merged = match (existing.and_then(|e| e.get(&name)), entry) {
                (Some(Value::Object(old)), Value::Object(new)) => {
                    let mut merged = new;
                    for (key, value) in old {
                        if !owned_keys.contains(&key.as_str()) && !merged.contains_key(key) {
                            merged.insert(key.clone(), value.clone());
                        }
                    }
                    Value::Object(merged)
                }
                (_, entry) => entry,
            };
            (name, merged)
        })
        .collect()
}

fn to_cst(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(b) => CstInputValue::Bool(*b),
        Value::Number(n) => CstInputValue::Number(n.to_string()),
        Value::String(s) => CstInputValue::String(s.clone()),
        Value::Array(items) => CstInputValue::Array(items.iter().map(to_cst).collect()),
        Value::Object(map) => {
            CstInputValue::Object(map.iter().map(|(k, v)| (k.clone(), to_cst(v))).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(text: &str) -> SettingsFile {
        SettingsFile {
            text: text.to_string(),
            value: parse_jsonc(text).unwrap(),
        }
    }

    #[test]
    fn test_render_new_file_is_pretty_json() {
        let out = render(
            None,
            &[("mcp", json!({"a": 1}))],
            &[("$schema", json!("s"))],
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&out).unwrap(),
            json!({"$schema": "s", "mcp": {"a": 1}})
        );
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn test_render_keeps_comments_and_user_keys() {
        let text = "// Zed settings\n{\n  // my theme\n  \"theme\": \"One Dark\",\n  \"vim_mode\": true,\n}\n";
        let out = render(
            Some(&file(text)),
            &[("context_servers", json!({"fs": {"command": "npx"}}))],
            &[],
        )
        .unwrap();
        assert!(out.contains("// Zed settings"), "{out}");
        assert!(out.contains("// my theme"), "{out}");
        let parsed = parse_jsonc(&out).unwrap();
        assert_eq!(parsed["theme"], "One Dark");
        assert_eq!(parsed["vim_mode"], true);
        assert_eq!(parsed["context_servers"]["fs"]["command"], "npx");
    }

    #[test]
    fn test_render_replaces_managed_key_and_is_idempotent() {
        let text = "{\n  // keep\n  \"mcp\": {\"old\": {}},\n  \"x\": 1\n}\n";
        let set = [(
            "mcp",
            json!({"new": {"type": "local", "command": ["a", "b"]}}),
        )];
        let once = render(Some(&file(text)), &set, &[]).unwrap();
        let parsed = parse_jsonc(&once).unwrap();
        assert!(parsed["mcp"].get("old").is_none());
        assert_eq!(parsed["mcp"]["new"]["command"], json!(["a", "b"]));
        assert!(once.contains("// keep"));
        let twice = render(Some(&file(&once)), &set, &[]).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn test_render_defaults_only_fill_missing_keys() {
        let text = "{\"$schema\": \"mine\"}";
        let out = render(Some(&file(text)), &[], &[("$schema", json!("theirs"))]).unwrap();
        assert_eq!(parse_jsonc(&out).unwrap()["$schema"], "mine");
    }

    #[test]
    fn test_load_refuses_unparsable_and_non_object_files() {
        let dir = tempfile::TempDir::new().unwrap();
        let broken = dir.path().join("broken.json");
        std::fs::write(&broken, "{ \"a\": ").unwrap();
        assert!(load(&broken).is_err());
        let array = dir.path().join("array.json");
        std::fs::write(&array, "[1, 2]").unwrap();
        assert!(load(&array).is_err());
        assert!(load(&dir.path().join("missing.json")).unwrap().is_none());
    }

    #[test]
    fn test_merge_server_entries_keeps_tool_specific_keys() {
        let existing = json!({
            "fs": {"command": "old", "alwaysAllow": ["read"], "disabled": true},
            "gone": {"command": "x"}
        });
        let generated = json!({"fs": {"command": "npx", "args": ["-y"]}});
        let merged = merge_server_entries(
            Some(&existing),
            generated.as_object().unwrap().clone(),
            &[
                "type", "command", "args", "env", "url", "headers", "disabled",
            ],
        );
        assert_eq!(
            Value::Object(merged),
            json!({"fs": {"command": "npx", "args": ["-y"], "alwaysAllow": ["read"]}})
        );
    }
}
