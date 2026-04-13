use anyhow::{bail, Context, Result};
use owo_colors::OwoColorize;
use std::path::Path;

use crate::adapters::{self, AiToolAdapter};
use crate::detect;
use crate::markdown;

/// Run the `init` command.
pub fn run_init(project_root: &Path, force: bool, verbose: bool) -> Result<()> {
    let agents_md = project_root.join("AGENTS.md");

    if agents_md.exists() && !force {
        println!(
            "{} AGENTS.md already exists. Use {} to overwrite.",
            "!".yellow(),
            "--force".bold()
        );
    } else {
        // Try to import from an existing tool config
        let adapters = adapters::all_adapters();
        let mut imported = false;

        if !force {
            for adapter in &adapters {
                if adapter.detect(project_root) {
                    if verbose {
                        println!("  Importing from {}...", adapter.name());
                    }
                    match adapter.read(project_root) {
                        Ok(config)
                            if !config.instructions.is_empty() || !config.rules.is_empty() =>
                        {
                            // Write imported config as AGENTS.md
                            let content = markdown::export_as_agents_md(&config);
                            std::fs::write(&agents_md, content)?;
                            println!(
                                "{} Imported existing config from {} into AGENTS.md",
                                "+".green(),
                                adapter.name().bold()
                            );
                            imported = true;
                            break;
                        }
                        _ => continue,
                    }
                }
            }
        }

        if !imported {
            let template = markdown::template_agents_md();
            std::fs::write(&agents_md, template)?;
            println!("{} Created AGENTS.md template", "+".green());
        }
    }

    // Now sync to all detected tools
    run_sync(project_root, false, None, verbose)
}

/// Run the `sync` command.
pub fn run_sync(
    project_root: &Path,
    dry_run: bool,
    only: Option<&[String]>,
    verbose: bool,
) -> Result<()> {
    let agents_md = project_root.join("AGENTS.md");
    if !agents_md.exists() {
        bail!(
            "No AGENTS.md found in {}. Run {} first.",
            project_root.display(),
            "conforme init".bold()
        );
    }

    let content = std::fs::read_to_string(&agents_md).context("failed to read AGENTS.md")?;
    let config = markdown::parse_agents_md(&content)?;

    if verbose {
        println!(
            "  Parsed AGENTS.md: {} instructions chars, {} rules",
            config.instructions.len(),
            config.rules.len()
        );
    }

    let adapters = adapters::all_adapters();
    let mut any_written = false;

    for adapter in &adapters {
        // Filter by --only
        if let Some(only_list) = only {
            if !only_list.iter().any(|o| o == adapter.id()) {
                continue;
            }
        }

        if !adapter.detect(project_root) {
            if verbose {
                println!(
                    "  {} {} (not detected, skipping)",
                    "-".dimmed(),
                    adapter.name().dimmed()
                );
            }
            continue;
        }

        if dry_run {
            let generated = adapter.generate(project_root, &config)?;
            println!("{} {} (dry-run):", ">".cyan(), adapter.name().bold());
            for (path, expected) in &generated {
                let status = if path.exists() {
                    let existing = std::fs::read_to_string(path)?;
                    if crate::hash::contents_match(&existing, expected) {
                        "unchanged".dimmed().to_string()
                    } else {
                        "would update".yellow().to_string()
                    }
                } else {
                    "would create".green().to_string()
                };
                println!(
                    "    {} {}",
                    status,
                    path.strip_prefix(project_root).unwrap_or(path).display()
                );
            }
        } else {
            let report = adapter.write(project_root, &config)?;
            if !report.files_written.is_empty() {
                any_written = true;
                println!("{} {}:", ">".green(), adapter.name().bold());
                for path in &report.files_written {
                    println!(
                        "    {} {}",
                        "wrote".green(),
                        path.strip_prefix(project_root).unwrap_or(path).display()
                    );
                }
            }
            if verbose {
                for path in &report.files_unchanged {
                    println!(
                        "    {} {}",
                        "unchanged".dimmed(),
                        path.strip_prefix(project_root).unwrap_or(path).display()
                    );
                }
            }
        }
    }

    if !dry_run && !any_written {
        println!("{} All configs already in sync.", "=".green());
    }

    Ok(())
}

/// Run the `check` command.
pub fn run_check(project_root: &Path, verbose: bool) -> Result<()> {
    let agents_md = project_root.join("AGENTS.md");
    if !agents_md.exists() {
        bail!(
            "No AGENTS.md found in {}. Run {} first.",
            project_root.display(),
            "conforme init".bold()
        );
    }

    let content = std::fs::read_to_string(&agents_md).context("failed to read AGENTS.md")?;
    let config = markdown::parse_agents_md(&content)?;

    let adapters = adapters::all_adapters();
    let mut out_of_sync = Vec::new();

    for adapter in &adapters {
        if !adapter.detect(project_root) {
            continue;
        }

        let generated = adapter.generate(project_root, &config)?;
        let mut tool_diffs = Vec::new();

        for (path, expected) in &generated {
            if path.exists() {
                let existing = std::fs::read_to_string(path)?;
                if !crate::hash::contents_match(&existing, expected) {
                    tool_diffs.push(path.clone());
                }
            } else {
                tool_diffs.push(path.clone());
            }
        }

        if !tool_diffs.is_empty() {
            out_of_sync.push((adapter.name().to_string(), tool_diffs));
        } else if verbose {
            println!("{} {} in sync", "+".green(), adapter.name());
        }
    }

    if out_of_sync.is_empty() {
        println!("{} All configs in sync.", "+".green());
        Ok(())
    } else {
        println!("{} Configs out of sync:", "x".red());
        for (tool_name, files) in &out_of_sync {
            println!("  {} ({} files differ):", tool_name.bold(), files.len());
            for path in files {
                println!(
                    "    {}",
                    path.strip_prefix(project_root).unwrap_or(path).display()
                );
            }
        }
        println!("\nRun {} to fix.", "conforme sync".bold());
        std::process::exit(1);
    }
}

/// Run the `status` command.
pub fn run_status(project_root: &Path, _verbose: bool) -> Result<()> {
    let has_agents = detect::has_agents_md(project_root);
    let tools = detect::detect_tools(project_root);

    println!("{}", "Tool Status".bold().underline());
    println!();

    // AGENTS.md status
    if has_agents {
        println!(
            "  {:<20} {:<12} {}",
            "AGENTS.md",
            "Yes".green(),
            "Source of truth".dimmed()
        );
    } else {
        println!(
            "  {:<20} {:<12} {}",
            "AGENTS.md",
            "No".red(),
            "Run `conforme init` to create".dimmed()
        );
    }

    // Check sync status for each tool
    let config = if has_agents {
        let content = std::fs::read_to_string(project_root.join("AGENTS.md"))?;
        Some(markdown::parse_agents_md(&content)?)
    } else {
        None
    };

    let adapters = adapters::all_adapters();
    for (tool, adapter) in tools.iter().zip(adapters.iter()) {
        let detected_str = if tool.detected {
            "Yes".green().to_string()
        } else {
            "No".dimmed().to_string()
        };

        let sync_status = if !tool.detected {
            "--".dimmed().to_string()
        } else if let Some(ref cfg) = config {
            match check_sync_status(project_root, adapter.as_ref(), cfg) {
                Ok(true) => "In sync".green().to_string(),
                Ok(false) => "Out of sync".yellow().to_string(),
                Err(_) => "Error".red().to_string(),
            }
        } else {
            "No source".dimmed().to_string()
        };

        println!("  {:<20} {:<12} {}", tool.name, detected_str, sync_status);
    }

    println!();
    Ok(())
}

fn check_sync_status(
    project_root: &Path,
    adapter: &dyn AiToolAdapter,
    config: &crate::config::NormalizedConfig,
) -> Result<bool> {
    let generated = adapter.generate(project_root, config)?;
    for (path, expected) in &generated {
        if path.exists() {
            let existing = std::fs::read_to_string(path)?;
            if !crate::hash::contents_match(&existing, expected) {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}
