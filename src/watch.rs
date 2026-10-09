use anyhow::{bail, Result};
use notify::{RecursiveMode, Watcher};
use notify_debouncer_mini::new_debouncer;
use owo_colors::OwoColorize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::adapters;
use crate::project_config::ProjectConfig;

/// Run the `watch` command — watch source files and auto-sync on changes.
pub fn run_watch(project_root: &Path, only: Option<&[String]>, verbose: bool) -> Result<()> {
    let project_cfg = ProjectConfig::load(project_root);

    // Determine what paths to watch based on the source
    let relevant = get_watch_paths(project_root, &project_cfg)?;

    if project_cfg.source.is_none() && !project_root.join("AGENTS.md").exists() {
        bail!(
            "No source paths to watch. Configure a source in .conformerc.toml or create AGENTS.md."
        );
    }

    println!("{} Watching for changes...", ">".cyan());
    for path in &relevant {
        println!(
            "  {} {}",
            "watching".dimmed(),
            path.strip_prefix(project_root).unwrap_or(path).display()
        );
    }
    println!("  Press Ctrl+C to stop.\n");

    let (tx, rx) = mpsc::channel();

    let mut debouncer = new_debouncer(Duration::from_millis(500), tx)?;
    let mut watched = HashSet::new();
    add_watches(
        debouncer.watcher(),
        &watch_plan(project_root, &relevant),
        &mut watched,
    );

    // Initial sync
    if let Err(e) = crate::sync::run_sync(
        project_root,
        false,
        only,
        project_cfg.source.as_deref(),
        false,
        verbose,
    ) {
        eprintln!("{} Initial sync failed: {}", "!".red(), e);
    }

    // Watch loop
    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                let relevant_now = get_watch_paths(project_root, &project_cfg)?;
                let changed = events
                    .iter()
                    .any(|e| !is_noise(&e.path) && is_relevant(&e.path, &relevant_now));

                if changed {
                    println!("\n{} Change detected, syncing...", ">".cyan());
                    if let Err(e) = crate::sync::run_sync(
                        project_root,
                        false,
                        only,
                        project_cfg.source.as_deref(),
                        false,
                        verbose,
                    ) {
                        eprintln!("{} Sync failed: {}", "!".red(), e);
                    }
                }
                // A directory or file created since: watch it from now on.
                add_watches(
                    debouncer.watcher(),
                    &watch_plan(project_root, &relevant_now),
                    &mut watched,
                );
            }
            Ok(Err(errors)) => {
                eprintln!("{} Watch error: {}", "!".red(), errors);
            }
            Err(e) => {
                eprintln!("{} Channel error: {}", "!".red(), e);
                break;
            }
        }
    }

    Ok(())
}

fn add_watches(
    watcher: &mut dyn Watcher,
    plan: &[(PathBuf, RecursiveMode)],
    watched: &mut HashSet<(PathBuf, bool)>,
) {
    for (path, mode) in plan {
        let key = (path.clone(), *mode == RecursiveMode::Recursive);
        if watched.contains(&key) {
            continue;
        }
        if watcher.watch(path, *mode).is_ok() {
            watched.insert(key);
        }
    }
}

/// An event concerns the source when it is inside a relevant path, or is a
/// directory on the way to one (`.opencode/` created before `agent/`).
fn is_relevant(event: &Path, relevant: &[PathBuf]) -> bool {
    relevant
        .iter()
        .any(|path| event.starts_with(path) || path.starts_with(event))
}

/// How to watch the relevant paths: an existing directory recursively; a file
/// (editors save by replacing it) or a path that does not exist yet through
/// its nearest existing parent directory, not recursively — a whole `.claude/`
/// or `.github/` tree is never watched just to see one file appear.
fn watch_plan(project_root: &Path, relevant: &[PathBuf]) -> Vec<(PathBuf, RecursiveMode)> {
    let mut plan: Vec<(PathBuf, RecursiveMode)> = Vec::new();
    let mut push = |entry: (PathBuf, RecursiveMode)| {
        if !plan.contains(&entry) {
            plan.push(entry);
        }
    };
    for path in relevant {
        if path.is_dir() {
            push((path.clone(), RecursiveMode::Recursive));
            continue;
        }
        let parent = path
            .ancestors()
            .skip(1)
            .find(|a| a.is_dir() && a.starts_with(project_root))
            .unwrap_or(project_root);
        push((parent.to_path_buf(), RecursiveMode::NonRecursive));
    }
    plan
}

/// Files that change without the configuration changing: Finder metadata and
/// editor swap or backup files. Other dot-files (`.mcp.json`, `.rules`,
/// `.windsurfrules`) are real sources and must trigger a sync.
fn is_noise(path: &Path) -> bool {
    let Some(name) = path.file_name().map(|n| n.to_string_lossy()) else {
        return false;
    };
    name == ".DS_Store"
        || name.starts_with(".#")
        || name.ends_with('~')
        || name.ends_with(".swp")
        || name.ends_with(".swx")
        || name.ends_with(".tmp")
}

/// Every location a source tool's `read()` may load, whether or not it exists
/// yet: a file created later must trigger a sync too.
fn source_locations(id: &str) -> &'static [&'static str] {
    match id {
        "claude" => &[
            "CLAUDE.md",
            ".claude/CLAUDE.md",
            "AGENTS.md",
            ".claude/AGENTS.md",
            ".claude/rules",
            ".claude/skills",
            ".claude/commands",
            ".claude/agents",
            ".mcp.json",
        ],
        "cursor" => &[
            ".cursor/rules",
            ".cursor/skills",
            ".cursor/agents",
            ".cursor/mcp.json",
        ],
        "devin" => &[
            ".devin/rules",
            ".devin/skills",
            ".devin/global_rules.md",
            ".devin/mcp_config.json",
            ".windsurf/rules",
            ".windsurf/skills",
            ".windsurf/global_rules.md",
            ".windsurfrules",
        ],
        "copilot" => &[
            ".github/copilot-instructions.md",
            ".github/instructions",
            ".github/skills",
            ".github/agents",
            ".vscode/mcp.json",
        ],
        "codex" => &["AGENTS.md", ".agents/skills", ".codex"],
        "opencode" => &[
            "AGENTS.md",
            "CLAUDE.md",
            "opencode.json",
            "opencode.jsonc",
            ".opencode/opencode.json",
            ".opencode/opencode.jsonc",
            ".opencode/skills",
            ".opencode/skill",
            ".opencode/agents",
            ".opencode/agent",
        ],
        "zoocode" => &[".roo/rules", ".roo/skills", ".roo/mcp.json", ".roorules"],
        "gemini" => &[
            "GEMINI.md",
            ".gemini/settings.json",
            ".gemini/skills",
            ".gemini/agents",
        ],
        "zed" => &[".rules", ".agents/skills", ".zed/settings.json"],
        "kiro" => &[
            ".kiro/steering",
            ".kiro/skills",
            ".kiro/agents",
            ".kiro/settings/mcp.json",
        ],
        "deepseek" => &["AGENTS.md", "CLAUDE.md", ".dsh/skills", ".agents/skills"],
        "vibe" => &[
            "AGENTS.md",
            ".vibe/skills",
            ".vibe/agents",
            ".vibe/prompts",
            ".vibe/config.toml",
            ".agents/skills",
        ],
        "kilo" => &[
            "AGENTS.md",
            "CLAUDE.md",
            "CONTEXT.md",
            ".kilo/rules",
            ".kilo/skills",
            ".kilo/skill",
            ".kilo/agents",
            ".kilo/agent",
            ".kilo/kilo.json",
            ".kilo/kilo.jsonc",
            ".kilocode/rules",
            ".kilocode/skills",
            ".kilocode/skill",
            ".kilocode/agents",
            ".kilocode/agent",
            ".kilocode/kilo.json",
            ".kilocode/kilo.jsonc",
            "kilo.json",
            "kilo.jsonc",
        ],
        _ => &[],
    }
}

/// The paths whose changes trigger a sync: every location the source reads
/// (see [`source_locations`]), its managed directories, the files it reads
/// in this project (`source_files`: Gemini's `context.fileName` files, …),
/// `.conformerc.toml`, and `AGENTS.md`.
fn get_watch_paths(project_root: &Path, project_cfg: &ProjectConfig) -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut push = |path: PathBuf| {
        if !paths.contains(&path) {
            paths.push(path);
        }
    };

    if let Some(ref source_id) = project_cfg.source {
        let adapters = adapters::all_adapters();
        if let Some(adapter) = adapters.iter().find(|a| a.id() == source_id.as_str()) {
            for location in source_locations(adapter.id()) {
                push(project_root.join(location));
            }
            for dir in adapter.managed_directories(project_root) {
                push(dir.path);
            }
            for file in adapter.source_files(project_root) {
                push(file);
            }
        }
    }

    push(project_root.join("AGENTS.md"));
    push(project_root.join(".conformerc.toml"));
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn config(source: &str) -> ProjectConfig {
        ProjectConfig {
            source: Some(source.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_codex_source_watches_config_directory() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".codex")).unwrap();

        let paths = get_watch_paths(root, &config("codex")).unwrap();

        // The config directory (editors replace config.toml atomically), the
        // shared skills root Codex reads, and AGENTS.md.
        for path in [".codex", ".agents/skills", "AGENTS.md"] {
            assert!(paths.contains(&root.join(path)), "{path}: {paths:?}");
        }
        let plan = watch_plan(root, &paths);
        assert!(plan.contains(&(root.join(".codex"), RecursiveMode::Recursive)));
    }

    /// Every file a source reads is watched, including those that do not
    /// exist when `watch` starts.
    #[test]
    fn test_every_source_read_location_is_watched() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        for (source, path) in [
            ("claude", ".claude/AGENTS.md"),
            ("claude", ".claude/commands"),
            ("gemini", ".gemini/settings.json"),
            ("devin", ".devin/global_rules.md"),
            ("devin", ".windsurfrules"),
            ("opencode", ".opencode/agent"),
            ("opencode", "CLAUDE.md"),
            ("zoocode", ".roorules"),
            ("deepseek", ".agents/skills"),
            ("opencode", ".opencode/opencode.json"),
            ("kilo", ".kilo/rules"),
        ] {
            let paths = get_watch_paths(root, &config(source)).unwrap();
            assert!(
                paths.contains(&root.join(path)),
                "{source} {path}: {paths:?}"
            );
        }
    }

    #[test]
    fn test_gemini_context_files_are_watched() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".gemini")).unwrap();
        std::fs::write(
            root.join(".gemini/settings.json"),
            r#"{"context": {"fileName": "CONTEXT.md"}}"#,
        )
        .unwrap();
        std::fs::write(root.join("CONTEXT.md"), "Instr.\n").unwrap();

        let paths = get_watch_paths(root, &config("gemini")).unwrap();

        assert!(paths.contains(&root.join("CONTEXT.md")), "{paths:?}");
    }

    /// A missing path is watched through its nearest existing parent, never
    /// by watching a whole tree recursively.
    #[test]
    fn test_missing_paths_are_watched_through_their_parent() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".opencode")).unwrap();

        let plan = watch_plan(
            root,
            &[root.join(".opencode/agent"), root.join("AGENTS.md")],
        );

        assert_eq!(
            plan,
            vec![
                (root.join(".opencode"), RecursiveMode::NonRecursive),
                (root.to_path_buf(), RecursiveMode::NonRecursive),
            ]
        );
    }

    #[test]
    fn test_events_outside_the_source_are_ignored() {
        let root = Path::new("/p");
        let relevant = vec![root.join(".claude/rules"), root.join("CLAUDE.md")];
        assert!(is_relevant(&root.join(".claude/rules/ts.md"), &relevant));
        assert!(is_relevant(&root.join(".claude"), &relevant));
        assert!(!is_relevant(&root.join("src/main.rs"), &relevant));
        assert!(!is_relevant(&root.join(".cursor/rules/ts.mdc"), &relevant));
    }

    #[test]
    fn test_dot_files_are_not_noise() {
        assert!(!is_noise(Path::new("/p/.mcp.json")));
        assert!(!is_noise(Path::new("/p/.rules")));
        assert!(is_noise(Path::new("/p/.DS_Store")));
        assert!(is_noise(Path::new("/p/.CLAUDE.md.swp")));
        assert!(is_noise(Path::new("/p/CLAUDE.md~")));
    }
}
