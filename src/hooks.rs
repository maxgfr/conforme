//! Hooks: commands a tool runs on its lifecycle events
//! ([`NormalizedHook`]), in each tool's format.
//!
//! The normalized event and matcher use Claude Code's vocabulary
//! (https://code.claude.com/docs/en/hooks). Every format carries what it can
//! express, and reports the rest. A hook script receives each tool's own JSON
//! on stdin and blocks an action its own way, so a hook translated from
//! another tool is only as safe as its script's handling of that payload: the
//! tools that get a translated hook warn about it.

use anyhow::Result;
use serde_json::{json, Map, Value};

use crate::config::NormalizedHook;

/// Claude Code's events a matcher applies to.
const TOOL_EVENTS: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
];

/// `${CLAUDE_PLUGIN_ROOT}` / `${CLAUDE_PLUGIN_DATA}` (Claude Code) and
/// `${PLUGIN_ROOT}` / `${PLUGIN_DATA}` (Vibe, Antigravity, the Agent Plugins
/// spec) name the same folders: normalized to the latter.
pub fn normalize_root(command: &str) -> String {
    command
        .replace("${CLAUDE_PLUGIN_ROOT}", "${PLUGIN_ROOT}")
        .replace("${CLAUDE_PLUGIN_DATA}", "${PLUGIN_DATA}")
}

/// [`normalize_root`] spelled for Claude Code.
fn claude_root(command: &str) -> String {
    command
        .replace("${PLUGIN_ROOT}", "${CLAUDE_PLUGIN_ROOT}")
        .replace("${PLUGIN_DATA}", "${CLAUDE_PLUGIN_DATA}")
}

/// A plugin hook's command for Antigravity, which runs it with "their working
/// directory (`cwd`) set to `<plugin-root>`" and asks for "relative paths
/// (such as `node ./scripts/policy.mjs`) rather than `${PLUGIN_ROOT}`"
/// (https://antigravity.google/docs/build-plugins/); `agy plugin validate
/// --strict` fails on `${PLUGIN_ROOT}` there since 1.5.6.
pub fn plugin_relative(command: &str) -> String {
    command
        .replace("${PLUGIN_ROOT}/", "./")
        .replace("${PLUGIN_ROOT}", ".")
}

/// An Antigravity plugin hook's command read back: a `./` path, relative to
/// the plugin root, becomes `${PLUGIN_ROOT}/` again.
pub fn plugin_rooted(command: &str) -> String {
    let mut out = String::with_capacity(command.len());
    let mut previous: Option<char> = None;
    let mut rest = command;
    while let Some(c) = rest.chars().next() {
        let at_word_start = previous.is_none_or(|p| {
            p.is_whitespace() || matches!(p, '"' | '\'' | '=' | '(' | ';' | '&' | '|')
        });
        if at_word_start && rest.starts_with("./") {
            out.push_str("${PLUGIN_ROOT}/");
            rest = &rest[2..];
            previous = Some('/');
            continue;
        }
        out.push(c);
        previous = Some(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// The tool names of a Claude Code matcher: "`Edit|Write` or `Edit, Write`
/// match either tool" (https://code.claude.com/docs/en/hooks).
fn matcher_names(matcher: &str) -> Vec<&str> {
    matcher.split(['|', ',']).map(str::trim).collect()
}

/// `PreToolUse` → `pre-tool-use`, for a hook's generated name.
fn kebab(event: &str) -> String {
    let mut out = String::new();
    for (i, c) in event.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('-');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// A name for each unnamed hook: `<event>-<n>`, counted per event.
fn name_hooks(hooks: &mut [NormalizedHook]) {
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for hook in hooks.iter_mut() {
        let n = counts.entry(hook.event.clone()).or_default();
        *n += 1;
        if hook.name.is_empty() {
            hook.name = format!("{}-{n}", kebab(&hook.event));
        }
    }
}

/// The hooks of a Claude Code `hooks` object (`{Event: [{matcher, hooks:
/// [{type: "command", command, timeout}]}]}`); a handler of another type
/// (`http`, `prompt`, …) is left out.
pub fn read_claude(hooks: &Value) -> Vec<NormalizedHook> {
    let mut out = Vec::new();
    let Some(events) = hooks.as_object() else {
        return out;
    };
    for (event, groups) in events {
        for group in groups.as_array().into_iter().flatten() {
            let matcher = group
                .get("matcher")
                .and_then(Value::as_str)
                .filter(|m| !m.is_empty() && *m != "*")
                .map(str::to_string);
            for handler in group
                .get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if handler.get("type").and_then(Value::as_str) != Some("command") {
                    continue;
                }
                let Some(command) = handler.get("command").and_then(Value::as_str) else {
                    continue;
                };
                out.push(NormalizedHook {
                    name: String::new(),
                    event: event.clone(),
                    matcher: matcher.clone(),
                    command: normalize_root(command),
                    timeout: handler.get("timeout").and_then(Value::as_u64),
                });
            }
        }
    }
    name_hooks(&mut out);
    out
}

/// A Claude Code `hooks` object for `hooks`, one matcher group per hook.
pub fn claude_json(hooks: &[NormalizedHook]) -> Value {
    let mut events: Map<String, Value> = Map::new();
    for hook in hooks {
        let mut handler = json!({ "type": "command", "command": claude_root(&hook.command) });
        if let Some(timeout) = hook.timeout {
            handler["timeout"] = json!(timeout);
        }
        let mut group = Map::new();
        if let Some(matcher) = &hook.matcher {
            group.insert("matcher".into(), Value::String(matcher.clone()));
        }
        group.insert("hooks".into(), json!([handler]));
        events
            .entry(hook.event.clone())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("an array")
            .push(Value::Object(group));
    }
    Value::Object(events)
}

/// The events of Codex's hooks file (`HookEventsToml` in
/// `codex-rs/protocol/src/hooks.rs`), which ignores any other key without a
/// word.
const CODEX_EVENTS: &[&str] = &[
    "PreModelRequest",
    "PostModelRequest",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "SubagentStart",
    "SubagentStop",
    "Stop",
    "Interrupt",
];

/// A Codex `hooks` object (Claude Code's shape) for the hooks on an event
/// Codex has, and a warning for each of the others.
pub fn codex_json(hooks: &[NormalizedHook], owner: &str) -> (Option<Value>, Vec<String>) {
    let (kept, warnings) = on_events(hooks, CODEX_EVENTS, owner, "Codex");
    let json = (!kept.is_empty()).then(|| claude_json(&kept));
    (json, warnings)
}

/// The hooks on one of `events`, and a warning for each other one.
fn on_events(
    hooks: &[NormalizedHook],
    events: &[&str],
    owner: &str,
    tool: &str,
) -> (Vec<NormalizedHook>, Vec<String>) {
    let mut kept = Vec::new();
    let mut warnings = Vec::new();
    for hook in hooks {
        if events.contains(&hook.event.as_str()) {
            kept.push(hook.clone());
        } else {
            warnings.push(format!(
                "{owner}: hook {} is not written: {tool} has no {} event",
                hook.name, hook.event
            ));
        }
    }
    (kept, warnings)
}

/// Copilot's events by their Claude Code name, which `copilot help config`
/// (CLI 1.0.109) maps, and their own (camelCase) name: "Configure the event
/// name in PascalCase (for example, `SessionStart`)" selects the payload
/// shared with VS Code and Claude Code's tool names in matchers
/// (https://docs.github.com/en/copilot/reference/hooks-reference).
const COPILOT_EVENTS: &[(&str, &str)] = &[
    ("PreToolUse", "preToolUse"),
    ("PostToolUse", "postToolUse"),
    ("UserPromptSubmit", "userPromptSubmitted"),
    ("Stop", "agentStop"),
    ("SessionStart", "sessionStart"),
];

/// A Copilot `hooks` object, keyed by event name with "the same schema as
/// `.github/hooks/*.json` files": one flat entry per hook (`type`,
/// `command`, `matcher`, `timeoutSec`), never Claude Code's matcher groups,
/// since in a settings file "any item-level validation error rejects the
/// whole `hooks` field" (cli-config-dir-reference, hooks-reference). A hook
/// on another event is left out with a warning.
pub fn copilot_json(hooks: &[NormalizedHook], owner: &str) -> (Option<Value>, Vec<String>) {
    let events: Vec<&str> = COPILOT_EVENTS.iter().map(|(claude, _)| *claude).collect();
    let (kept, warnings) = on_events(hooks, &events, owner, "Copilot");
    if kept.is_empty() {
        return (None, warnings);
    }
    let mut out: Map<String, Value> = Map::new();
    for hook in &kept {
        let mut entry = json!({ "type": "command", "command": hook.command });
        if let Some(matcher) = &hook.matcher {
            entry["matcher"] = json!(matcher);
        }
        if let Some(timeout) = hook.timeout {
            entry["timeoutSec"] = json!(timeout);
        }
        out.entry(hook.event.clone())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("an array")
            .push(entry);
    }
    (Some(Value::Object(out)), warnings)
}

/// The hooks of a Copilot `hooks` object: flat entries under a Claude Code
/// or Copilot event name, and Claude Code's matcher groups, which earlier
/// conforme versions wrote there.
pub fn read_copilot(hooks: &Value) -> Vec<NormalizedHook> {
    let mut out = Vec::new();
    let Some(events) = hooks.as_object() else {
        return out;
    };
    for (name, entries) in events {
        let Some((event, _)) = COPILOT_EVENTS
            .iter()
            .find(|(claude, copilot)| claude == name || copilot == name)
        else {
            continue;
        };
        for entry in entries.as_array().into_iter().flatten() {
            if entry.get("hooks").is_some() {
                out.extend(read_claude(&json!({ *event: [entry] })));
                continue;
            }
            if entry.get("type").and_then(Value::as_str) != Some("command") {
                continue;
            }
            let Some(command) = ["command", "bash"]
                .iter()
                .find_map(|k| entry.get(*k).and_then(Value::as_str))
            else {
                continue;
            };
            out.push(NormalizedHook {
                name: String::new(),
                event: event.to_string(),
                matcher: entry
                    .get("matcher")
                    .and_then(Value::as_str)
                    .filter(|m| !m.is_empty() && *m != "*")
                    .map(str::to_string),
                command: command.to_string(),
                timeout: entry
                    .get("timeoutSec")
                    .or_else(|| entry.get("timeout"))
                    .and_then(Value::as_u64),
            });
        }
    }
    for hook in &mut out {
        hook.name.clear();
    }
    name_hooks(&mut out);
    out
}

/// Vibe's hook types for Claude Code's events: `pre_tool`, `post_tool`,
/// `post_agent`, the three Vibe's own Claude Code importer maps
/// (`vibe/core/plugins/_claude.py`). Its other types (`pre_agent`,
/// `notification`, `session_start`, `session_end`, Vibe 3.1.1) are fired
/// by its app server, not in the turn Claude Code's events describe.
const VIBE_EVENTS: &[(&str, &str)] = &[
    ("PreToolUse", "pre_tool"),
    ("PostToolUse", "post_tool"),
    ("Stop", "post_agent"),
];

/// Claude Code's tool names a hook matcher commonly lists, to read a Vibe
/// `match` back.
const CLAUDE_TOOLS: &[&str] = &[
    "Bash",
    "Read",
    "Write",
    "Edit",
    "MultiEdit",
    "Grep",
    "Glob",
    "WebFetch",
    "WebSearch",
    "Task",
    "TodoWrite",
];

/// A Claude Code matcher as a Vibe `match`: one tool name, or `re:^(a|b)$`
/// for several; `None` when a name has no Vibe equivalent or the matcher is
/// a pattern, not a list of names.
fn vibe_match(matcher: &str) -> Option<String> {
    let names = matcher_names(matcher);
    if names
        .iter()
        .any(|n| n.is_empty() || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
    {
        return None;
    }
    let mut tools: Vec<&str> = Vec::new();
    for name in names {
        let tool = crate::skills::vibe_tool(name)?;
        if !tools.contains(&tool) {
            tools.push(tool);
        }
    }
    Some(match tools.as_slice() {
        [one] => one.to_string(),
        many => format!("re:^({})$", many.join("|")),
    })
}

/// A Vibe `match` read back as a Claude Code matcher.
fn claude_matcher(vibe: &str) -> String {
    let names: Vec<&str> = match vibe
        .strip_prefix("re:^(")
        .and_then(|rest| rest.strip_suffix(")$"))
    {
        Some(inner) => inner.split('|').collect(),
        None => vec![vibe],
    };
    names
        .iter()
        .map(|vibe_name| {
            CLAUDE_TOOLS
                .iter()
                .find(|claude| crate::skills::vibe_tool(claude) == Some(vibe_name))
                .map_or_else(|| vibe_name.to_string(), |claude| claude.to_string())
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// A Vibe `hooks.toml` (`[[hooks]]` with `name`, `type`, `match`, `command`,
/// `timeout`) for the hooks it can express, and a warning for each one it
/// cannot: another event, or a matcher naming a tool Vibe does not have.
pub fn vibe_toml(hooks: &[NormalizedHook], owner: &str) -> (Option<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut array = toml_edit::ArrayOfTables::new();
    for hook in hooks {
        let Some((_, kind)) = VIBE_EVENTS.iter().find(|(e, _)| *e == hook.event) else {
            warnings.push(format!(
                "{owner}: hook {} is not written: Vibe has no {} event",
                hook.name, hook.event
            ));
            continue;
        };
        let matcher = match &hook.matcher {
            Some(m) if hook.event != "Stop" => match vibe_match(m) {
                Some(vibe) => Some(vibe),
                None => {
                    warnings.push(format!(
                        "{owner}: hook {} is not written: Vibe has no tool matching {m}",
                        hook.name
                    ));
                    continue;
                }
            },
            _ => None,
        };
        let mut table = toml_edit::Table::new();
        table["name"] = toml_edit::value(&hook.name);
        table["type"] = toml_edit::value(*kind);
        if let Some(matcher) = matcher {
            table["match"] = toml_edit::value(matcher);
        }
        table["command"] = toml_edit::value(&hook.command);
        if let Some(timeout) = hook.timeout {
            table["timeout"] = toml_edit::value(timeout as i64);
        }
        array.push(table);
    }
    if array.is_empty() {
        return (None, warnings);
    }
    let mut document = toml_edit::DocumentMut::new();
    document.insert("hooks", toml_edit::Item::ArrayOfTables(array));
    (Some(document.to_string()), warnings)
}

/// The hooks of a Vibe `hooks.toml`.
pub fn read_vibe(text: &str) -> Result<Vec<NormalizedHook>> {
    let table: toml::Table = text.parse()?;
    let mut out = Vec::new();
    for hook in table
        .get("hooks")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        let text = |k: &str| {
            hook.get(k)
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        };
        let (Some(kind), Some(command)) = (text("type"), text("command")) else {
            continue;
        };
        let Some((event, _)) = VIBE_EVENTS.iter().find(|(_, k)| *k == kind) else {
            continue;
        };
        out.push(NormalizedHook {
            name: text("name").unwrap_or_default(),
            event: event.to_string(),
            matcher: text("match").map(|m| claude_matcher(&m)),
            command,
            timeout: hook
                .get("timeout")
                .and_then(|t| t.as_integer().or_else(|| t.as_float().map(|f| f as i64)))
                .and_then(|t| u64::try_from(t).ok()),
        });
    }
    name_hooks(&mut out);
    Ok(out)
}

/// Antigravity's events: tool events take a matcher and nested handlers,
/// the others a flat list (https://antigravity.google/docs/hooks/).
const ANTIGRAVITY_EVENTS: &[&str] = &["PreToolUse", "PostToolUse", "SessionStart", "Stop"];

/// The group conforme writes its hooks under in an Antigravity `hooks.json`.
const ANTIGRAVITY_GROUP: &str = "conforme";

/// Claude Code's tool names as an Antigravity matcher, as `agy plugin
/// validate` (Antigravity CLI 1.5.3) names them: "Bash is Claude Code's name
/// for a tool, so it never matches; match \"run_command\" instead". Where it
/// names two, both are matched. `TodoWrite` has no equivalent.
const ANTIGRAVITY_TOOLS: &[(&str, &str)] = &[
    ("Bash", "run_command"),
    ("Read", "view_file|read_file"),
    ("Write", "write_to_file|write_file"),
    ("Edit", "replace_file_content|edit_file"),
    ("MultiEdit", "multi_replace_file_content|edit_file"),
    ("Grep", "grep_search"),
    ("Glob", "find_by_name"),
    ("WebFetch", "read_url_content"),
    ("WebSearch", "search_web"),
    ("Task", "invoke_subagent"),
    ("LS", "list_dir"),
    ("NotebookEdit", "notebook_edit"),
];

/// A Claude Code matcher in Antigravity's tool names; `None` when a name has
/// no equivalent or the matcher is a pattern, not a list of names.
fn antigravity_match(matcher: &str) -> Option<String> {
    let mut tools: Vec<&str> = Vec::new();
    for name in matcher_names(matcher) {
        let (_, agy) = ANTIGRAVITY_TOOLS
            .iter()
            .find(|(claude, _)| *claude == name)?;
        for tool in agy.split('|') {
            if !tools.contains(&tool) {
                tools.push(tool);
            }
        }
    }
    Some(tools.join("|"))
}

/// An Antigravity matcher read back in Claude Code's tool names; a name with
/// no Claude Code equivalent is kept as it is.
fn claude_from_antigravity(matcher: &str) -> String {
    let mut names: Vec<String> = Vec::new();
    for tool in matcher.split('|').map(str::trim) {
        let name = ANTIGRAVITY_TOOLS
            .iter()
            .find(|(_, agy)| agy.split('|').any(|t| t == tool))
            .map_or_else(|| tool.to_string(), |(claude, _)| claude.to_string());
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names.join("|")
}

/// An Antigravity `hooks.json` (`{"<group>": {"PreToolUse": [{matcher,
/// hooks: [...]}], "SessionStart": [{type, command, timeout}]}}`) for the
/// hooks it can express, and a warning for the others.
pub fn antigravity_json(hooks: &[NormalizedHook], owner: &str) -> (Option<Value>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut events: Map<String, Value> = Map::new();
    for hook in hooks {
        if !ANTIGRAVITY_EVENTS.contains(&hook.event.as_str()) {
            warnings.push(format!(
                "{owner}: hook {} is not written: Antigravity has no {} event",
                hook.name, hook.event
            ));
            continue;
        }
        let mut handler = json!({ "type": "command", "command": hook.command });
        if let Some(timeout) = hook.timeout {
            handler["timeout"] = json!(timeout);
        }
        let entry = if TOOL_EVENTS.contains(&hook.event.as_str()) {
            let matcher = match &hook.matcher {
                None => "*".to_string(),
                Some(m) => match antigravity_match(m) {
                    Some(agy) => agy,
                    None => {
                        warnings.push(format!(
                            "{owner}: hook {} is not written: Antigravity has no tool \
                             matching {m}",
                            hook.name
                        ));
                        continue;
                    }
                },
            };
            let mut group = Map::new();
            group.insert("matcher".into(), Value::String(matcher));
            group.insert("hooks".into(), json!([handler]));
            Value::Object(group)
        } else {
            handler
        };
        events
            .entry(hook.event.clone())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("an array")
            .push(entry);
    }
    if events.is_empty() {
        return (None, warnings);
    }
    (
        Some(json!({ ANTIGRAVITY_GROUP: Value::Object(events) })),
        warnings,
    )
}

/// The hooks of an Antigravity `hooks.json`, every group's.
pub fn read_antigravity(doc: &Value) -> Vec<NormalizedHook> {
    let mut out = Vec::new();
    for group in doc.as_object().into_iter().flat_map(|o| o.values()) {
        if group.get("enabled").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        for event in ANTIGRAVITY_EVENTS {
            for entry in group
                .get(*event)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let (matcher, handlers): (Option<String>, Vec<&Value>) =
                    match entry.get("hooks").and_then(Value::as_array) {
                        Some(nested) => (
                            entry
                                .get("matcher")
                                .and_then(Value::as_str)
                                .filter(|m| !m.is_empty() && *m != "*")
                                .map(claude_from_antigravity),
                            nested.iter().collect(),
                        ),
                        None => (None, vec![entry]),
                    };
                for handler in handlers {
                    if handler.get("type").and_then(Value::as_str) != Some("command") {
                        continue;
                    }
                    let Some(command) = handler.get("command").and_then(Value::as_str) else {
                        continue;
                    };
                    out.push(NormalizedHook {
                        name: String::new(),
                        event: event.to_string(),
                        matcher: matcher.clone(),
                        command: command.to_string(),
                        timeout: handler.get("timeout").and_then(Value::as_u64),
                    });
                }
            }
        }
    }
    name_hooks(&mut out);
    out
}

/// The prefix of the project hooks conforme writes in Vibe's `.vibe/hooks.toml`:
/// any other hook there is the user's.
pub const VIBE_PREFIX: &str = "conforme-";

/// `existing` (a `.vibe/hooks.toml`) with conforme's hooks (named
/// `conforme-…`) replaced by `hooks`, the user's own kept as they are, and a
/// warning for each hook Vibe cannot express.
pub fn merge_vibe_project_toml(
    existing: &str,
    hooks: &[NormalizedHook],
) -> Result<(String, Vec<String>)> {
    let mut document: toml_edit::DocumentMut = existing.parse().map_err(|e| {
        anyhow::anyhow!("failed to parse .vibe/hooks.toml, nothing was changed: {e}")
    })?;
    let prefixed: Vec<NormalizedHook> = hooks
        .iter()
        .map(|h| NormalizedHook {
            name: format!("{VIBE_PREFIX}{}", h.name),
            ..h.clone()
        })
        .collect();
    let (ours, warnings) = vibe_toml(&prefixed, "project");
    let mut tables = toml_edit::ArrayOfTables::new();
    if let Some(array) = document.get("hooks").and_then(|i| i.as_array_of_tables()) {
        for table in array.iter() {
            let theirs = !table
                .get("name")
                .and_then(|n| n.as_str())
                .is_some_and(|n| n.starts_with(VIBE_PREFIX));
            if theirs {
                tables.push(table.clone());
            }
        }
    }
    if let Some(ours) = ours {
        let parsed: toml_edit::DocumentMut = ours.parse()?;
        if let Some(array) = parsed.get("hooks").and_then(|i| i.as_array_of_tables()) {
            for table in array.iter() {
                tables.push(table.clone());
            }
        }
    }
    if tables.is_empty() {
        document.remove("hooks");
    } else {
        document.insert("hooks", toml_edit::Item::ArrayOfTables(tables));
    }
    Ok((document.to_string(), warnings))
}

/// The project hooks of a Vibe `.vibe/hooks.toml`, conforme's prefix taken
/// off their names.
pub fn read_vibe_project(text: &str) -> Result<Vec<NormalizedHook>> {
    Ok(read_vibe(text)?
        .into_iter()
        .map(|h| NormalizedHook {
            name: h
                .name
                .strip_prefix(VIBE_PREFIX)
                .map_or_else(|| h.name.clone(), str::to_string),
            ..h
        })
        .collect())
}

/// The `conforme` group of an Antigravity `hooks.json` for `hooks`: the
/// value to set under that key, and a warning for each hook it cannot
/// express. Other groups are the user's.
pub fn antigravity_group(hooks: &[NormalizedHook], owner: &str) -> (Option<Value>, Vec<String>) {
    let (doc, warnings) = antigravity_json(hooks, owner);
    (
        doc.and_then(|d| d.get(ANTIGRAVITY_GROUP).cloned()),
        warnings,
    )
}

/// The key of conforme's group in an Antigravity `hooks.json`.
pub fn antigravity_group_key() -> &'static str {
    ANTIGRAVITY_GROUP
}

/// The warning for a tool that runs a plugin's hooks under its own protocol.
pub fn payload_warning(owner: &str, tool: &str, blocks: &str) -> String {
    format!(
        "{owner}: its hooks run in {tool} with {tool}'s own JSON on stdin, and \
         {blocks}; check each script handles that before relying on it as a guard"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook(event: &str, matcher: Option<&str>, command: &str) -> NormalizedHook {
        NormalizedHook {
            name: String::new(),
            event: event.to_string(),
            matcher: matcher.map(str::to_string),
            command: command.to_string(),
            timeout: Some(30),
        }
    }

    #[test]
    fn test_claude_hooks_round_trip_with_the_plugin_root() {
        let mut hooks = vec![
            hook("PreToolUse", Some("Bash"), "${PLUGIN_ROOT}/guard.sh"),
            hook("SessionStart", None, "echo hi"),
        ];
        name_hooks(&mut hooks);
        let json = claude_json(&hooks);
        assert_eq!(
            json["PreToolUse"][0]["hooks"][0]["command"],
            "${CLAUDE_PLUGIN_ROOT}/guard.sh"
        );
        let mut back = read_claude(&json);
        back.sort_by(|a, b| a.event.cmp(&b.event));
        hooks.sort_by(|a, b| a.event.cmp(&b.event));
        assert_eq!(back, hooks);
    }

    #[test]
    fn test_vibe_hooks_translate_events_and_tool_names() {
        let mut hooks = vec![
            hook("PreToolUse", Some("Bash"), "./guard.sh"),
            hook("PostToolUse", Some("Edit|Write"), "./fmt.sh"),
            hook("Stop", None, "./done.sh"),
            hook("SessionStart", None, "./start.sh"),
            hook("PreToolUse", Some("mcp__.*"), "./mcp.sh"),
        ];
        name_hooks(&mut hooks);
        let (text, warnings) = vibe_toml(&hooks, "plugin kit");
        let text = text.unwrap();
        assert!(
            text.contains("type = \"pre_tool\"") && text.contains("match = \"bash\""),
            "{text}"
        );
        assert!(
            text.contains("match = \"re:^(edit|write_file)$\""),
            "{text}"
        );
        assert!(text.contains("type = \"post_agent\""), "{text}");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        let back = read_vibe(&text).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(back[1].matcher.as_deref(), Some("Edit|Write"));
        assert_eq!(back[2].event, "Stop");
    }

    #[test]
    fn test_comma_separated_matchers_translate() {
        // "`Edit|Write` or `Edit, Write` match either tool" (Claude Code).
        assert_eq!(
            vibe_match("Edit, Write").as_deref(),
            Some("re:^(edit|write_file)$")
        );
        assert_eq!(
            antigravity_match("Bash,Grep").as_deref(),
            Some("run_command|grep_search")
        );
    }

    #[test]
    fn test_plugin_root_becomes_relative_for_antigravity_and_back() {
        let command = "node ${PLUGIN_ROOT}/scripts/policy.mjs \"${PLUGIN_ROOT}/x.sh\"";
        let relative = plugin_relative(command);
        assert_eq!(relative, "node ./scripts/policy.mjs \"./x.sh\"");
        assert_eq!(plugin_rooted(&relative), command);
        // Only a word that starts with `./` is the plugin root's.
        assert_eq!(plugin_rooted("a../b c/./d"), "a../b c/./d");
        assert_eq!(
            plugin_rooted("./a;./b"),
            "${PLUGIN_ROOT}/a;${PLUGIN_ROOT}/b"
        );
    }

    #[test]
    fn test_codex_hooks_leave_out_events_codex_has_not() {
        let mut hooks = vec![
            hook("PreToolUse", Some("Bash"), "./guard.sh"),
            hook("Notification", None, "./ping.sh"),
            hook("PostToolUseFailure", Some("Bash"), "./fail.sh"),
        ];
        name_hooks(&mut hooks);
        let (json, warnings) = codex_json(&hooks, "project");
        let json = json.unwrap();
        assert_eq!(json["PreToolUse"][0]["hooks"][0]["command"], "./guard.sh");
        assert!(json.get("Notification").is_none(), "{json}");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].contains("Codex has no Notification event"));
        let (none, _) = codex_json(&hooks[1..], "project");
        assert!(none.is_none());
    }

    #[test]
    fn test_copilot_hooks_are_flat_entries_and_read_back() {
        let mut hooks = vec![
            hook("PreToolUse", Some("Bash"), "./guard.sh"),
            hook("SessionStart", None, "./start.sh"),
            hook("Notification", None, "./ping.sh"),
        ];
        name_hooks(&mut hooks);
        let (json, warnings) = copilot_json(&hooks, "project");
        let json = json.unwrap();
        assert_eq!(
            json["PreToolUse"][0],
            json!({"type": "command", "command": "./guard.sh", "matcher": "Bash", "timeoutSec": 30})
        );
        assert!(json["PreToolUse"][0].get("hooks").is_none());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        let mut back = read_copilot(&json);
        back.sort_by(|a, b| a.command.cmp(&b.command));
        let mut expected = hooks[..2].to_vec();
        expected.sort_by(|a, b| a.command.cmp(&b.command));
        assert_eq!(back, expected);

        // Copilot's own names, `bash` and `timeout`, and the matcher groups
        // earlier conforme versions wrote there.
        let theirs = json!({
            "preToolUse": [{"type": "command", "bash": "./mine.sh", "matcher": "bash", "timeout": 5}],
            "agentStop": [{"matcher": "*", "hooks": [{"type": "command", "command": "./done.sh"}]}],
            "errorOccurred": [{"type": "command", "bash": "./err.sh"}]
        });
        let back = read_copilot(&theirs);
        assert_eq!(back.len(), 2, "{back:?}");
        assert!(back.iter().any(|h| h.event == "PreToolUse"
            && h.command == "./mine.sh"
            && h.timeout == Some(5)
            && h.name == "pre-tool-use-1"));
        assert!(back
            .iter()
            .any(|h| h.event == "Stop" && h.command == "./done.sh"));
    }

    #[test]
    fn test_antigravity_hooks_round_trip() {
        let mut hooks = vec![
            hook("PreToolUse", Some("Bash"), "./guard.sh"),
            hook("PostToolUse", Some("Edit|Read"), "./fmt.sh"),
            hook("SessionStart", None, "./start.sh"),
            hook("UserPromptSubmit", None, "./prompt.sh"),
            hook("PreToolUse", Some("TodoWrite"), "./todo.sh"),
        ];
        name_hooks(&mut hooks);
        let (doc, warnings) = antigravity_json(&hooks, "plugin kit");
        let doc = doc.unwrap();
        assert_eq!(doc["conforme"]["PreToolUse"][0]["matcher"], "run_command");
        assert_eq!(
            doc["conforme"]["PostToolUse"][0]["matcher"],
            "replace_file_content|edit_file|view_file|read_file"
        );
        assert_eq!(doc["conforme"]["SessionStart"][0]["command"], "./start.sh");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        let mut back = read_antigravity(&doc);
        back.sort_by(|a, b| a.command.cmp(&b.command));
        let mut expected = hooks[..3].to_vec();
        expected.sort_by(|a, b| a.command.cmp(&b.command));
        assert_eq!(back, expected);
    }
}
