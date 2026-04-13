use owo_colors::OwoColorize;

pub fn print_help_ai() {
    println!(
        "{}",
        "conforme — Supported AI Coding Tools".bold().underline()
    );
    println!();
    println!(
        "conforme reads config from your preferred tool (or AGENTS.md) and syncs to all others."
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
            "Commands (.claude/commands/*.md) → synced as skills to other tools",
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
            "Skills synced to .cursor/skills/<name>/SKILL.md",
            "Reads AGENTS.md natively",
        ],
    );
    print_tool(
        "Windsurf",
        "windsurf",
        ".windsurf/rules/*.md",
        &[
            "Frontmatter: trigger (always_on|glob|model_decision|manual), description, globs",
            "Skills synced to .windsurf/skills/<name>/SKILL.md",
            "Reads AGENTS.md natively",
            "MCP synced to .windsurf/mcp.json (project-level)",
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
            "Skills synced to .roo/skills/<name>/SKILL.md",
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
            "Agents synced to .amazonq/cli-agents/<name>.json (JSON format)",
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
        "AGENTS.md (native) + .agents/skills/",
        &[
            "Config at ~/.codex/config.toml",
            "MCP is global-only (~/.codex/config.toml, TOML format)",
            "Skills synced to .agents/skills/<name>/SKILL.md",
        ],
    );
    print_tool(
        "OpenCode",
        "opencode",
        "AGENTS.md (native) + opencode.json",
        &[
            "Skills synced to .opencode/skills/<name>/SKILL.md",
            "MCP synced to .opencode/mcp.json (OpenCode format: type:local)",
            "Agents synced to .opencode/agents.json (mode:subagent)",
            "Also scans .claude/skills/, .agents/skills/",
        ],
    );
    print_tool(
        "Gemini CLI",
        "gemini",
        "GEMINI.md + .gemini/settings.json",
        &[
            "Hierarchical: ~/.gemini/GEMINI.md → project → subdirs",
            "Skills synced to .gemini/skills/<name>/SKILL.md (name + description only)",
            "Agents synced to .gemini/agents/<name>.md (kind:local frontmatter)",
            "MCP synced to .gemini/settings.json (Gemini format: no type field, httpUrl for HTTP)",
            "Supports @file.md imports",
        ],
    );
    print_tool(
        "Zed AI",
        "zed",
        ".rules + .zed/settings.json",
        &[
            "Fallback chain: .rules → .cursorrules → .windsurfrules → .clinerules → .github/copilot-instructions.md → AGENT.md → AGENTS.md → CLAUDE.md → GEMINI.md",
            "MCP synced to .zed/settings.json (context_servers format)",
            "Single .rules file, no frontmatter",
        ],
    );
    print_tool(
        "Amp (Sourcegraph)",
        "amp",
        "AGENTS.md (native), falls back to AGENT.md or CLAUDE.md",
        &[
            "Skills synced to .agents/skills/<name>/SKILL.md (shared format)",
            "MCP synced to .amp/settings.json (standard mcpServers format)",
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
