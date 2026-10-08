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

/// Tool ids conforme renamed, as `(old, new, version)`. An old id in
/// `.conformerc.toml` or on the command line is an error rather than an
/// unknown name to skip: an ignored `exclude = ["windsurf"]` would sync, and
/// clean, the very tool the user meant to leave alone.
const RENAMED_IDS: &[(&str, &str, &str)] = &[("windsurf", "devin", "4.0.0")];

fn check_renamed_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Result<()> {
    for id in ids {
        if let Some((old, new, version)) = RENAMED_IDS.iter().find(|(old, ..)| *old == id) {
            bail!(
                "the `{old}` tool id was renamed `{new}` in conforme {version}; \
                 replace it in .conformerc.toml (source, only, exclude) and on the command line"
            );
        }
    }
    Ok(())
}

fn find_adapter(id: &str) -> Option<Box<dyn AiToolAdapter>> {
    adapters::all_adapters().into_iter().find(|a| a.id() == id)
}

/// The config a target tool is generated from. When the target writes skills
/// into a directory the source tool reads its own skills from
/// (`.agents/skills/` is shared by Codex, Zed and Amp), that directory is
/// left to the source: the target would add flat copies of the source's
/// nested skills there, which the source would then load twice.
fn target_config<'a>(
    project_root: &Path,
    source_id: &str,
    target: &dyn AiToolAdapter,
    config: &'a NormalizedConfig,
) -> std::borrow::Cow<'a, NormalizedConfig> {
    let skills_dirs = |adapter: &dyn AiToolAdapter| -> Vec<std::path::PathBuf> {
        adapter
            .managed_directories(project_root)
            .into_iter()
            .filter(|d| d.orphan_suffix.is_none() && d.superseded_by.is_none())
            .map(|d| d.path)
            .collect()
    };
    let Some(source) = find_adapter(source_id) else {
        return std::borrow::Cow::Borrowed(config);
    };
    // The source's skills roots, plus a fallback root it reads (DeepSeek's
    // `.agents/skills` when `.dsh/skills` is empty).
    let mut source_dirs = skills_dirs(source.as_ref());
    source_dirs.extend(source.source_files(project_root));
    if skills_dirs(target).iter().any(|d| source_dirs.contains(d)) {
        let mut stripped = config.clone();
        stripped.skills.clear();
        std::borrow::Cow::Owned(stripped)
    } else {
        std::borrow::Cow::Borrowed(config)
    }
}

/// What the source reads outside its own managed directories (see
/// [`AiToolAdapter::source_files`]); `AGENTS.md` itself when it is the source.
/// No target writes these, and `remove`/`migrate` never delete them.
fn source_files(project_root: &Path, source_id: &str) -> Vec<std::path::PathBuf> {
    if source_id == "agents.md" {
        return vec![project_root.join("AGENTS.md")];
    }
    find_adapter(source_id)
        .map(|source| source.source_files(project_root))
        .unwrap_or_default()
}

fn is_source_file(path: &Path, source_files: &[std::path::PathBuf]) -> bool {
    source_files.iter().any(|p| path.starts_with(p))
}

/// The detected tools sync writes to: every tool but the source, filtered by
/// `only` (command line, else `.conformerc.toml`) and `exclude`. `check`,
/// `diff` and `status` use the same set, so an excluded tool is never
/// reported out of sync.
fn selected_targets<'a>(
    adapters: &'a [Box<dyn AiToolAdapter>],
    project_root: &Path,
    source_id: &str,
    only: Option<&[String]>,
    project_cfg: &ProjectConfig,
) -> Vec<&'a dyn AiToolAdapter> {
    let only = only
        .map(<[String]>::to_vec)
        .or_else(|| project_cfg.only.clone());
    adapters
        .iter()
        .map(|a| a.as_ref())
        .filter(|a| a.id() != source_id)
        .filter(|a| {
            only.as_ref()
                .is_none_or(|o| o.iter().any(|id| id == a.id()))
        })
        .filter(|a| {
            project_cfg
                .exclude
                .as_ref()
                .is_none_or(|e| !e.iter().any(|id| id == a.id()))
        })
        .filter(|a| a.detect(project_root))
        .collect()
}

/// The files a target tool gets: what it generates from [`target_config`],
/// minus the files the source reads (an OpenCode source reading `CLAUDE.md`
/// keeps it out of the Claude Code target's reach).
fn target_files(
    project_root: &Path,
    source_id: &str,
    target: &dyn AiToolAdapter,
    config: &NormalizedConfig,
    source_files: &[std::path::PathBuf],
) -> Result<Vec<(std::path::PathBuf, String)>> {
    let config = target_config(project_root, source_id, target, config);
    Ok(target
        .generate(project_root, &config)?
        .into_iter()
        .filter(|(path, _)| !is_source_file(path, source_files))
        .collect())
}

/// Write a target's files (see [`target_files`]). The adapter's own `write`
/// runs unless a source file had to be left out of its output.
fn write_target(
    project_root: &Path,
    source_id: &str,
    target: &dyn AiToolAdapter,
    config: &NormalizedConfig,
    source_files: &[std::path::PathBuf],
) -> Result<adapters::WriteReport> {
    let config = target_config(project_root, source_id, target, config);
    let generated = target.generate(project_root, &config)?;
    if !generated
        .iter()
        .any(|(path, _)| is_source_file(path, source_files))
    {
        return target.write(project_root, &config);
    }
    let mut report = adapters::WriteReport {
        files_written: Vec::new(),
        files_unchanged: Vec::new(),
    };
    for (path, content) in generated {
        if !is_source_file(&path, source_files) {
            adapters::write_if_changed(&path, &content, &mut report)?;
        }
    }
    Ok(report)
}

/// The AGENTS.md sync generates, when it does: a tool is the source, the
/// source does not read AGENTS.md itself, and `generate_agents_md` is on.
fn generated_agents_md(
    project_root: &Path,
    source_id: &str,
    project_cfg: &ProjectConfig,
    config: &NormalizedConfig,
) -> Option<(std::path::PathBuf, String)> {
    let source_reads_agents_md =
        find_adapter(source_id).is_some_and(|a| a.reads_agents_md(project_root));
    (source_id != "agents.md" && !source_reads_agents_md && project_cfg.generate_agents_md).then(
        || {
            (
                project_root.join("AGENTS.md"),
                markdown::export_as_agents_md(config),
            )
        },
    )
}

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
        check_renamed_ids([id.as_str()])?;
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
# only = ["cursor", "copilot", "devin"]

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

    // A source that reads back as empty (an unconfigured tool, a format
    // conforme does not read) would blank AGENTS.md and clean every rule,
    // skill and agent of the other tools: refuse to treat it as a config.
    if config.is_empty() {
        eprintln!(
            "{} {} has no instructions, rules, skills, agents or MCP servers: nothing to sync.",
            "!".yellow(),
            source_id
        );
        return Ok(());
    }

    let adapters = adapters::all_adapters();
    let mut any_written = false;

    // Resolve the effective --only list (CLI > .conformerc.toml)
    let effective_only: Option<Vec<String>> = only
        .map(|o| o.to_vec())
        .or_else(|| project_cfg.only.clone());
    check_renamed_ids(
        effective_only
            .iter()
            .chain(project_cfg.exclude.iter())
            .flatten()
            .map(String::as_str),
    )?;

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
    let source_files = source_files(project_root, &source_id);

    if verbose {
        for adapter in &adapters {
            if adapter.id() != source_id && !adapter.detect(project_root) {
                println!(
                    "  {} {} (not detected, skipping)",
                    "-".dimmed(),
                    adapter.name().dimmed()
                );
            }
        }
    }

    for adapter in selected_targets(&adapters, project_root, &source_id, only, &project_cfg) {
        // Warn about capability loss
        warn_capability_loss(adapter, &config);

        if dry_run {
            let generated =
                target_files(project_root, &source_id, adapter, &config, &source_files)?;
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
            let report = write_target(project_root, &source_id, adapter, &config, &source_files)?;
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
                    let target_config = target_config(project_root, &source_id, adapter, &config);
                    let generated = adapter.generate(project_root, &target_config)?;
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

    // Optionally generate AGENTS.md as output — unless the source tool reads
    // AGENTS.md itself, in which case that file is the source.
    if let Some((agents_path, agents_content)) = (!dry_run)
        .then(|| generated_agents_md(project_root, &source_id, &project_cfg, &config))
        .flatten()
    {
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
    check_renamed_ids(tools.iter().map(String::as_str))?;
    let (config, source_id) = resolve_config(project_root, None, &project_cfg, verbose)?;

    let adapters = adapters::all_adapters();
    let known_ids: Vec<&str> = adapters.iter().map(|a| a.id()).collect();

    // A file the source or another tool that stays also generates is never
    // removed: `.agents/skills/` belongs to Codex, Zed and Amp at once. Nor is
    // a file the source reads (an Amp source reading `CLAUDE.md`).
    let source_files = source_files(project_root, &source_id);
    let mut kept = std::collections::HashSet::new();
    for adapter in &adapters {
        let is_source = adapter.id() == source_id;
        if !is_source && (tools.iter().any(|t| t == adapter.id()) || !adapter.detect(project_root))
        {
            continue;
        }
        kept.extend(
            adapter
                .generate(project_root, &config)?
                .into_iter()
                .map(|(path, _)| path),
        );
    }

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
            if adapter.is_shared_file(path)
                || kept.contains(path)
                || is_source_file(path, &source_files)
            {
                if verbose && path.exists() {
                    println!(
                        "  {} preserved shared config {}",
                        "-".dimmed(),
                        path.strip_prefix(project_root).unwrap_or(path).display()
                    );
                }
                continue;
            }
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

    // Configs sync would refuse are not "in sync" either.
    if !validate::validate(&config, verbose) {
        bail!("Validation failed. Fix the errors above, then run `conforme sync`.");
    }

    let adapters = adapters::all_adapters();
    let mut out_of_sync = Vec::new();
    let source_files = source_files(project_root, &source_id);
    // An empty source makes sync stop without writing anything: nothing to
    // compare either.
    let targets = if config.is_empty() {
        Vec::new()
    } else {
        selected_targets(&adapters, project_root, &source_id, None, &project_cfg)
    };

    for adapter in targets {
        let generated = target_files(project_root, &source_id, adapter, &config, &source_files)?;
        let tool_diffs = differing_files(&generated)?;

        if !tool_diffs.is_empty() {
            out_of_sync.push((adapter.name().to_string(), tool_diffs));
        } else if verbose {
            println!("{} {} in sync", "+".green(), adapter.name());
        }
    }

    // The AGENTS.md sync generates is an output like any other: Codex,
    // OpenCode, Amp and DeepSeek read it.
    if !config.is_empty() {
        if let Some(agents_md) =
            generated_agents_md(project_root, &source_id, &project_cfg, &config)
        {
            let diffs = differing_files(&[agents_md])?;
            if !diffs.is_empty() {
                out_of_sync.push(("AGENTS.md".to_string(), diffs));
            }
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
            match project_cfg.source.as_deref() {
                None => "Source of truth",
                // Codex, OpenCode, Amp, DeepSeek (and Claude Code or Gemini
                // CLI when they fall back to it) read AGENTS.md as their config.
                Some(source)
                    if find_adapter(source).is_some_and(|a| a.reads_agents_md(project_root)) =>
                {
                    "Source's own file"
                }
                Some(_) if project_cfg.generate_agents_md => "Generated output",
                Some(_) => "Not managed",
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
    let source_id = config
        .as_ref()
        .map(|(_, id)| id.clone())
        .unwrap_or_default();
    let selected: Vec<&str> =
        selected_targets(&adapters, project_root, &source_id, None, &project_cfg)
            .iter()
            .map(|a| a.id())
            .collect();
    let source_files = source_files(project_root, &source_id);
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
            } else if !selected.contains(&adapter.id()) {
                "Excluded".dimmed().to_string()
            } else {
                match target_files(
                    project_root,
                    source_id,
                    adapter.as_ref(),
                    cfg,
                    &source_files,
                )
                .and_then(|generated| differing_files(&generated))
                {
                    Ok(differing) if differing.is_empty() => "In sync".green().to_string(),
                    Ok(_) => "Out of sync".yellow().to_string(),
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

/// The generated files whose content on disk differs (or that are missing).
fn differing_files(generated: &[(std::path::PathBuf, String)]) -> Result<Vec<std::path::PathBuf>> {
    let mut differing = Vec::new();
    for (path, expected) in generated {
        let in_sync =
            path.exists() && crate::hash::contents_match(&std::fs::read_to_string(path)?, expected);
        if !in_sync {
            differing.push(path.clone());
        }
    }
    Ok(differing)
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
    check_renamed_ids(only.into_iter().flatten().map(String::as_str))?;

    let adapters = adapters::all_adapters();
    let mut any_diff = false;
    let source_files = source_files(project_root, &source_id);
    let mut outputs: Vec<(String, Vec<(std::path::PathBuf, String)>)> = Vec::new();
    if !config.is_empty() {
        for adapter in selected_targets(&adapters, project_root, &source_id, only, &project_cfg) {
            outputs.push((
                adapter.name().to_string(),
                target_files(project_root, &source_id, adapter, &config, &source_files)?,
            ));
        }
        if let Some(agents_md) =
            generated_agents_md(project_root, &source_id, &project_cfg, &config)
        {
            outputs.push(("AGENTS.md".to_string(), vec![agents_md]));
        }
    }

    for (name, generated) in &outputs {
        let mut tool_has_diff = false;

        for (path, expected) in generated {
            let existing = if path.exists() {
                std::fs::read_to_string(path)?
            } else {
                String::new()
            };

            if !crate::hash::contents_match(&existing, expected) {
                if !tool_has_diff {
                    println!("{} {}:", ">".cyan(), name.bold());
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
    check_renamed_ids([source, output])?;
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

    // What sync would refuse, migrate refuses too: it deletes the source.
    if !validate::validate(&config, verbose) {
        bail!("Validation failed. Fix the errors above before migrating.");
    }
    if config.is_empty() {
        bail!(
            "{} has no instructions, rules, skills, agents or MCP servers: nothing to migrate.",
            source_adapter.name()
        );
    }

    // Warn about capability loss on the output adapter
    warn_capability_loss(output_adapter.as_ref(), &config);

    // The output leaves a skills directory it shares with the source
    // (`.agents/skills/` for Codex, Amp and Zed) alone, as `sync` does: the
    // skills there are already the output's, and regenerating them would drop
    // frontmatter conforme does not model and flatten nested skills.
    let output_config = target_config(project_root, source, output_adapter.as_ref(), &config);

    // Codex, OpenCode, Amp and DeepSeek (and Gemini CLI when it only loads
    // AGENTS.md) keep their instructions in AGENTS.md: generating nothing
    // for them would delete the source's instructions and rules into nothing.
    let agents_md = migrated_agents_md(
        project_root,
        source_adapter.as_ref(),
        output_adapter.as_ref(),
        &config,
    )?;

    // Collect source files to delete (generated files for the source adapter)
    let source_files = source_adapter.generate(project_root, &config)?;
    // Tools that keep using the project after the migration: the output and
    // every other detected tool. A file one of them generates is never
    // deleted, even when it sits in a directory it shares with the source.
    let staying: Vec<&Box<dyn AiToolAdapter>> = adapters
        .iter()
        .filter(|a| a.id() == output || (a.id() != source && a.detect(project_root)))
        .collect();
    let mut kept_paths = std::collections::HashSet::new();
    let mut staying_dirs = Vec::new();
    for adapter in &staying {
        let adapter_config = target_config(project_root, source, adapter.as_ref(), &config);
        // Another tool's unreadable settings file must not block the
        // migration; its managed directories below are still protected.
        let generated = match adapter.generate(project_root, &adapter_config) {
            Err(e) if adapter.id() == output => return Err(e),
            result => result.unwrap_or_default(),
        };
        kept_paths.extend(generated.into_iter().map(|(path, _)| path));
        staying_dirs.extend(
            adapter
                .managed_directories(project_root)
                .into_iter()
                .map(|dir| dir.path),
        );
    }
    // What the output cannot hold stays where it is: skills or agents for a
    // tool without them (`sync` warned about it above).
    let caps = output_adapter.capabilities();
    let lost = NormalizedConfig {
        skills: if caps.skills {
            Vec::new()
        } else {
            config.skills.clone()
        },
        agents: if caps.agents {
            Vec::new()
        } else {
            config.agents.clone()
        },
        ..Default::default()
    };
    if !lost.skills.is_empty() || !lost.agents.is_empty() {
        kept_paths.extend(
            source_adapter
                .generate(project_root, &lost)?
                .into_iter()
                .map(|(path, _)| path),
        );
    }
    // Migrating away from a tool clears what conforme manages in its
    // directories, except a directory a staying tool manages too: everything
    // in it (bundled skill scripts included) belongs to that tool.
    let source_managed_dirs: Vec<adapters::ManagedDir> = source_adapter
        .managed_directories(project_root)
        .into_iter()
        .filter(|dir| !staying_dirs.contains(&dir.path))
        .collect();
    // A skill folder that bundles other files (scripts, references) keeps
    // them, and its SKILL.md with them: only SKILL.md reaches the output.
    let bundled_skill_dirs: Vec<std::path::PathBuf> = source_adapter
        .managed_directories(project_root)
        .iter()
        .filter(|dir| dir.orphan_suffix.is_none())
        .flat_map(|dir| bundled_skill_folders(&dir.path))
        .collect();
    let is_kept = |path: &std::path::PathBuf| {
        kept_paths.contains(path)
            || staying_dirs.iter().any(|dir| path.starts_with(dir))
            || bundled_skill_dirs.iter().any(|dir| path.starts_with(dir))
    };
    let managed_files = |dir: &adapters::ManagedDir| -> Result<Vec<std::path::PathBuf>> {
        Ok(migrated_files(dir)?
            .into_iter()
            .filter(|path| !is_kept(path))
            .collect())
    };

    if dry_run {
        // Show what would be written
        let generated = output_adapter.generate(project_root, &output_config)?;
        println!(
            "{} {} (dry-run, would generate):",
            ">".cyan(),
            output_adapter.name().bold()
        );
        for (path, content) in generated.iter().chain(agents_md.iter()) {
            let rel = path.strip_prefix(project_root).unwrap_or(path);
            if !path.exists() {
                println!("    {} {}", "would create".green(), rel.display());
            } else if crate::hash::contents_match(&std::fs::read_to_string(path)?, content) {
                println!("    {} {}", "unchanged".dimmed(), rel.display());
            } else {
                println!("    {} {}", "would update".yellow(), rel.display());
            }
        }

        // Show what would be deleted
        println!(
            "{} {} (dry-run, would delete):",
            "x".cyan(),
            source_adapter.name().bold()
        );
        for (path, _) in &source_files {
            if path.exists() && !is_kept(path) {
                let rel = path.strip_prefix(project_root).unwrap_or(path);
                if source_adapter.is_shared_file(path) {
                    println!(
                        "    {} {}",
                        "would preserve shared config".yellow(),
                        rel.display()
                    );
                } else {
                    println!("    {} {}", "would remove".red(), rel.display());
                }
            }
        }
        // Also show managed directory contents (recursive)
        for dir in &source_managed_dirs {
            for path in managed_files(dir)? {
                if source_files.iter().any(|(p, _)| *p == path) {
                    continue;
                }
                let rel = path.strip_prefix(project_root).unwrap_or(&path);
                println!("    {} {}", "would remove".red(), rel.display());
            }
        }
    } else {
        // 1. Write output files
        let mut report = output_adapter.write(project_root, &output_config)?;
        if let Some((path, content)) = &agents_md {
            adapters::write_if_changed(path, content, &mut report)?;
        }
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
            if source_adapter.is_shared_file(path) || is_kept(path) {
                continue;
            }
            if path.exists() {
                std::fs::remove_file(path)?;
                removed_paths.push(path.clone());
            }
        }

        // Also clean managed directories (recursive)
        for dir in &source_managed_dirs {
            for path in managed_files(dir)? {
                if path.exists() {
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

    // With a tool source that does not read AGENTS.md, the next sync
    // regenerates AGENTS.md from that tool and the addition is lost.
    if let Some(source) = ProjectConfig::load(project_root).source {
        if find_adapter(&source).is_some_and(|a| !a.reads_agents_md(project_root)) {
            bail!(
                "the source is {source}, so AGENTS.md is regenerated from it on every sync; \
                 add the entry in {source}'s own files instead"
            );
        }
    }

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
/// The AGENTS.md `migrate` writes when the output keeps its instructions
/// there (it reads AGENTS.md and generates nothing for instructions or
/// rules), or `None`. An existing AGENTS.md that differs, and that the source
/// does not read itself, is refused rather than overwritten.
fn migrated_agents_md(
    project_root: &Path,
    source: &dyn AiToolAdapter,
    output: &dyn AiToolAdapter,
    config: &NormalizedConfig,
) -> Result<Option<(std::path::PathBuf, String)>> {
    if config.instructions.trim().is_empty() && config.rules.is_empty() {
        return Ok(None);
    }
    let instructions_only = NormalizedConfig {
        instructions: config.instructions.clone(),
        rules: config.rules.clone(),
        ..Default::default()
    };
    let output_holds_instructions = !output
        .generate(project_root, &instructions_only)?
        .is_empty();
    if !output.reads_agents_md(project_root) || output_holds_instructions {
        return Ok(None);
    }
    let path = project_root.join("AGENTS.md");
    if source.source_files(project_root).contains(&path) {
        // The source already keeps its instructions there.
        return Ok(None);
    }
    let content = markdown::export_as_agents_md(config);
    if path.exists() && !crate::hash::contents_match(&std::fs::read_to_string(&path)?, &content) {
        bail!(
            "{} keeps its instructions in AGENTS.md, which already exists and differs from what {} holds. \
             Merge the two by hand (or remove AGENTS.md), then migrate again; nothing was changed.",
            output.name(),
            source.name()
        );
    }
    Ok(Some((path, content)))
}

/// Skill folders of a skills directory that hold files besides `SKILL.md`
/// and conforme's `agents/openai.yaml`.
fn bundled_skill_folders(skills_dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(skills_dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|folder| folder.is_dir())
        .filter(|folder| {
            collect_files_recursive(folder).is_ok_and(|files| {
                files.iter().any(|file| {
                    let rel = file.strip_prefix(folder).unwrap_or(file);
                    rel != Path::new("SKILL.md") && rel != Path::new("agents/openai.yaml")
                })
            })
        })
        .collect()
}

/// The files `migrate` removes from a managed directory of the source tool:
/// in a rules or agents directory, the files (nested ones included) with the
/// suffix conforme reads there and that `keep` does not protect; in a skills
/// directory, every file inside a skill folder. A file the tool accepts but
/// conforme never reads (a Kiro `.json` agent, a Zoo Code `.txt` rule, a dsh
/// flat skill) was not migrated, so it stays.
fn migrated_files(dir: &adapters::ManagedDir) -> Result<Vec<std::path::PathBuf>> {
    let files = collect_files_recursive(&dir.path)?;
    Ok(match dir.orphan_suffix {
        Some(suffix) => files
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(suffix))
                    && !dir.keep.is_some_and(|keep| keep(path))
            })
            .collect(),
        None => files
            .into_iter()
            .filter(|path| path.parent() != Some(dir.path.as_path()))
            .collect(),
    })
}

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
