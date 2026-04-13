use owo_colors::OwoColorize;

pub fn print_help_ai() {
    println!(
        "{}",
        "conforme — Supported AI Coding Tools".bold().underline()
    );
    println!();
    println!(
        "conforme treats {} as the source of truth and syncs to all tool-specific formats.",
        "AGENTS.md".bold()
    );
    println!("AGENTS.md is governed by the Agentic AI Foundation (Linux Foundation).");
    println!();
    println!("{}", "Tools with per-rule config files:".bold());
    println!();
    print_tool(
        "Claude Code",
        "claude",
        "CLAUDE.md + .claude/rules/*.md",
        &[
            "Frontmatter: paths (glob array)",
            "Always rules → embedded in CLAUDE.md",
            "Glob rules → .claude/rules/{name}.md with paths: frontmatter",
            "Does NOT read AGENTS.md natively (use @AGENTS.md include)",
        ],
    );
    print_tool(
        "Cursor",
        "cursor",
        ".cursor/rules/*.mdc",
        &[
            "Frontmatter: alwaysApply (bool), globs (string), description (string)",
            "4 rule types: Always, Auto Attached (globs), Agent Requested (description), Manual",
            "Reads AGENTS.md natively",
        ],
    );
    print_tool(
        "Windsurf",
        "windsurf",
        ".windsurf/rules/*.md",
        &[
            "Frontmatter: trigger (always_on|glob|model_decision|manual), description, globs",
            "Reads AGENTS.md natively",
            "MCP is global-only (~/.codeium/windsurf/mcp_config.json)",
        ],
    );
    print_tool(
        "GitHub Copilot",
        "copilot",
        ".github/copilot-instructions.md + .github/instructions/*.instructions.md",
        &[
            "Frontmatter: applyTo (glob string), excludeAgent (optional)",
            "Glob rules → .github/instructions/{name}.instructions.md",
            "Reads AGENTS.md, CLAUDE.md, and GEMINI.md natively",
        ],
    );
    print_tool(
        "Continue.dev",
        "continue",
        ".continue/rules/*.md",
        &[
            "Frontmatter: name, globs (array), alwaysApply (bool), description",
            "Globs use YAML array format, not comma-separated",
        ],
    );
    print_tool(
        "Kiro (AWS)",
        "kiro",
        ".kiro/steering/*.md",
        &[
            "Frontmatter: inclusion (always|fileMatch|auto|manual), fileMatchPattern, name, description",
            "Successor to Amazon Q CLI",
            "Reads AGENTS.md natively",
        ],
    );
    print_tool(
        "Roo Code / Cline",
        "roocode",
        ".roo/rules/*.md",
        &[
            "Plain Markdown — NO YAML frontmatter",
            "Files loaded alphabetically (use numeric prefixes: 00-, 01-)",
            "Mode-specific rules in .roo/rules-{mode}/",
            "Reads AGENTS.md natively",
        ],
    );
    print_tool(
        "Amazon Q",
        "amazonq",
        ".amazonq/rules/*.md",
        &[
            "Plain Markdown — NO frontmatter",
            "IDE version; CLI version migrating to Kiro format",
        ],
    );
    println!();
    println!(
        "{}",
        "Tools that read AGENTS.md natively (single-file sync):".bold()
    );
    println!();
    print_tool(
        "OpenAI Codex CLI",
        "codex",
        "AGENTS.md (native)",
        &[
            "Config at ~/.codex/config.toml",
            "Also supports AGENTS.override.md",
        ],
    );
    print_tool(
        "OpenCode",
        "opencode",
        "AGENTS.md (native), falls back to CLAUDE.md",
        &[
            "Config at opencode.json",
            "Also scans .opencode/skills/, .claude/skills/, .agents/skills/",
        ],
    );
    print_tool(
        "Gemini CLI",
        "gemini",
        "GEMINI.md",
        &[
            "Hierarchical: ~/.gemini/GEMINI.md → project → subdirs",
            "Supports @file.md imports",
            "Can be configured to read AGENTS.md via settings.json",
        ],
    );
    print_tool(
        "Zed AI",
        "zed",
        ".rules",
        &[
            "Fallback chain: .rules → .cursorrules → .windsurfrules → AGENTS.md → CLAUDE.md",
            "Single file, no frontmatter",
        ],
    );
    print_tool(
        "Amp (Sourcegraph)",
        "amp",
        "AGENTS.md (native), falls back to AGENT.md or CLAUDE.md",
        &[
            "Settings at .amp/settings.json",
            "Supports @doc/file.md references in AGENTS.md",
        ],
    );
    println!();
    println!("{}", "Activation mode mapping:".bold());
    println!();
    println!(
        "  {:<16} {:<20} {:<22} {:<20} {:<18}",
        "Mode".underline(),
        "Cursor".underline(),
        "Windsurf".underline(),
        "Copilot".underline(),
        "Kiro".underline()
    );
    println!(
        "  {:<16} {:<20} {:<22} {:<20} {:<18}",
        "Always", "alwaysApply:true", "trigger:always_on", "(in main file)", "inclusion:always"
    );
    println!(
        "  {:<16} {:<20} {:<22} {:<20} {:<18}",
        "GlobMatch", "globs:\"...\"", "trigger:glob", "applyTo:\"...\"", "inclusion:fileMatch"
    );
    println!(
        "  {:<16} {:<20} {:<22} {:<20} {:<18}",
        "AgentDecision",
        "description:\"...\"",
        "trigger:model_decision",
        "(in main file)",
        "inclusion:auto"
    );
    println!(
        "  {:<16} {:<20} {:<22} {:<20} {:<18}",
        "Manual", "alwaysApply:false", "trigger:manual", "(in main file)", "inclusion:manual"
    );
    println!();
    println!(
        "For more info: {}",
        "https://github.com/maxgfr/conforme".dimmed()
    );
}

fn print_tool(name: &str, id: &str, format: &str, details: &[&str]) {
    println!("  {} ({})", name.green().bold(), id.dimmed());
    println!("    Format: {}", format);
    for detail in details {
        println!("    - {detail}");
    }
    println!();
}
