use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::Path;

/// Project-level configuration from `.conformerc.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    /// Source tool ID (e.g., "claude", "cursor"). If unset, falls back to AGENTS.md.
    pub source: Option<String>,
    /// Only sync to these tools (overridden by CLI --only).
    pub only: Option<Vec<String>>,
    /// Exclude these tools from sync.
    pub exclude: Option<Vec<String>>,
    /// Generate AGENTS.md as output when source is not AGENTS.md.
    #[serde(default = "default_true")]
    pub generate_agents_md: bool,
    /// Auto-clean orphaned files after sync.
    #[serde(default = "default_true")]
    pub clean: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            source: None,
            only: None,
            exclude: None,
            generate_agents_md: true,
            clean: true,
        }
    }
}

/// Tool ids conforme renamed, as `(old, new, version)`. An old id in
/// `.conformerc.toml` or on the command line is an error rather than an
/// unknown name to skip: an ignored `exclude = ["windsurf"]` would sync, and
/// clean, the very tool the user meant to leave alone.
const RENAMED_IDS: &[(&str, &str, &str)] = &[("windsurf", "devin", "4.0.0")];

/// Refuse a renamed tool id.
pub fn check_renamed_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Result<()> {
    for id in ids {
        if let Some((old, new, version)) = RENAMED_IDS.iter().find(|(old, ..)| *old == id) {
            bail!(
                "the `{old}` tool id was renamed `{new}` in conforme {version}; \
                 replace it in .conformerc.toml (source, only, exclude) and on the command line"
            );
        }
    }
    Ok(())
}

impl ProjectConfig {
    /// Load project config from `.conformerc.toml` if it exists, and the
    /// default config when it does not. A file that does not parse, an
    /// unknown key (a misspelt `exlude`) or a tool id conforme does not know
    /// is an error: falling back to the defaults would sync, and clean, every
    /// tool.
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".conformerc.toml");
        if !config_path.exists() {
            return Ok(Self::default());
        }
        let content = crate::config::read_text(&config_path)?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("invalid {}", config_path.display()))?;
        config.check_ids()?;
        Ok(config)
    }

    fn check_ids(&self) -> Result<()> {
        let ids: Vec<&str> = self
            .source
            .iter()
            .chain(self.only.iter().flatten())
            .chain(self.exclude.iter().flatten())
            .map(String::as_str)
            .collect();
        check_renamed_ids(ids.iter().copied())?;
        let adapters = crate::adapters::all_adapters();
        let known: Vec<&str> = adapters.iter().map(|a| a.id()).collect();
        if let Some(unknown) = ids.iter().find(|id| !known.contains(id)) {
            bail!(
                "unknown tool id `{unknown}` in .conformerc.toml (source, only, exclude). \
                 Known tools: {}",
                known.join(", ")
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_missing_file() {
        let dir = TempDir::new().unwrap();
        let config = ProjectConfig::load(dir.path()).unwrap();
        assert!(config.source.is_none());
        assert!(config.generate_agents_md);
        assert!(config.clean);
    }

    #[test]
    fn test_load_minimal() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".conformerc.toml"), "source = \"claude\"\n").unwrap();
        let config = ProjectConfig::load(dir.path()).unwrap();
        assert_eq!(config.source.as_deref(), Some("claude"));
        assert!(config.generate_agents_md);
        assert!(config.clean);
    }

    #[test]
    fn test_load_full() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join(".conformerc.toml"),
            r#"
source = "cursor"
only = ["claude", "copilot"]
exclude = ["zed"]
generate_agents_md = false
clean = false
"#,
        )
        .unwrap();
        let config = ProjectConfig::load(dir.path()).unwrap();
        assert_eq!(config.source.as_deref(), Some("cursor"));
        assert_eq!(config.only, Some(vec!["claude".into(), "copilot".into()]));
        assert_eq!(config.exclude, Some(vec!["zed".into()]));
        assert!(!config.generate_agents_md);
        assert!(!config.clean);
    }

    #[test]
    fn test_project_config_unknown_key_is_an_error() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".conformerc.toml"), "exlude = [\"zed\"]\n").unwrap();
        let error = format!("{:#}", ProjectConfig::load(dir.path()).unwrap_err());
        assert!(error.contains("exlude"), "{error}");
    }

    #[test]
    fn test_project_config_parse_error_is_an_error() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".conformerc.toml"), "source = \n").unwrap();
        assert!(ProjectConfig::load(dir.path()).is_err());
    }

    #[test]
    fn test_project_config_unknown_or_renamed_id_is_an_error() {
        let dir = TempDir::new().unwrap();
        for (text, needle) in [
            ("only = [\"cursr\"]\n", "cursr"),
            ("exclude = [\"windsurf\"]\n", "renamed"),
            ("source = \"nope\"\n", "nope"),
        ] {
            std::fs::write(dir.path().join(".conformerc.toml"), text).unwrap();
            let error = format!("{:#}", ProjectConfig::load(dir.path()).unwrap_err());
            assert!(error.contains(needle), "{error}");
        }
    }
}
