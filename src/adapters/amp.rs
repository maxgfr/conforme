use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::adapters::AiToolAdapter;
use crate::config::NormalizedConfig;

/// Amp (Sourcegraph) adapter.
/// Reads AGENTS.md natively as primary, falls back to AGENT.md or CLAUDE.md.
/// Global config at ~/.config/amp/AGENTS.md.
/// Settings at .amp/settings.json.
pub struct AmpAdapter;

impl AiToolAdapter for AmpAdapter {
    fn name(&self) -> &str {
        "Amp"
    }

    fn id(&self) -> &str {
        "amp"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join(".amp").is_dir()
    }

    fn read(&self, project_root: &Path) -> Result<NormalizedConfig> {
        // Amp reads AGENTS.md natively
        let agents_md = project_root.join("AGENTS.md");
        let instructions = if agents_md.exists() {
            std::fs::read_to_string(&agents_md)?.trim().to_string()
        } else {
            String::new()
        };
        Ok(NormalizedConfig {
            instructions,
            rules: Vec::new(),
            ..Default::default()
        })
    }

    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>> {
        // Amp reads AGENTS.md natively — no need to re-generate it
        // since AGENTS.md is already our source of truth.
        let _ = project_root;
        let _ = config;
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NormalizedConfig;
    use std::path::Path;

    fn make_adapter() -> AmpAdapter {
        AmpAdapter
    }

    #[test]
    fn test_generate_no_files() {
        let adapter = make_adapter();
        let config = NormalizedConfig {
            instructions: "General instructions.".to_string(),
            rules: vec![],
            ..Default::default()
        };
        let files = adapter.generate(Path::new("/tmp/test"), &config).unwrap();
        // Amp reads AGENTS.md natively — no files generated
        assert!(files.is_empty());
    }
}
