use anyhow::{bail, Result};
use notify_debouncer_mini::new_debouncer;
use owo_colors::OwoColorize;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use crate::adapters;
use crate::project_config::ProjectConfig;

/// Run the `watch` command — watch source files and auto-sync on changes.
pub fn run_watch(project_root: &Path, only: Option<&[String]>, verbose: bool) -> Result<()> {
    let project_cfg = ProjectConfig::load(project_root);

    // Determine what paths to watch based on the source
    let watch_paths = get_watch_paths(project_root, &project_cfg)?;

    if watch_paths.is_empty() {
        bail!(
            "No source paths to watch. Configure a source in .conformerc.toml or create AGENTS.md."
        );
    }

    println!("{} Watching for changes...", ">".cyan());
    for path in &watch_paths {
        println!(
            "  {} {}",
            "watching".dimmed(),
            path.strip_prefix(project_root).unwrap_or(path).display()
        );
    }
    println!("  Press Ctrl+C to stop.\n");

    let (tx, rx) = mpsc::channel();

    let mut debouncer = new_debouncer(Duration::from_millis(500), tx)?;

    for path in &watch_paths {
        if path.exists() {
            debouncer
                .watcher()
                .watch(path, notify::RecursiveMode::Recursive)?;
        }
    }

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
                let relevant = events.iter().any(|e| !is_noise(&e.path));

                if relevant {
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

/// Get the paths to watch based on the configured source.
fn get_watch_paths(
    project_root: &Path,
    project_cfg: &ProjectConfig,
) -> Result<Vec<std::path::PathBuf>> {
    let mut paths = Vec::new();

    if let Some(ref source_id) = project_cfg.source {
        let adapters = adapters::all_adapters();
        if let Some(adapter) = adapters.iter().find(|a| a.id() == source_id.as_str()) {
            // Watch the managed directories of the source adapter
            let managed = adapter.managed_directories(project_root);
            paths.extend(managed.into_iter().map(|dir| dir.path));

            // ...and every file it reads back: what it would generate from its
            // own config (CLAUDE.md, GEMINI.md, settings files, …). A skills
            // root is watched as a whole so a new skill is seen too.
            if let Ok(files) = adapter
                .read(project_root)
                .and_then(|config| adapter.generate(project_root, &config))
            {
                for (path, _) in files {
                    let skills_root = path
                        .strip_prefix(project_root)
                        .ok()
                        .and_then(|rel| {
                            rel.ancestors()
                                .find(|a| a.file_name().is_some_and(|n| n == "skills"))
                        })
                        .map(|rel| project_root.join(rel));
                    let watched = skills_root.unwrap_or(path);
                    if watched.exists() && !paths.contains(&watched) {
                        paths.push(watched);
                    }
                }
            }

            // Also watch tool-specific main files
            match source_id.as_str() {
                "claude" => {
                    let claude_md = project_root.join("CLAUDE.md");
                    if claude_md.exists() {
                        paths.push(claude_md);
                    }
                    let mcp = project_root.join(".mcp.json");
                    if mcp.exists() {
                        paths.push(mcp);
                    }
                }
                "cursor" => {
                    let mcp = project_root.join(".cursor").join("mcp.json");
                    if mcp.exists() {
                        paths.push(mcp);
                    }
                }
                "codex" => {
                    // Watch the directory, not the file inode: editors commonly
                    // save by atomically replacing config.toml, and the file may
                    // also be created after watch starts.
                    let codex_dir = project_root.join(".codex");
                    if codex_dir.is_dir() {
                        paths.retain(|p| !p.starts_with(&codex_dir));
                        paths.push(codex_dir);
                    }
                }
                _ => {}
            }
        }
    }

    // Always watch AGENTS.md if it exists
    let agents_md = project_root.join("AGENTS.md");
    if agents_md.exists() && !paths.contains(&agents_md) {
        paths.push(agents_md);
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_codex_source_watches_config_directory() {
        let dir = TempDir::new().unwrap();
        let codex_dir = dir.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();
        let project_config = ProjectConfig {
            source: Some("codex".to_string()),
            ..Default::default()
        };

        let paths = get_watch_paths(dir.path(), &project_config).unwrap();

        // The config directory, and the shared skills root Codex reads.
        assert_eq!(
            paths,
            vec![dir.path().join(".agents").join("skills"), codex_dir]
        );
    }

    #[test]
    fn test_source_files_and_skill_roots_are_watched() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("opencode.json"),
            r#"{"mcp": {"fs": {"type": "local", "command": ["npx"]}}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join(".opencode/skills/deploy")).unwrap();
        std::fs::write(
            root.join(".opencode/skills/deploy/SKILL.md"),
            "---\nname: deploy\ndescription: Deploy\n---\nRun.\n",
        )
        .unwrap();
        let project_config = ProjectConfig {
            source: Some("opencode".to_string()),
            ..Default::default()
        };

        let paths = get_watch_paths(root, &project_config).unwrap();

        assert!(paths.contains(&root.join("opencode.json")), "{paths:?}");
        assert!(paths.contains(&root.join(".opencode/skills")), "{paths:?}");
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
