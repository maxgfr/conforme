use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

type Fields = BTreeMap<String, serde_yaml_ng::Value>;

/// Split a Markdown file into its `---` frontmatter block and its body. A
/// leading BOM is skipped and CRLF line ends are accepted; without a closing
/// `---` line the whole file is body.
fn split(content: &str) -> Option<(&str, &str)> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let (first, mut rest) = content.split_once('\n')?;
    if first.trim_end() != "---" {
        return None;
    }
    let yaml_start = rest;
    let mut yaml_len = 0;
    loop {
        let (line, after) = match rest.split_once('\n') {
            Some((line, after)) => (line, after),
            None => (rest, ""),
        };
        if line.trim_end() == "---" {
            return Some((&yaml_start[..yaml_len], after));
        }
        if after.is_empty() && !rest.contains('\n') {
            return None;
        }
        yaml_len += line.len() + 1;
        rest = after;
    }
}

/// Parse YAML frontmatter from a Markdown file.
/// Returns (frontmatter fields, body content).
///
/// Tools accept frontmatter that is not strict YAML (Cursor's own
/// `globs: **/*.ts`, `description: Use when: x`): when the block does not
/// parse as YAML, each top-level `key: value` entry is read on its own, and a
/// value that is not YAML is kept as a string.
pub fn parse(content: &str) -> Result<(Fields, String)> {
    let Some((yaml, body)) = split(content) else {
        let content = content.strip_prefix('\u{feff}').unwrap_or(content);
        return Ok((Fields::new(), content.to_string()));
    };
    let yaml = yaml.replace("\r\n", "\n");
    let fields = if yaml.trim().is_empty() {
        Fields::new()
    } else {
        match serde_yaml_ng::from_str::<Fields>(&yaml) {
            Ok(fields) => fields,
            Err(_) => parse_lenient(&yaml)?,
        }
    };
    Ok((fields, body.to_string()))
}

/// Whether the frontmatter block, if any, is strict YAML: what a tool that
/// skips a file with a broken block (Antigravity) loads.
pub fn is_strict_yaml(content: &str) -> bool {
    split(content).is_none_or(|(yaml, _)| {
        let yaml = yaml.replace("\r\n", "\n");
        yaml.trim().is_empty() || serde_yaml_ng::from_str::<Fields>(&yaml).is_ok()
    })
}

/// Read a Markdown file (see [`crate::config::read_text`]) and parse its
/// frontmatter; errors name the file.
pub fn read(path: &Path) -> Result<(Fields, String)> {
    let content = crate::config::read_text(path)?;
    parse(&content)
        .with_context(|| format!("failed to parse the frontmatter of {}", path.display()))
}

/// The line-by-line reading of a frontmatter block that is not YAML: each
/// top-level `key:` entry with the indented or `- item` lines under it.
fn parse_lenient(yaml: &str) -> Result<Fields> {
    let mut entries: Vec<(String, Vec<&str>)> = Vec::new();
    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indented = line.starts_with([' ', '\t']) || trimmed.starts_with("- ") || trimmed == "-";
        if indented {
            match entries.last_mut() {
                Some((_, lines)) => lines.push(line),
                None => bail!("frontmatter line `{trimmed}` belongs to no key"),
            }
            continue;
        }
        match line.split_once(':') {
            Some((key, _)) if !key.trim().is_empty() && !key.contains(['[', '{', '"', '\'']) => {
                entries.push((key.trim().to_string(), vec![line]));
            }
            _ => bail!("frontmatter line `{trimmed}` is not a `key: value` entry"),
        }
    }
    let mut fields = Fields::new();
    for (key, lines) in entries {
        let block = lines.join("\n");
        if let Ok(parsed) = serde_yaml_ng::from_str::<Fields>(&block) {
            fields.extend(parsed);
            continue;
        }
        let inline = lines[0]
            .split_once(':')
            .map(|(_, value)| value.trim())
            .unwrap_or("");
        let value = if !inline.is_empty() {
            serde_yaml_ng::Value::String(unquote(inline).to_string())
        } else {
            let items: Vec<serde_yaml_ng::Value> = lines[1..]
                .iter()
                .filter_map(|l| l.trim().strip_prefix('-'))
                .map(|item| serde_yaml_ng::Value::String(unquote(item.trim()).to_string()))
                .collect();
            if items.is_empty() {
                continue;
            }
            serde_yaml_ng::Value::Sequence(items)
        };
        fields.insert(key, value);
    }
    Ok(fields)
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

/// Serialize YAML frontmatter + body into a Markdown string.
pub fn serialize(fields: &Fields, body: &str) -> Result<String> {
    if fields.is_empty() {
        return Ok(body.to_string());
    }

    let yaml = serde_yaml_ng::to_string(fields).context("failed to serialize frontmatter")?;
    // serde_yaml_ng adds a trailing newline, trim it
    let yaml = yaml.trim_end();

    Ok(format!("---\n{}\n---\n\n{}", yaml, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_with_frontmatter() {
        let input = "---\nalwaysApply: true\ndescription: test\n---\n\nBody content here.";
        let (fields, body) = parse(input).unwrap();
        assert_eq!(
            fields.get("alwaysApply"),
            Some(&serde_yaml_ng::Value::Bool(true))
        );
        assert!(body.contains("Body content here."));
    }

    #[test]
    fn test_parse_without_frontmatter() {
        let input = "Just plain markdown content.";
        let (fields, body) = parse(input).unwrap();
        assert!(fields.is_empty());
        assert!(body.contains("Just plain markdown"));
    }

    #[test]
    fn test_frontmatter_short_files_keep_their_body() {
        for input in ["Rx\n", "R", "", "---", "--\n"] {
            let (fields, body) = parse(input).unwrap();
            assert!(fields.is_empty(), "{input:?}");
            assert_eq!(body, input, "{input:?}");
        }
    }

    #[test]
    fn test_frontmatter_bom_and_crlf() {
        let (fields, body) = parse("\u{feff}---\r\nalwaysApply: true\r\n---\r\nBody\r\n").unwrap();
        assert_eq!(
            fields.get("alwaysApply"),
            Some(&serde_yaml_ng::Value::Bool(true))
        );
        assert_eq!(body.trim(), "Body");
    }

    #[test]
    fn test_frontmatter_lenient_values() {
        let input = "---\ndescription: Use when: editing TS\nglobs: **/*.ts\nalwaysApply: false\ntags:\n  - a\n  - *b\n---\nBody\n";
        let (fields, body) = parse(input).unwrap();
        let text = |k: &str| fields.get(k).and_then(|v| v.as_str()).map(str::to_string);
        assert_eq!(text("description").as_deref(), Some("Use when: editing TS"));
        assert_eq!(text("globs").as_deref(), Some("**/*.ts"));
        assert_eq!(
            fields.get("alwaysApply"),
            Some(&serde_yaml_ng::Value::Bool(false))
        );
        assert_eq!(
            fields.get("tags"),
            Some(&serde_yaml_ng::Value::Sequence(vec![
                "a".into(),
                "*b".into()
            ]))
        );
        assert_eq!(body, "Body\n");
    }

    #[test]
    fn test_frontmatter_unreadable_block_is_an_error() {
        assert!(parse("---\nalwaysApply: true\n[[[ not yaml\n---\nx\n").is_err());
    }

    #[test]
    fn test_serialize_roundtrip() {
        let mut fields = BTreeMap::new();
        fields.insert("alwaysApply".to_string(), serde_yaml_ng::Value::Bool(true));
        let body = "Rule content here.";
        let output = serialize(&fields, body).unwrap();
        assert!(output.starts_with("---\n"));
        assert!(output.contains("alwaysApply: true"));
        assert!(output.contains("Rule content here."));
    }

    #[test]
    fn test_serialize_empty_fields() {
        let fields = BTreeMap::new();
        let body = "Just content.";
        let output = serialize(&fields, body).unwrap();
        assert_eq!(output, "Just content.");
    }
}
