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
#[derive(Debug, Clone, Default)]
pub struct NormalizedSkill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub allowed_tools: Vec<String>,
    /// Keep this skill available only through explicit user invocation.
    pub manual_invocation: bool,
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
    name.chars()
        .map(|c| match fold_to_ascii(c) {
            Some(folded) => folded.to_string(),
            None if c.is_alphanumeric() => c.to_lowercase().collect(),
            None => "-".to_string(),
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

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

/// Split a comma-separated glob list, ignoring commas inside `{…}` braces so
/// `src/**/*.{ts,tsx}, docs/**` yields two patterns, not three.
pub fn split_globs(globs: &str) -> Vec<String> {
    let mut patterns = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for c in globs.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
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
