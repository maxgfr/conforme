use anyhow::{bail, Context, Result};
use owo_colors::OwoColorize;
use similar::{ChangeTag, TextDiff};
use std::path::Path;

use crate::adapters::{self, clean_orphans, AiToolAdapter};
use crate::cli::AddTarget;
use crate::config::NormalizedConfig;
use crate::detect;
use crate::markdown;
use crate::project_config::ProjectConfig;
use crate::validate;

/// Resolve the source config: reads from the configured source tool or AGENTS.md.
fn resolve_config(
    project_root: &Path,
    from: Option<&str>,
    project_cfg: &ProjectConfig,
    verbose: bool,
) -> Result<(NormalizedConfig, String)> {
    let adapters = adapters::all_adapters();

    // Priority: --from flag > .conformerc.toml source > AGENTS.md fallback
    let source_id = from
        .map(|s| s.to_string())
        .or_else(|| project_cfg.source.clone());

    if let Some(ref id) = source_id {
        // Read from a specific tool adapter
        let adapter = adapters
            .iter()
            .find(|a| a.id() == id.as_str())
            .ok_or_else(|| {
                let known: Vec<&str> = adapters.iter().map(|a| a.id()).collect();
                anyhow::anyhow!(
                    "Unknown source tool '{}'. Known tools: {}",
                    id,
                    known.join(", ")
                )
            })?;

        if verbose {
            println!("  Reading config from {}...", adapter.name().bold());
        }

        let config = adapter
            .read(project_root)
            .with_context(|| format!("failed to read config from {}", adapter.name()))?;

        return Ok((config, id.clone()));
    }

    // Fallback: read AGENTS.md
    let agents_md = project_root.join("AGENTS.md");
    if agents_md.exists() {
        let content = std::fs::read_to_string(&agents_md).context("failed to read AGENTS.md")?;
        let config = markdown::parse_agents_md(&content)?;
        return Ok((config, "agents.md".to_string()));
    }

    bail!(
        "No source configured. Either:\n  \
         - Create AGENTS.md with {}\n  \
         - Set source in .conformerc.toml: source = \"claude\"\n  \
         - Use --from flag: conforme sync --from claude",
        "conforme init".bold()
    );
}

/// Run the `init` command.
pub fn run_init(project_root: &Path, force: bool, verbose: bool) -> Result<()> {
    // Create .conformerc.toml if it doesn't exist
    let config_path = project_root.join(".conformerc.toml");
    if !config_path.exists() {
        let template = r#"# conforme configuration
# Source tool — conforme reads config from here and syncs to all others
# source = "claude"

# Only sync to these tools (default: all detected)
# only = ["cursor", "copilot", "windsurf"]

# Exclude these tools from sync
# exclude = ["zed", "amp"]

# Auto-generate AGENTS.md from source (default: true)
generate_agents_md = true

# Clean orphan files on sync (default: true)
clean = true
"#;
        std::fs::write(&config_path, template)?;
        println!("{} Created .conformerc.toml", "+".green());
    }

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
    run_sync(project_root, false, None, None, false, verbose)
}

/// Run the `sync` command.
pub fn run_sync(
    project_root: &Path,
    dry_run: bool,
    only: Option<&[String]>,
    from: Option<&str>,
    no_clean: bool,
    verbose: bool,
) -> Result<()> {
    let project_cfg = ProjectConfig::load(project_root);
    let (config, source_id) = resolve_config(project_root, from, &project_cfg, verbose)?;

    if verbose {
        println!(
            "  Source: {} ({} rules, {} skills, {} agents, {} MCP servers)",
            source_id.bold(),
            config.rules.len(),
            config.skills.len(),
            config.agents.len(),
            config.mcp_servers.len(),
        );
    }

    // Validate
    if !validate::validate(&config, verbose) {
        bail!("Validation failed. Fix the errors above before syncing.");
    }

    let adapters = adapters::all_adapters();
    let mut any_written = false;

    // Resolve the effective --only list (CLI > .conformerc.toml)
    let effective_only: Option<Vec<String>> = only
        .map(|o| o.to_vec())
        .or_else(|| project_cfg.only.clone());

    // Warn about unknown tool names
    if let Some(ref only_list) = effective_only {
        let known_ids: Vec<&str> = adapters.iter().map(|a| a.id()).collect();
        for o in only_list {
            if !known_ids.contains(&o.as_str()) {
                eprintln!(
                    "{} Unknown tool '{}'. Known tools: {}",
                    "!".yellow(),
                    o,
                    known_ids.join(", ")
                );
            }
        }
    }

    // Determine if we should clean orphans
    let should_clean = !no_clean && project_cfg.clean;

    for adapter in &adapters {
        // Skip the source tool (don't write back to it)
        if adapter.id() == source_id {
            continue;
        }

        // Filter by --only / .conformerc.toml only
        if let Some(ref only_list) = effective_only {
            if !only_list.iter().any(|o| o == adapter.id()) {
                continue;
            }
        }

        // Filter by .conformerc.toml exclude
        if let Some(ref exclude) = project_cfg.exclude {
            if exclude.iter().any(|e| e == adapter.id()) {
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

        // Warn about capability loss
        warn_capability_loss(adapter.as_ref(), &config);

        if dry_run {
            let generated = adapter.generate(project_root, &config)?;
            println!("{} {} (dry-run):", ">".cyan(), adapter.name().bold());
            for (path, expected) in &generated {
                if path.exists() {
                    let existing = std::fs::read_to_string(path)?;
                    if crate::hash::contents_match(&existing, expected) {
                        println!(
                            "    {} {}",
                            "unchanged".dimmed(),
                            path.strip_prefix(project_root).unwrap_or(path).display()
                        );
                    } else {
                        println!(
                            "    {} {}",
                            "would update".yellow(),
                            path.strip_prefix(project_root).unwrap_or(path).display()
                        );
                        // Show diff in dry-run
                        print_diff(&existing, expected);
                    }
                } else {
                    println!(
                        "    {} {}",
                        "would create".green(),
                        path.strip_prefix(project_root).unwrap_or(path).display()
                    );
                }
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

            // Clean orphans
            if should_clean {
                let managed_dirs = adapter.managed_directories(project_root);
                if !managed_dirs.is_empty() {
                    let generated = adapter.generate(project_root, &config)?;
                    match clean_orphans(&managed_dirs, &generated) {
                        Ok(cleaned) => {
                            for path in &cleaned {
                                any_written = true;
                                println!(
                                    "    {} {}",
                                    "cleaned".red(),
                                    path.strip_prefix(project_root).unwrap_or(path).display()
                                );
                            }
                        }
                        Err(e) if verbose => {
                            eprintln!("  {} Failed to clean orphans: {}", "!".yellow(), e);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Optionally generate AGENTS.md as output
    if !dry_run && source_id != "agents.md" && project_cfg.generate_agents_md {
        let agents_content = markdown::export_as_agents_md(&config);
        let agents_path = project_root.join("AGENTS.md");
        let should_write = if agents_path.exists() {
            let existing = std::fs::read_to_string(&agents_path)?;
            !crate::hash::contents_match(&existing, &agents_content)
        } else {
            true
        };
        if should_write {
            std::fs::write(&agents_path, &agents_content)?;
            any_written = true;
            println!(
                "{} {} (generated from {})",
                ">".green(),
                "AGENTS.md".bold(),
                source_id
            );
        }
    }

    if !dry_run && !any_written {
        println!("{} All configs already in sync.", "=".green());
    }

    Ok(())
}

/// Warn about capabilities lost when syncing to this adapter.
fn warn_capability_loss(adapter: &dyn AiToolAdapter, config: &NormalizedConfig) {
    let caps = adapter.capabilities();

    if !caps.activation_modes {
        let has_non_always = config
            .rules
            .iter()
            .any(|r| !matches!(r.activation, crate::config::ActivationMode::Always));
        if has_non_always {
            eprintln!(
                "  {} {} does not support activation modes — all rules will be always-on",
                "!".yellow(),
                adapter.name()
            );
        }
    }

    if !caps.skills && !config.skills.is_empty() {
        eprintln!(
            "  {} {} does not support skills — {} skill(s) will be skipped",
            "!".yellow(),
            adapter.name(),
            config.skills.len()
        );
    }

    if !caps.agents && !config.agents.is_empty() {
        eprintln!(
            "  {} {} does not support agents — {} agent(s) will be skipped",
            "!".yellow(),
            adapter.name(),
            config.agents.len()
        );
    }

    if !caps.mcp && !config.mcp_servers.is_empty() {
        eprintln!(
            "  {} {} does not support MCP servers — {} server(s) will be skipped",
            "!".yellow(),
            adapter.name(),
            config.mcp_servers.len()
        );
    }
}

/// Run the `remove` command.
pub fn run_remove(project_root: &Path, tools: &[String], verbose: bool) -> Result<()> {
    let project_cfg = ProjectConfig::load(project_root);
    let (config, _) = resolve_config(project_root, None, &project_cfg, verbose)?;

    let adapters = adapters::all_adapters();
    let known_ids: Vec<&str> = adapters.iter().map(|a| a.id()).collect();

    for tool in tools {
        if !known_ids.contains(&tool.as_str()) {
            eprintln!(
                "{} Unknown tool '{}'. Known tools: {}",
                "!".yellow(),
                tool,
                known_ids.join(", ")
            );
        }
    }

    let mut any_removed = false;

    for adapter in &adapters {
        if !tools.iter().any(|t| t == adapter.id()) {
            continue;
        }

        let generated = adapter.generate(project_root, &config)?;
        let mut removed_files = Vec::new();

        for (path, _) in &generated {
            if path.exists() {
                std::fs::remove_file(path)?;
                removed_files.push(path.clone());
            }
        }

        if !removed_files.is_empty() {
            any_removed = true;
            println!("{} {}:", "x".red(), adapter.name().bold());
            for path in &removed_files {
                println!(
                    "    {} {}",
                    "removed".red(),
                    path.strip_prefix(project_root).unwrap_or(path).display()
                );
            }
        } else if verbose {
            println!(
                "  {} {} (no files to remove)",
                "-".dimmed(),
                adapter.name().dimmed()
            );
        }
    }

    if !any_removed {
        println!("{} No files to remove.", "=".green());
    }

    Ok(())
}

/// Run the `check` command.
pub fn run_check(project_root: &Path, from: Option<&str>, verbose: bool) -> Result<()> {
    let project_cfg = ProjectConfig::load(project_root);
    let (config, source_id) = resolve_config(project_root, from, &project_cfg, verbose)?;

    let adapters = adapters::all_adapters();
    let mut out_of_sync = Vec::new();

    for adapter in &adapters {
        if adapter.id() == source_id {
            continue;
        }

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
    let project_cfg = ProjectConfig::load(project_root);

    println!("{}", "Tool Status".bold().underline());
    println!();

    // Source info
    if let Some(ref source) = project_cfg.source {
        println!("  {:<20} {}", "Source:".bold(), source.green());
    } else if has_agents {
        println!(
            "  {:<20} {}",
            "Source:".bold(),
            "AGENTS.md (default)".green()
        );
    } else {
        println!("  {:<20} {}", "Source:".bold(), "Not configured".red());
    }

    // AGENTS.md status
    if has_agents {
        println!(
            "  {:<20} {:<12} {}",
            "AGENTS.md",
            "Yes".green(),
            if project_cfg.source.is_some() {
                "Generated output"
            } else {
                "Source of truth"
            }
            .dimmed()
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
    let config = resolve_config(project_root, None, &project_cfg, false).ok();

    let adapters = adapters::all_adapters();
    for (tool, adapter) in tools.iter().zip(adapters.iter()) {
        let detected_str = if tool.detected {
            "Yes".green().to_string()
        } else {
            "No".dimmed().to_string()
        };

        let sync_status = if !tool.detected {
            "--".dimmed().to_string()
        } else if let Some((ref cfg, ref source_id)) = config {
            if adapter.id() == source_id.as_str() {
                "Source".cyan().to_string()
            } else {
                match check_sync_status(project_root, adapter.as_ref(), cfg) {
                    Ok(true) => "In sync".green().to_string(),
                    Ok(false) => "Out of sync".yellow().to_string(),
                    Err(_) => "Error".red().to_string(),
                }
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
    config: &NormalizedConfig,
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

/// Run the `diff` command.
pub fn run_diff(
    project_root: &Path,
    only: Option<&[String]>,
    from: Option<&str>,
    verbose: bool,
) -> Result<()> {
    let project_cfg = ProjectConfig::load(project_root);
    let (config, source_id) = resolve_config(project_root, from, &project_cfg, verbose)?;

    let adapters = adapters::all_adapters();
    let mut any_diff = false;

    for adapter in &adapters {
        if adapter.id() == source_id {
            continue;
        }

        if let Some(only_list) = only {
            if !only_list.iter().any(|o| o == adapter.id()) {
                continue;
            }
        }

        if !adapter.detect(project_root) {
            continue;
        }

        let generated = adapter.generate(project_root, &config)?;
        let mut tool_has_diff = false;

        for (path, expected) in &generated {
            let existing = if path.exists() {
                std::fs::read_to_string(path)?
            } else {
                String::new()
            };

            if !crate::hash::contents_match(&existing, expected) {
                if !tool_has_diff {
                    println!("{} {}:", ">".cyan(), adapter.name().bold());
                    tool_has_diff = true;
                    any_diff = true;
                }
                let rel_path = path.strip_prefix(project_root).unwrap_or(path);
                println!("  {}:", rel_path.display().to_string().bold());
                print_diff(&existing, expected);
            }
        }
    }

    if !any_diff {
        println!("{} All configs in sync.", "+".green());
    }

    Ok(())
}

/// Print a unified diff between two strings.
fn print_diff(old: &str, new: &str) {
    let diff = TextDiff::from_lines(old, new);
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => print!("    {}", format!("-{change}").red()),
            ChangeTag::Insert => print!("    {}", format!("+{change}").green()),
            ChangeTag::Equal => {}
        }
    }
}

/// Run the `migrate` command: read from source, write to output, delete source files.
pub fn run_migrate(
    project_root: &Path,
    source: &str,
    output: &str,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    if source == output {
        bail!("Source and output tools cannot be the same.");
    }

    let adapters = adapters::all_adapters();
    let known_ids: Vec<&str> = adapters.iter().map(|a| a.id()).collect();

    let source_adapter = adapters.iter().find(|a| a.id() == source).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown source tool '{}'. Known tools: {}",
            source,
            known_ids.join(", ")
        )
    })?;

    let output_adapter = adapters.iter().find(|a| a.id() == output).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown output tool '{}'. Known tools: {}",
            output,
            known_ids.join(", ")
        )
    })?;

    if !source_adapter.detect(project_root) {
        bail!(
            "{} is not detected in this project. Cannot read config from it.",
            source_adapter.name()
        );
    }

    // Read config from source
    let config = source_adapter
        .read(project_root)
        .with_context(|| format!("failed to read config from {}", source_adapter.name()))?;

    if verbose {
        println!(
            "  Source: {} ({} rules, {} skills, {} agents, {} MCP servers)",
            source_adapter.name().bold(),
            config.rules.len(),
            config.skills.len(),
            config.agents.len(),
            config.mcp_servers.len(),
        );
    }

    // Warn about capability loss on the output adapter
    warn_capability_loss(output_adapter.as_ref(), &config);

    // Collect source files to delete (generated files for the source adapter)
    let source_files = source_adapter.generate(project_root, &config)?;
    let source_managed_dirs = source_adapter.managed_directories(project_root);

    if dry_run {
        // Show what would be written
        let generated = output_adapter.generate(project_root, &config)?;
        println!(
            "{} {} (dry-run, would generate):",
            ">".cyan(),
            output_adapter.name().bold()
        );
        for (path, _) in &generated {
            let rel = path.strip_prefix(project_root).unwrap_or(path);
            if path.exists() {
                println!("    {} {}", "would update".yellow(), rel.display());
            } else {
                println!("    {} {}", "would create".green(), rel.display());
            }
        }

        // Show what would be deleted
        println!(
            "{} {} (dry-run, would delete):",
            "x".cyan(),
            source_adapter.name().bold()
        );
        for (path, _) in &source_files {
            if path.exists() {
                let rel = path.strip_prefix(project_root).unwrap_or(path);
                println!("    {} {}", "would remove".red(), rel.display());
            }
        }
        // Also show managed directory contents (recursive)
        for dir in &source_managed_dirs {
            if dir.is_dir() {
                for path in collect_files_recursive(dir)? {
                    let rel = path.strip_prefix(project_root).unwrap_or(&path);
                    println!("    {} {}", "would remove".red(), rel.display());
                }
            }
        }
    } else {
        // 1. Write output files
        let report = output_adapter.write(project_root, &config)?;
        if !report.files_written.is_empty() {
            println!("{} {}:", ">".green(), output_adapter.name().bold());
            for path in &report.files_written {
                println!(
                    "    {} {}",
                    "wrote".green(),
                    path.strip_prefix(project_root).unwrap_or(path).display()
                );
            }
        }

        // 2. Delete source files
        let mut any_removed = false;
        let mut removed_paths = Vec::new();

        for (path, _) in &source_files {
            if path.exists() {
                std::fs::remove_file(path)?;
                removed_paths.push(path.clone());
            }
        }

        // Also clean managed directories (recursive)
        for dir in &source_managed_dirs {
            if dir.is_dir() {
                for path in collect_files_recursive(dir)? {
                    std::fs::remove_file(&path)?;
                    removed_paths.push(path);
                }
            }
        }

        if !removed_paths.is_empty() {
            any_removed = true;
            println!("{} {}:", "x".red(), source_adapter.name().bold());
            for path in &removed_paths {
                println!(
                    "    {} {}",
                    "removed".red(),
                    path.strip_prefix(project_root).unwrap_or(path).display()
                );
            }
        }

        if report.files_written.is_empty() && !any_removed {
            println!("{} Nothing to do.", "=".green());
        } else {
            println!(
                "\n{} Migrated from {} to {}.",
                "+".green(),
                source_adapter.name().bold(),
                output_adapter.name().bold()
            );
        }
    }

    Ok(())
}

/// Run the `add` command.
pub fn run_add(project_root: &Path, target: &AddTarget, verbose: bool) -> Result<()> {
    let agents_path = project_root.join("AGENTS.md");

    // Read existing AGENTS.md or start fresh
    let mut content = if agents_path.exists() {
        std::fs::read_to_string(&agents_path)?
    } else {
        "# Project Instructions\n\n".to_string()
    };

    // Ensure trailing newline
    if !content.ends_with('\n') {
        content.push('\n');
    }

    match target {
        AddTarget::Rule {
            name,
            activation,
            content: rule_content,
        } => {
            content.push_str(&format!("\n## Rule: {name}\n"));
            content.push_str(&format!("<!-- activation: {activation} -->\n"));
            if !rule_content.is_empty() {
                content.push_str(&format!("\n{rule_content}\n"));
            } else {
                content.push_str("\n<!-- Add rule content here -->\n");
            }
            println!("{} Added rule '{}'", "+".green(), name.bold());
        }
        AddTarget::Skill {
            name,
            description,
            tools,
            content: skill_content,
        } => {
            content.push_str(&format!("\n## Skill: {name}\n"));
            if !description.is_empty() {
                content.push_str(&format!("<!-- description: {description} -->\n"));
            }
            if !tools.is_empty() {
                content.push_str(&format!("<!-- tools: {tools} -->\n"));
            }
            if !skill_content.is_empty() {
                content.push_str(&format!("\n{skill_content}\n"));
            } else {
                content.push_str("\n<!-- Add skill content here -->\n");
            }
            println!("{} Added skill '{}'", "+".green(), name.bold());
        }
        AddTarget::Agent {
            name,
            description,
            model,
            tools,
            content: agent_content,
        } => {
            content.push_str(&format!("\n## Agent: {name}\n"));
            if !description.is_empty() {
                content.push_str(&format!("<!-- description: {description} -->\n"));
            }
            if let Some(m) = model {
                content.push_str(&format!("<!-- model: {m} -->\n"));
            }
            if !tools.is_empty() {
                content.push_str(&format!("<!-- tools: {tools} -->\n"));
            }
            if !agent_content.is_empty() {
                content.push_str(&format!("\n{agent_content}\n"));
            } else {
                content.push_str("\n<!-- Add agent instructions here -->\n");
            }
            println!("{} Added agent '{}'", "+".green(), name.bold());
        }
        AddTarget::Mcp {
            name,
            command,
            args,
            url,
        } => {
            content.push_str(&format!("\n## MCP: {name}\n"));
            if let Some(cmd) = command {
                content.push_str(&format!("<!-- command: {cmd} -->\n"));
                if !args.is_empty() {
                    content.push_str(&format!("<!-- args: {args} -->\n"));
                }
            } else if let Some(u) = url {
                content.push_str(&format!("<!-- url: {u} -->\n"));
            }
            content.push('\n');
            println!("{} Added MCP server '{}'", "+".green(), name.bold());
        }
    }

    std::fs::write(&agents_path, &content)?;

    if verbose {
        println!("  Updated AGENTS.md");
    }

    Ok(())
}

/// Recursively collect all files under a directory.
fn collect_files_recursive(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_files_recursive(&path)?);
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}
