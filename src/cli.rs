use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "conforme",
    version,
    about = "Universal AI coding agent config synchronization",
    long_about = "Conforme synchronizes configuration across 13 AI coding tools.\n\n\
        It treats AGENTS.md as the source of truth and generates/updates \
        tool-specific config files for Claude Code, Cursor, Windsurf, \
        GitHub Copilot, Codex CLI, OpenCode, Roo Code, Gemini CLI, \
        Continue.dev, Zed AI, Amazon Q, Kiro, and Amp.",
    after_help = "\x1b[1mExamples:\x1b[0m\n  \
        conforme init                        Detect tools and create configs\n  \
        conforme sync                        Sync AGENTS.md to all tool configs\n  \
        conforme sync --dry-run              Preview changes without writing\n  \
        conforme sync --only claude,cursor   Sync only to specific tools\n  \
        conforme check                       Check if configs are in sync (CI)\n  \
        conforme status                      Show detected tools and sync state\n  \
        conforme remove cursor,windsurf       Remove generated files for tools\n  \
        conforme hook install                Install git pre-commit hook\n  \
        conforme hook uninstall              Remove git pre-commit hook\n  \
        conforme help-ai                     Show all supported tools and formats"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Project root directory [default: current directory]
    #[arg(short = 'C', long = "dir", global = true)]
    pub dir: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initialize conforme for this project
    Init {
        /// Overwrite existing tool configs
        #[arg(long)]
        force: bool,
    },
    /// Sync AGENTS.md to all detected tool configs
    Sync {
        /// Preview changes without writing files
        #[arg(short = 'n', long)]
        dry_run: bool,
        /// Only sync to specific tools (comma-separated: claude,cursor,windsurf,copilot)
        #[arg(short, long, value_delimiter = ',')]
        only: Option<Vec<String>>,
    },
    /// Remove generated config files for specific tools
    Remove {
        /// Tools to remove (comma-separated: claude,cursor,windsurf)
        #[arg(value_delimiter = ',')]
        tools: Vec<String>,
    },
    /// Check if configs are in sync (exits 1 if out of sync)
    Check,
    /// Show detected tools and sync status
    Status,
    /// Manage git pre-commit hook (like Husky)
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    /// Show all supported AI tools and their config formats
    HelpAi,
}

#[derive(Subcommand, Debug)]
pub enum HookAction {
    /// Install a git pre-commit hook that runs `conforme check`
    Install,
    /// Remove the conforme pre-commit hook
    Uninstall,
}
