pub mod amazonq;
pub mod claude;
pub mod codex;
pub mod continuedev;
pub mod copilot;
pub mod cursor;
pub mod gemini;
pub mod opencode;
pub mod roocode;
pub mod windsurf;
pub mod zed;

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::config::NormalizedConfig;

/// Report of what was written by an adapter.
pub struct WriteReport {
    pub files_written: Vec<PathBuf>,
    pub files_unchanged: Vec<PathBuf>,
}

/// Trait for AI tool configuration adapters.
pub trait AiToolAdapter: Send + Sync {
    /// Human-readable tool name (e.g., "Claude Code")
    fn name(&self) -> &str;

    /// Short CLI identifier (e.g., "claude") for --only flag
    fn id(&self) -> &str;

    /// Returns true if this tool's config files/directories exist
    fn detect(&self, project_root: &Path) -> bool;

    /// Read this tool's current config into normalized form
    fn read(&self, project_root: &Path) -> Result<NormalizedConfig>;

    /// Write normalized config into this tool's format.
    /// Returns a report of what files were written/unchanged.
    fn write(&self, project_root: &Path, config: &NormalizedConfig) -> Result<WriteReport>;

    /// Generate expected file contents without writing.
    /// Returns Vec<(path, expected_content)>.
    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>>;
}

/// Get all registered adapters.
pub fn all_adapters() -> Vec<Box<dyn AiToolAdapter>> {
    vec![
        Box::new(claude::ClaudeAdapter),
        Box::new(cursor::CursorAdapter),
        Box::new(windsurf::WindsurfAdapter),
        Box::new(copilot::CopilotAdapter),
        Box::new(codex::CodexAdapter),
        Box::new(opencode::OpenCodeAdapter),
        Box::new(roocode::RooCodeAdapter),
        Box::new(gemini::GeminiAdapter),
        Box::new(continuedev::ContinueDevAdapter),
        Box::new(zed::ZedAdapter),
        Box::new(amazonq::AmazonQAdapter),
    ]
}

/// Write a file only if its content differs from what's already on disk.
pub fn write_if_changed(path: &Path, content: &str, report: &mut WriteReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if path.exists() {
        let existing = std::fs::read_to_string(path)?;
        if crate::hash::contents_match(&existing, content) {
            report.files_unchanged.push(path.to_path_buf());
            return Ok(());
        }
    }

    std::fs::write(path, content)?;
    report.files_written.push(path.to_path_buf());
    Ok(())
}
