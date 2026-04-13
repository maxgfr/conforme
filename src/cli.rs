use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "conforme",
    version,
    about = "Universal AI coding agent config synchronization",
    long_about = "Conforme synchronizes configuration across 13 AI coding tools.\n\n\
        It reads config from a source tool (or AGENTS.md) and generates/updates \
        tool-specific config files for Claude Code, Cursor, Windsurf, \
        GitHub Copilot, Codex CLI, OpenCode, Roo Code, Gemini CLI, \
        Continue.dev, Zed AI, Amazon Q, Kiro, and Amp.",
    after_help = "\x1b[1mExamples:\x1b[0m\n  \
        conforme init                        Detect tools and create configs\n  \
        conforme sync                        Sync source to all tool configs\n  \
        conforme sync --from claude          Sync from Claude Code as source\n  \
        conforme sync --dry-run              Preview changes without writing\n  \
        conforme sync --only claude,cursor   Sync only to specific tools\n  \
        conforme sync --no-clean             Don't remove orphaned files\n  \
        conforme diff                        Show diff between expected and actual\n  \
        conforme check                       Check if configs are in sync (CI)\n  \
        conforme status                      Show detected tools and sync state\n  \
        conforme add rule \"Name\" --activation \"glob **/*.ts\"\n  \
        conforme remove cursor,windsurf      Remove generated files for tools\n  \
        conforme hook install                Install git pre-commit hook\n  \
        conforme hook uninstall              Remove git pre-commit hook\n  \
        conforme gitignore install           Add generated configs to .gitignore\n  \
        conforme gitignore uninstall         Remove conforme entries from .gitignore\n  \
        conforme watch                       Watch source and auto-sync\n  \
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
    /// Sync source config to all detected tool configs
    Sync {
        /// Preview changes without writing files
        #[arg(short = 'n', long)]
        dry_run: bool,
        /// Only sync to specific tools (comma-separated: claude,cursor,windsurf,copilot)
        #[arg(short, long, value_delimiter = ',')]
        only: Option<Vec<String>>,
        /// Read config from this tool instead of configured source
        #[arg(long)]
        from: Option<String>,
        /// Don't remove orphaned files from managed directories
        #[arg(long)]
        no_clean: bool,
    },
    /// Show diff between expected and actual config files
    Diff {
        /// Only diff specific tools (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        only: Option<Vec<String>>,
        /// Read config from this tool instead of configured source
        #[arg(long)]
        from: Option<String>,
    },
    /// Add a rule, skill, agent, or MCP server to AGENTS.md
    Add {
        #[command(subcommand)]
        what: AddTarget,
    },
    /// Remove generated config files for specific tools
    Remove {
        /// Tools to remove (comma-separated: claude,cursor,windsurf)
        #[arg(value_delimiter = ',')]
        tools: Vec<String>,
    },
    /// Check if configs are in sync (exits 1 if out of sync)
    Check {
        /// Read config from this tool instead of configured source
        #[arg(long)]
        from: Option<String>,
    },
    /// Show detected tools and sync status
    Status,
    /// Manage git pre-commit hook (like Husky)
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    /// Manage .gitignore entries for generated tool configs
    Gitignore {
        #[command(subcommand)]
        action: GitignoreAction,
    },
    /// Watch source config and auto-sync on changes
    Watch {
        /// Only sync to specific tools (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        only: Option<Vec<String>>,
    },
    /// Show all supported AI tools and their config formats
    HelpAi,
}

#[derive(Subcommand, Debug)]
pub enum AddTarget {
    /// Add a new rule
    Rule {
        /// Rule name
        name: String,
        /// Activation mode (always, manual, agent-decision, or "glob <patterns>")
        #[arg(long, default_value = "always")]
        activation: String,
        /// Rule content
        #[arg(long, default_value = "")]
        content: String,
    },
    /// Add a new skill
    Skill {
        /// Skill name
        name: String,
        /// Description
        #[arg(long, default_value = "")]
        description: String,
        /// Allowed tools (comma-separated)
        #[arg(long, default_value = "")]
        tools: String,
        /// Skill content
        #[arg(long, default_value = "")]
        content: String,
    },
    /// Add a new agent
    Agent {
        /// Agent name
        name: String,
        /// Description
        #[arg(long, default_value = "")]
        description: String,
        /// Model to use
        #[arg(long)]
        model: Option<String>,
        /// Tools (comma-separated)
        #[arg(long, default_value = "")]
        tools: String,
        /// Agent content
        #[arg(long, default_value = "")]
        content: String,
    },
    /// Add a new MCP server
    Mcp {
        /// Server name
        name: String,
        /// Command for stdio transport
        #[arg(long)]
        command: Option<String>,
        /// Arguments (comma-separated)
        #[arg(long, default_value = "")]
        args: String,
        /// URL for HTTP transport
        #[arg(long)]
        url: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum HookAction {
    /// Install a git pre-commit hook that runs `conforme check`
    Install,
    /// Remove the conforme pre-commit hook
    Uninstall,
}

#[derive(Subcommand, Debug)]
pub enum GitignoreAction {
    /// Add generated tool configs to .gitignore (keeps source tool tracked)
    Install,
    /// Remove conforme-managed entries from .gitignore
    Uninstall,
}
