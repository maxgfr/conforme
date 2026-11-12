use std::collections::BTreeMap;

/// Text as conforme reads it: without a leading byte-order mark, and with
/// CRLF line ends turned into LF, so neither leaks into generated files.
pub fn normalize_text(text: &str) -> std::borrow::Cow<'_, str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.contains('\r') {
        std::borrow::Cow::Owned(text.replace("\r\n", "\n"))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

/// Read a text file the tools' users write (see [`normalize_text`]); errors
/// name the file.
pub fn read_text(path: &std::path::Path) -> anyhow::Result<String> {
    use anyhow::Context;
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(normalize_text(&text).into_owned())
}

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
#[derive(Debug, Clone, Default)]
pub struct NormalizedSkill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub allowed_tools: Vec<String>,
    /// Keep this skill available only through explicit user invocation.
    pub manual_invocation: bool,
    /// Files bundled in the skill folder besides `SKILL.md` (scripts,
    /// references, templates), by path relative to the folder with `/`
    /// separators. Copied into every tool's copy of the skill, so a `SKILL.md`
    /// that points at `scripts/check.sh` finds it everywhere.
    pub files: BTreeMap<String, String>,
}

/// A normalized MCP server definition.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedMcpServer {
    pub name: String,
    pub transport: McpTransport,
    pub env: BTreeMap<String, String>,
}

/// MCP server transport type.
#[derive(Debug, Clone, PartialEq)]
pub enum McpTransport {
    Stdio {
        command: String,
        args: Vec<String>,
    },
    Http {
        url: String,
        headers: BTreeMap<String, String>,
    },
    /// A remote server on the legacy HTTP+SSE transport (MCP 2024-11-05).
    Sse {
        url: String,
        headers: BTreeMap<String, String>,
    },
}

/// A normalized custom agent definition.
#[derive(Debug, Clone, Default)]
pub struct NormalizedAgent {
    pub name: String,
    pub description: String,
    pub content: String,
    pub model: Option<String>,
    pub tools: Vec<String>,
    /// Claude-specific: subagent accent color
    /// (`red`, `blue`, `green`, `yellow`, `purple`, `orange`, `pink`, `cyan`).
    /// Preserved for round-trip fidelity; not mapped to other tools.
    pub color: Option<String>,
    /// Claude-specific: subagent permission mode
    /// (`default`, `acceptEdits`, `plan`, `bypassPermissions`).
    /// Preserved for round-trip fidelity; not mapped to other tools.
    pub permission_mode: Option<String>,
}

/// A plugin bundle: a named, versioned package of rules, skills, agents and
/// MCP servers that a tool loads together (Claude Code
/// `.claude-plugin/plugin.json`, an Agent Plugins `plugin.json`). A tool that
/// cannot hold a bundle gets its content as its own items (see
/// [`crate::plugins::flatten`]).
#[derive(Debug, Clone, Default)]
pub struct NormalizedPlugin {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    /// The author's name.
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub keywords: Vec<String>,
    /// The plugin is installed but switched off (Claude Code `enabledPlugins`
    /// set to `false`, Vibe `disabled_plugins`, Antigravity `disabled`).
    pub disabled: bool,
    /// Instructions the plugin always adds (Antigravity `rules/AGENTS.md`).
    pub instructions: String,
    pub rules: Vec<NormalizedRule>,
    pub skills: Vec<NormalizedSkill>,
    pub agents: Vec<NormalizedAgent>,
    pub mcp_servers: Vec<NormalizedMcpServer>,
    /// Files of the bundle outside its components (scripts, assets), by
    /// `/`-separated path relative to the bundle root.
    pub files: BTreeMap<String, String>,
}

impl NormalizedPlugin {
    /// Whether the plugin holds no component at all.
    pub fn is_empty(&self) -> bool {
        self.instructions.trim().is_empty()
            && self.rules.is_empty()
            && self.skills.is_empty()
            && self.agents.is_empty()
            && self.mcp_servers.is_empty()
            && self.files.is_empty()
    }
}

/// A plugin the project enables from a marketplace, by id
/// `<plugin>@<marketplace>` (Claude Code and Copilot `enabledPlugins`, Codex
/// `[plugins."<id>"]`). Unlike a [`NormalizedPlugin`], its content is not in
/// the project: each tool installs it from the marketplace.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedPluginRef {
    pub plugin: String,
    pub marketplace: String,
    /// `false`: listed but switched off.
    pub enabled: bool,
}

impl NormalizedPluginRef {
    /// The id tools key it by: `<plugin>@<marketplace>`.
    pub fn id(&self) -> String {
        format!("{}@{}", self.plugin, self.marketplace)
    }
}

/// A marketplace the project declares, so its plugins can be enabled
/// (Claude Code and Copilot `extraKnownMarketplaces`).
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedMarketplace {
    pub name: String,
    /// The source object in Claude Code's shape, the one Copilot reads too:
    /// `{"source": "github", "repo": "owner/repo", "ref"?, "path"?}`,
    /// `{"source": "git", "url": …}`, `{"source": "directory", "path": …}`, …
    /// (https://code.claude.com/docs/en/plugins/marketplace-reference.md).
    pub source: serde_json::Value,
}

/// Full normalized configuration: instructions + rules + skills + MCP + agents
/// + plugins.
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
    /// Plugin bundles
    pub plugins: Vec<NormalizedPlugin>,
    /// Plugins enabled from a marketplace
    pub plugin_refs: Vec<NormalizedPluginRef>,
    /// Marketplaces the project declares
    pub marketplaces: Vec<NormalizedMarketplace>,
}

impl NormalizedConfig {
    pub fn new() -> Self {
        Self {
            instructions: String::new(),
            rules: Vec::new(),
            skills: Vec::new(),
            mcp_servers: Vec::new(),
            agents: Vec::new(),
            plugins: Vec::new(),
            plugin_refs: Vec::new(),
            marketplaces: Vec::new(),
        }
    }

    /// Whether the config holds nothing at all to sync.
    pub fn is_empty(&self) -> bool {
        self.instructions.trim().is_empty()
            && self.rules.is_empty()
            && self.skills.is_empty()
            && self.mcp_servers.is_empty()
            && self.agents.is_empty()
            && self.plugins.iter().all(NormalizedPlugin::is_empty)
            && self.plugin_refs.is_empty()
            && self.marketplaces.is_empty()
    }
}

impl Default for NormalizedConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Longest name the strictest tools accept for a skill or agent (Codex, Zed,
/// Kiro and Zoo Code all cap names at 64 characters).
pub const MAX_NAME_LEN: usize = 64;

/// Sanitize a rule name into a filesystem-safe identifier.
/// "TypeScript Conventions" → "typescript-conventions"
///
/// The result is kebab-case ASCII (`^[a-z0-9]+(-[a-z0-9]+)*$`), at most
/// [`MAX_NAME_LEN`] characters: Zoo Code, Zed, Kiro, OpenCode and the DeepSeek
/// Harness skip a skill whose name is anything else. Accented Latin letters
/// are folded to their base letter (`Déployer` → `deployer`); any other
/// non-ASCII character separates words. A name with no ASCII letter or digit
/// sanitizes to an empty string, which `validate` rejects.
pub fn sanitize_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        match fold_to_ascii(c) {
            Some(folded) => out.push_str(folded),
            None if c.is_ascii_alphanumeric() => out.push(c.to_ascii_lowercase()),
            None => out.push('-'),
        }
    }
    let mut name = out
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if name.len() > MAX_NAME_LEN {
        name.truncate(MAX_NAME_LEN);
        name.truncate(name.trim_end_matches('-').len());
    }
    name
}

/// The file name (without extension) of a rule. Rules only become files —
/// no tool validates their names — so, unlike [`sanitize_name`] for skills
/// and agents, letters outside ASCII are kept: a rule named `部署` is still
/// written, as `部署.md`. ASCII names give the same result as `sanitize_name`.
pub fn rule_file_name(name: &str) -> String {
    let ascii = sanitize_name(name);
    if name.is_ascii() {
        return ascii;
    }
    let mut file_name = name
        .chars()
        .map(|c| match fold_to_ascii(c) {
            Some(folded) => folded.to_string(),
            None if c.is_alphanumeric() => c.to_lowercase().collect(),
            None => "-".to_string(),
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if file_name.len() > MAX_RULE_FILE_NAME_BYTES {
        let mut end = MAX_RULE_FILE_NAME_BYTES;
        while !file_name.is_char_boundary(end) {
            end -= 1;
        }
        file_name.truncate(end);
        file_name.truncate(file_name.trim_end_matches('-').len());
    }
    file_name
}

/// Longest [`rule_file_name`] of a name outside ASCII, in bytes: file systems
/// cap a file name at 255 bytes, and a tool adds up to a `000-` prefix and an
/// `.instructions.md` suffix to it.
const MAX_RULE_FILE_NAME_BYTES: usize = 230;

/// The lowercase ASCII spelling of an accented Latin letter, if `c` is one.
fn fold_to_ascii(c: char) -> Option<&'static str> {
    Some(match c.to_lowercase().next().unwrap_or(c) {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'œ' => "oe",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' => "s",
        'ß' => "ss",
        'ţ' | 'ť' | 'ŧ' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'þ' => "th",
        _ => return None,
    })
}

/// Split a comma-separated glob list, ignoring commas inside `{…}` braces and
/// `[…]` character classes, so `src/**/*.{ts,tsx}, docs/[a,b]*` yields two
/// patterns, not four.
pub fn split_globs(globs: &str) -> Vec<String> {
    let mut patterns = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut in_class = false;
    for c in globs.chars() {
        match c {
            '[' => in_class = true,
            ']' => in_class = false,
            '{' if !in_class => depth += 1,
            '}' if !in_class => depth = depth.saturating_sub(1),
            ',' if depth == 0 && !in_class => {
                patterns.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    patterns.push(current);
    patterns
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Read a glob frontmatter field written either as a YAML list or as a
/// comma-separated string (`split_globs`); blank entries are dropped.
pub fn yaml_globs(value: Option<&serde_norway::Value>) -> Vec<String> {
    match value {
        Some(serde_norway::Value::Sequence(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        Some(serde_norway::Value::String(s)) => split_globs(s),
        _ => Vec::new(),
    }
}

/// Expand `{a,b}` alternatives in a glob (`*.{ts,tsx}` → `*.ts`, `*.tsx`).
/// Tools that store several globs in one comma-separated string (Cursor
/// `globs`, Copilot `applyTo`, Devin `globs`) split it on every comma, so a
/// brace group would be cut in half; expanding it keeps the same matches.
pub fn expand_braces(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_string()];
    };
    let mut depth = 0usize;
    let mut close = None;
    let mut splits = Vec::new();
    for (i, c) in pattern[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            ',' if depth == 1 => splits.push(open + i),
            _ => {}
        }
    }
    let Some(close) = close else {
        return vec![pattern.to_string()];
    };
    let (prefix, suffix) = (&pattern[..open], &pattern[close + 1..]);
    let mut bounds = vec![open];
    bounds.extend(splits);
    bounds.push(close);
    let mut out = Vec::new();
    for pair in bounds.windows(2) {
        let alternative = &pattern[pair[0] + 1..pair[1]];
        out.extend(expand_braces(&format!("{prefix}{alternative}{suffix}")));
    }
    out
}

/// Join globs into one comma-separated string, expanding brace groups first
/// (see [`expand_braces`]).
pub fn join_flat_globs(globs: &[String], separator: &str) -> String {
    globs
        .iter()
        .flat_map(|g| expand_braces(g))
        .collect::<Vec<_>>()
        .join(separator)
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

    #[test]
    fn test_sanitize_name_is_kebab_case_ascii() {
        assert_eq!(sanitize_name("Déployer l'App"), "deployer-l-app");
        assert_eq!(sanitize_name("ÉTAPE Œuvre"), "etape-oeuvre");
        assert_eq!(sanitize_name("部署 deploy"), "deploy");
        assert_eq!(sanitize_name("部署"), "");
        let long = sanitize_name(&"word ".repeat(30));
        assert!(long.len() <= MAX_NAME_LEN, "{long}");
        assert!(!long.ends_with('-'));
        for name in ["Déployer l'App", "ÉTAPE", "a_b c", &"x-".repeat(40)] {
            let s = sanitize_name(name);
            assert!(
                s.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{s}"
            );
        }
    }

    #[test]
    fn test_split_globs_keeps_character_classes() {
        assert_eq!(
            split_globs("src/[a,b]*.ts, docs/**, *.{md,[x,y]}"),
            vec!["src/[a,b]*.ts", "docs/**", "*.{md,[x,y]}"]
        );
        // Written back joined by commas, the list reads back the same.
        let globs = vec!["lib/[a,b].rs".to_string(), "**/*.{ts,tsx}".to_string()];
        assert_eq!(split_globs(&globs.join(",")), globs);
    }

    #[test]
    fn test_rule_file_name_fits_a_file_name() {
        // 300 CJK characters are 900 bytes: no file system takes that name.
        let long = "部".repeat(300);
        let file_name = rule_file_name(&long);
        assert!(file_name.len() <= 230, "{}", file_name.len());
        assert!(format!("000-{file_name}.instructions.md").len() <= 255);
        assert!(file_name.chars().all(|c| c == '部'));
        // Cut at a separator: no trailing `-`.
        let name = format!("{} {}", "é".repeat(229), "x".repeat(10));
        assert_eq!(rule_file_name(&name), "e".repeat(229));
        // Shorter names are unchanged.
        assert_eq!(rule_file_name("部署 Rules"), "部署-rules");
    }

    #[test]
    fn test_split_globs_keeps_brace_groups() {
        assert_eq!(
            split_globs("src/**/*.{ts,tsx}, docs/**"),
            vec!["src/**/*.{ts,tsx}", "docs/**"]
        );
        assert_eq!(split_globs("a,,b"), vec!["a", "b"]);
    }

    #[test]
    fn test_expand_braces() {
        assert_eq!(expand_braces("*.{ts,tsx}"), vec!["*.ts", "*.tsx"]);
        assert_eq!(
            expand_braces("{src,lib}/*.{a,b}"),
            vec!["src/*.a", "src/*.b", "lib/*.a", "lib/*.b"]
        );
        assert_eq!(expand_braces("plain/**"), vec!["plain/**"]);
        assert_eq!(expand_braces("broken{a,b"), vec!["broken{a,b"]);
        assert_eq!(
            join_flat_globs(&["src/*.{ts,tsx}".to_string(), "x".to_string()], ","),
            "src/*.ts,src/*.tsx,x"
        );
    }
}
