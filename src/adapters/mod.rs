pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod deepseek;
pub mod devin;
pub mod gemini;
pub mod kilo;
pub mod kiro;
pub mod opencode;
pub mod vibe;
pub mod zed;
pub mod zoocode;

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

    /// Directories managed by this adapter (for orphan cleanup, `watch`, and
    /// `migrate`). During `sync`, top-level files in these directories that
    /// carry the directory's `orphan_suffix` and are not in the generate()
    /// output are removed.
    fn managed_directories(&self, _project_root: &Path) -> Vec<ManagedDir> {
        Vec::new()
    }

    /// Whether a generated path also contains user-owned settings and must not
    /// be deleted wholesale by `remove` or source cleanup during `migrate`.
    fn is_shared_file(&self, _path: &Path) -> bool {
        false
    }

    /// Whether the tool reads `AGENTS.md` itself as its instruction file in
    /// this project: always for Codex, OpenCode, DeepSeek Harness, Mistral Vibe and Kilo Code;
    /// Claude Code when the project has no `CLAUDE.md`; Gemini CLI when
    /// `context.fileName` names it. When such a tool is the source,
    /// `AGENTS.md` *is* its config: sync never regenerates it, and
    /// `gitignore install` never ignores it.
    fn reads_agents_md(&self, _project_root: &Path) -> bool {
        false
    }

    /// The files (and fallback directories) `read()` loads in this project
    /// outside the tool's own managed directories: an `AGENTS.md` or
    /// `CLAUDE.md` it reads natively or as a fallback, Gemini's context files,
    /// Devin's `global_rules.md`, DeepSeek's `.agents/skills` fallback, …
    /// Only paths that exist and that `read()` actually uses are listed.
    /// When the tool is the source these are its config: no target writes
    /// them, `remove` and `migrate` never delete them, and `gitignore
    /// install` never ignores them.
    fn source_files(&self, _project_root: &Path) -> Vec<PathBuf> {
        Vec::new()
    }

    /// What the tool will not load as written, for `sync` and `migrate` to
    /// print: a setting it cannot express, or a file in the project it refuses.
    fn warnings(&self, _project_root: &Path, _config: &NormalizedConfig) -> Vec<String> {
        Vec::new()
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

/// The first of `candidates` (relative to `project_root`) that is a file: the
/// one a tool reading "AGENTS.md, else CLAUDE.md, …" actually loads.
pub fn first_existing_file(project_root: &Path, candidates: &[&str]) -> Vec<PathBuf> {
    candidates
        .iter()
        .map(|name| project_root.join(name))
        .find(|path| path.is_file())
        .into_iter()
        .collect()
}

/// A directory an adapter writes into.
pub struct ManagedDir {
    pub path: PathBuf,
    /// File-name suffix of the files conforme writes directly in `path`
    /// (`.md`, `.mdc`, `.agent.md`, …). Orphan cleanup only deletes top-level
    /// files with this suffix, so files the tool also accepts but conforme
    /// never writes (Kiro `.json` agents, dsh flat `<name>.md` skills,
    /// hand-written Copilot `.md` agents) survive. `None` means conforme only
    /// writes sub-directories there (skills), so no top-level file is ever
    /// one of its orphans.
    pub orphan_suffix: Option<&'static str>,
    /// For a legacy directory the tool still reads next to the current one
    /// (Devin's `.windsurf/rules/` and `.windsurf/skills/`): the directory
    /// conforme now writes to. Only a legacy copy of something generated
    /// there under the same name is removed — the tool would otherwise load
    /// it twice; anything else in the legacy directory is the user's.
    pub superseded_by: Option<PathBuf>,
    /// A file with the orphan suffix that this predicate accepts is the
    /// user's, not an orphan (a README beside Claude Code agents, a Gemini
    /// `_draft.md` agent).
    pub keep: Option<fn(&Path) -> bool>,
}

impl ManagedDir {
    /// A directory holding conforme-generated files ending in `suffix`.
    pub fn files(path: PathBuf, suffix: &'static str) -> Self {
        Self {
            path,
            orphan_suffix: Some(suffix),
            superseded_by: None,
            keep: None,
        }
    }

    /// Like [`ManagedDir::files`], but a file `keep` accepts is never treated
    /// as an orphan.
    pub fn files_except(path: PathBuf, suffix: &'static str, keep: fn(&Path) -> bool) -> Self {
        Self {
            keep: Some(keep),
            ..Self::files(path, suffix)
        }
    }

    /// A directory where conforme only writes sub-directories (skills).
    pub fn subdirs(path: PathBuf) -> Self {
        Self {
            path,
            orphan_suffix: None,
            superseded_by: None,
            keep: None,
        }
    }

    /// A legacy directory of `suffix` files whose files conforme now writes
    /// to `current` (see [`ManagedDir::superseded_by`]).
    pub fn legacy_files(path: PathBuf, current: PathBuf, suffix: &'static str) -> Self {
        Self {
            superseded_by: Some(current),
            ..Self::files(path, suffix)
        }
    }

    /// A legacy skills directory whose skills conforme now writes to
    /// `current` (see [`ManagedDir::superseded_by`]).
    pub fn legacy_skills(path: PathBuf, current: PathBuf) -> Self {
        Self {
            superseded_by: Some(current),
            ..Self::subdirs(path)
        }
    }
}

/// The orphans of the managed directories, without removing anything: the
/// top-level files that carry a directory's orphan suffix and that the
/// expected file list lacks; inside a skill folder whose `SKILL.md` is
/// expected, the bundled files the source no longer has; and every file of a
/// skill folder conforme generated (it holds [`crate::skills::SKILL_MARKER`])
/// whose skill left the source. A skill folder without the marker is the
/// user's and never an orphan. `check` reports these, `sync` removes them
/// through [`clean_orphans`].
pub fn find_orphans(
    managed_dirs: &[ManagedDir],
    expected_files: &[(PathBuf, String)],
) -> Result<Vec<PathBuf>> {
    let expected_set: std::collections::HashSet<_> =
        expected_files.iter().map(|(p, _)| p.clone()).collect();

    let mut orphans = Vec::new();
    for dir in managed_dirs {
        if !dir.path.is_dir() || dir.superseded_by.is_some() {
            continue;
        }
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir.path)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        paths.sort();
        let Some(suffix) = dir.orphan_suffix else {
            for folder in paths.iter().filter(|p| p.is_dir()) {
                orphans.extend(crate::skills::stale_bundled_files(folder, &expected_set)?);
                orphans.extend(crate::skills::stale_skill_folder_files(
                    folder,
                    &expected_set,
                )?);
            }
            continue;
        };
        for path in paths {
            let generated_kind = path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(suffix));
            if generated_kind
                && path.is_file()
                && !expected_set.contains(&path)
                && !dir.keep.is_some_and(|keep| keep(&path))
            {
                orphans.push(path);
            }
        }
    }
    Ok(orphans)
}

/// Clean orphan files from managed directories: every file
/// [`find_orphans`] lists, plus legacy duplicates of generated files and
/// skills (see [`ManagedDir::superseded_by`]).
pub fn clean_orphans(
    managed_dirs: &[ManagedDir],
    expected_files: &[(PathBuf, String)],
) -> Result<Vec<PathBuf>> {
    let expected_set: std::collections::HashSet<_> =
        expected_files.iter().map(|(p, _)| p.clone()).collect();

    let mut cleaned = Vec::new();
    for dir in managed_dirs {
        let Some(current) = dir.superseded_by.as_ref().filter(|_| dir.path.is_dir()) else {
            continue;
        };
        if dir.path == *current {
            continue;
        }
        cleaned.extend(match dir.orphan_suffix {
            Some(suffix) => clean_superseded_files(&dir.path, current, suffix, &expected_set)?,
            None => clean_superseded_skills(&dir.path, current, &expected_set)?,
        });
    }
    for path in find_orphans(managed_dirs, expected_files)? {
        std::fs::remove_file(&path)?;
        remove_emptied_dirs(&path, managed_dirs);
        cleaned.push(path);
    }
    Ok(cleaned)
}

/// Remove the directories a deleted orphan leaves empty, up to (not
/// including) the managed directory that holds it: a stale skill's folder
/// goes with its last file.
fn remove_emptied_dirs(removed: &Path, managed_dirs: &[ManagedDir]) {
    let Some(root) = managed_dirs
        .iter()
        .map(|d| d.path.as_path())
        .find(|root| removed.starts_with(root))
    else {
        return;
    };
    for dir in removed.ancestors().skip(1) {
        if dir == root || !dir.starts_with(root) {
            break;
        }
        if std::fs::remove_dir(dir).is_err() {
            break;
        }
    }
}

/// Remove `<legacy>/<file>` for every `<current>/<file>` that is generated.
fn clean_superseded_files(
    legacy: &Path,
    current: &Path,
    suffix: &str,
    expected: &std::collections::HashSet<PathBuf>,
) -> Result<Vec<PathBuf>> {
    let mut cleaned = Vec::new();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(legacy)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    for path in paths {
        let Some(name) = path.file_name() else {
            continue;
        };
        if name.to_string_lossy().ends_with(suffix) && expected.contains(&current.join(name)) {
            std::fs::remove_file(&path)?;
            cleaned.push(path);
        }
    }
    Ok(cleaned)
}

/// Remove `<legacy>/<name>/` for every skill generated at
/// `<current>/<name>/SKILL.md`, when it holds nothing but a `SKILL.md` (and
/// the Codex policy sidecar conforme writes beside it). A folder that also
/// bundles scripts or references is left alone, since removing only its
/// `SKILL.md` would strand them.
fn clean_superseded_skills(
    legacy: &Path,
    current: &Path,
    expected: &std::collections::HashSet<PathBuf>,
) -> Result<Vec<PathBuf>> {
    let mut cleaned = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(legacy)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    entries.sort();
    for skill_dir in entries {
        let Some(name) = skill_dir.file_name() else {
            continue;
        };
        if !expected.contains(&current.join(name).join("SKILL.md")) {
            continue;
        }
        let skill_md = skill_dir.join("SKILL.md");
        let sidecar = skill_dir.join("agents").join("openai.yaml");
        let marker = skill_dir.join(crate::skills::SKILL_MARKER);
        let only_skill = std::fs::read_dir(&skill_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .all(|p| {
                p == skill_md
                    || p == marker
                    || (p == skill_dir.join("agents")
                        && std::fs::read_dir(&p)
                            .map(|mut it| it.all(|e| e.is_ok_and(|e| e.path() == sidecar)))
                            .unwrap_or(false))
            });
        if !only_skill {
            continue;
        }
        for file in [&skill_md, &sidecar, &marker] {
            if file.is_file() {
                std::fs::remove_file(file)?;
                cleaned.push(file.clone());
            }
        }
        for dir in [skill_dir.join("agents"), skill_dir.clone()] {
            if dir.is_dir() && std::fs::read_dir(&dir)?.next().is_none() {
                std::fs::remove_dir(&dir)?;
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
        Box::new(devin::DevinAdapter),
        Box::new(copilot::CopilotAdapter),
        Box::new(codex::CodexAdapter),
        Box::new(opencode::OpenCodeAdapter),
        Box::new(zoocode::ZooCodeAdapter),
        Box::new(gemini::GeminiAdapter),
        Box::new(zed::ZedAdapter),
        Box::new(kiro::KiroAdapter),
        Box::new(deepseek::DeepSeekAdapter),
        Box::new(vibe::VibeAdapter),
        Box::new(kilo::KiloAdapter),
        Box::new(antigravity::AntigravityAdapter),
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

/// Collect rule files with the given extension from `dir`, recursing into
/// subdirectories.
///
/// Claude Code, Cursor and Zoo Code all document that their rules directory is
/// scanned recursively, so a project may organise rules under `frontend/`,
/// `backend/`, … Reading only the top level silently dropped those rules when
/// such a tool was used as the sync source.
///
/// Results are sorted by file name first (case-insensitively, which is what Roo
/// Code documents and what keeps `00-`/`01-` numeric prefixes meaningful), then
/// by full path so that two files sharing a base name stay in a stable order.
pub fn collect_rule_files(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_rule_files_into(dir, extension, &mut files)?;
    files.sort_by(|a, b| {
        let key = |p: &Path| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        };
        key(a).cmp(&key(b)).then_with(|| a.cmp(b))
    });
    Ok(files)
}

fn collect_rule_files_into(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    // Deterministic descent order; the caller re-sorts the flattened result.
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rule_files_into(&path, extension, out)?;
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
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
