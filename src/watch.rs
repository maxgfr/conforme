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
                let relevant = events.iter().any(|e| {
                    let path = &e.path;
                    // Skip hidden files like .DS_Store
                    !path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                });

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
            paths.extend(managed);

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
                _ => {}
            }
        }
    }

    // Always watch AGENTS.md if it exists
    let agents_md = project_root.join("AGENTS.md");
    if agents_md.exists() {
        paths.push(agents_md);
    }

    Ok(paths)
}
