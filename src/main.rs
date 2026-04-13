mod adapters;
mod cli;
mod config;
mod detect;
mod frontmatter;
mod hash;
mod help_ai;
mod hook;
mod markdown;
mod mcp;
mod skills;
mod sync;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = cli::Cli::parse();
    let project_root = args
        .dir
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine current directory"));

    match args.command {
        cli::Command::Init { force } => sync::run_init(&project_root, force, args.verbose),
        cli::Command::Sync { dry_run, only } => {
            sync::run_sync(&project_root, dry_run, only.as_deref(), args.verbose)
        }
        cli::Command::Check => sync::run_check(&project_root, args.verbose),
        cli::Command::Status => sync::run_status(&project_root, args.verbose),
        cli::Command::Hook { action } => match action {
            cli::HookAction::Install => hook::install(&project_root, args.verbose),
            cli::HookAction::Uninstall => hook::uninstall(&project_root, args.verbose),
        },
        cli::Command::HelpAi => {
            help_ai::print_help_ai();
            Ok(())
        }
    }
}
