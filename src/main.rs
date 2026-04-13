mod adapters;
mod cli;
mod config;
mod detect;
mod frontmatter;
mod gitignore;
mod hash;
mod help_ai;
mod hook;
mod markdown;
mod mcp;
mod project_config;
mod skills;
mod sync;
mod validate;
mod watch;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = cli::Cli::parse();
    let project_root = args
        .dir
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine current directory"));

    match args.command {
        cli::Command::Init { force } => sync::run_init(&project_root, force, args.verbose),
        cli::Command::Sync {
            dry_run,
            only,
            from,
            no_clean,
        } => sync::run_sync(
            &project_root,
            dry_run,
            only.as_deref(),
            from.as_deref(),
            no_clean,
            args.verbose,
        ),
        cli::Command::Diff { only, from } => sync::run_diff(
            &project_root,
            only.as_deref(),
            from.as_deref(),
            args.verbose,
        ),
        cli::Command::Add { ref what } => sync::run_add(&project_root, what, args.verbose),
        cli::Command::Remove { tools } => sync::run_remove(&project_root, &tools, args.verbose),
        cli::Command::Check { from } => {
            sync::run_check(&project_root, from.as_deref(), args.verbose)
        }
        cli::Command::Status => sync::run_status(&project_root, args.verbose),
        cli::Command::Hook { action } => match action {
            cli::HookAction::Install => hook::install(&project_root, args.verbose),
            cli::HookAction::Uninstall => hook::uninstall(&project_root, args.verbose),
        },
        cli::Command::Gitignore { action } => match action {
            cli::GitignoreAction::Install => gitignore::install(&project_root, args.verbose),
            cli::GitignoreAction::Uninstall => gitignore::uninstall(&project_root, args.verbose),
        },
        cli::Command::Watch { only } => {
            watch::run_watch(&project_root, only.as_deref(), args.verbose)
        }
        cli::Command::Migrate {
            source,
            output,
            dry_run,
        } => sync::run_migrate(&project_root, &source, &output, dry_run, args.verbose),
        cli::Command::HelpAi => {
            help_ai::print_help_ai();
            Ok(())
        }
    }
}
