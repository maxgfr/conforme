use serde::Deserialize;
use std::path::Path;

/// Project-level configuration from `.conformerc.toml`.
#[derive(Debug, Deserialize)]
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

impl ProjectConfig {
    /// Load project config from `.conformerc.toml` if it exists.
    /// Returns default config if file doesn't exist.
    pub fn load(project_root: &Path) -> Self {
        let config_path = project_root.join(".conformerc.toml");
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("Warning: failed to parse .conformerc.toml: {e}");
                    }
                },
                Err(e) => {
                    eprintln!("Warning: failed to read .conformerc.toml: {e}");
                }
            }
        }
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_missing_file() {
        let dir = TempDir::new().unwrap();
        let config = ProjectConfig::load(dir.path());
        assert!(config.source.is_none());
        assert!(config.generate_agents_md);
        assert!(config.clean);
    }

    #[test]
    fn test_load_minimal() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".conformerc.toml"), "source = \"claude\"\n").unwrap();
        let config = ProjectConfig::load(dir.path());
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
        let config = ProjectConfig::load(dir.path());
        assert_eq!(config.source.as_deref(), Some("cursor"));
        assert_eq!(config.only, Some(vec!["claude".into(), "copilot".into()]));
        assert_eq!(config.exclude, Some(vec!["zed".into()]));
        assert!(!config.generate_agents_md);
        assert!(!config.clean);
    }
}
