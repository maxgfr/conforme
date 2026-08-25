pub mod amazonq;
pub mod amp;
pub mod claude;
pub mod codex;
pub mod continuedev;
pub mod copilot;
pub mod cursor;
pub mod gemini;
pub mod kiro;
pub mod opencode;
pub mod roocode;
pub mod windsurf;
pub mod zed;

use anyhow::{Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::NormalizedConfig;

/// Report of what was written by an adapter.
pub struct WriteReport {
    pub files_written: Vec<PathBuf>,
    pub files_unchanged: Vec<PathBuf>,
}

/// Declares what features an adapter supports.
#[derive(Default)]
pub struct AdapterCapabilities {
    /// Supports per-rule activation modes (glob, agent-decision, manual).
    pub activation_modes: bool,
    /// Supports skills generation.
    pub skills: bool,
    /// Supports agents generation.
    pub agents: bool,
    /// Supports MCP server config generation.
    pub mcp: bool,
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

    /// Declare what features this adapter supports.
    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities::default()
    }

    /// Directories managed by this adapter (for orphan cleanup).
    /// Files in these directories that are not in the generate() output will be removed.
    fn managed_directories(&self, _project_root: &Path) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Whether a generated path also contains user-owned settings and must not
    /// be deleted wholesale by `remove` or source cleanup during `migrate`.
    fn is_shared_file(&self, _path: &Path) -> bool {
        false
    }

    /// Write normalized config into this tool's format.
    /// Returns a report of what files were written/unchanged.
    /// Default implementation calls generate() then write_if_changed for each file.
    fn write(&self, project_root: &Path, config: &NormalizedConfig) -> Result<WriteReport> {
        let generated = self.generate(project_root, config)?;
        let mut report = WriteReport {
            files_written: Vec::new(),
            files_unchanged: Vec::new(),
        };
        for (path, content) in generated {
            write_if_changed(&path, &content, &mut report)?;
        }
        Ok(report)
    }

    /// Generate expected file contents without writing.
    /// Returns Vec<(path, expected_content)>.
    fn generate(
        &self,
        project_root: &Path,
        config: &NormalizedConfig,
    ) -> Result<Vec<(PathBuf, String)>>;
}

/// Clean orphan files from managed directories.
/// Removes files that exist on disk but are not in the expected file list.
pub fn clean_orphans(
    managed_dirs: &[PathBuf],
    expected_files: &[(PathBuf, String)],
) -> Result<Vec<PathBuf>> {
    let expected_set: std::collections::HashSet<_> =
        expected_files.iter().map(|(p, _)| p.clone()).collect();

    let mut cleaned = Vec::new();
    for dir in managed_dirs {
        if !dir.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && !expected_set.contains(&path) {
                std::fs::remove_file(&path)?;
                cleaned.push(path);
            }
        }
    }
    Ok(cleaned)
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
        Box::new(kiro::KiroAdapter),
        Box::new(amp::AmpAdapter),
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

/// Atomically replace a file after fully writing and syncing a sibling temp file.
/// Use this for mixed-ownership config files where truncation could destroy
/// unrelated user settings.
pub fn write_if_changed_atomic(path: &Path, content: &str, report: &mut WriteReport) -> Result<()> {
    // Persisting directly over a symlink replaces the link itself. Resolve an
    // existing link first so centrally managed/dotfile-backed configs remain
    // linked and their target is updated atomically instead.
    let write_path = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => std::fs::canonicalize(path)
            .with_context(|| format!("failed to resolve symlink {}", path.display()))?,
        Ok(_) => path.to_path_buf(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => path.to_path_buf(),
        Err(error) => return Err(error.into()),
    };

    let parent = write_path
        .parent()
        .context("cannot atomically write a path without a parent directory")?;
    std::fs::create_dir_all(parent)?;

    if write_path.exists() {
        let existing = std::fs::read_to_string(&write_path)?;
        if crate::hash::contents_match(&existing, content) {
            report.files_unchanged.push(path.to_path_buf());
            return Ok(());
        }
    }

    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary file beside {}", path.display()))?;
    temporary
        .write_all(content.as_bytes())
        .with_context(|| format!("failed to write temporary file for {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("failed to sync temporary file for {}", path.display()))?;

    if write_path.exists() {
        let permissions = std::fs::metadata(&write_path)?.permissions();
        temporary.as_file().set_permissions(permissions)?;
    }

    temporary
        .persist(&write_path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to atomically replace {}", write_path.display()))?;
    report.files_written.push(path.to_path_buf());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_atomic_write_preserves_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("target.toml");
        let link = dir.path().join("config.toml");
        std::fs::write(&target, "old").unwrap();
        symlink(&target, &link).unwrap();
        let mut report = WriteReport {
            files_written: Vec::new(),
            files_unchanged: Vec::new(),
        };

        write_if_changed_atomic(&link, "new", &mut report).unwrap();

        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(report.files_written, vec![link]);
    }
}
