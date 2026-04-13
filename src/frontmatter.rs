use anyhow::{Context, Result};
use gray_matter::engine::YAML;
use gray_matter::{Matter, ParsedEntity};
use std::collections::BTreeMap;

/// Parse YAML frontmatter from a Markdown file.
/// Returns (frontmatter fields, body content).
pub fn parse(content: &str) -> Result<(BTreeMap<String, serde_yaml_ng::Value>, String)> {
    let matter = Matter::<YAML>::new();
    let result: ParsedEntity<BTreeMap<String, serde_yaml_ng::Value>> = matter
        .parse(content)
        .context("failed to parse frontmatter")?;

    let fields = result.data.unwrap_or_default();

    Ok((fields, result.content))
}

/// Serialize YAML frontmatter + body into a Markdown string.
pub fn serialize(fields: &BTreeMap<String, serde_yaml_ng::Value>, body: &str) -> Result<String> {
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
