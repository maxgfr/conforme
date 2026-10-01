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
            "Frontmatter: paths (glob array; a comma-separated string is also read)",
            "Always rules → embedded in CLAUDE.md",
            "Glob rules → .claude/rules/{name}.md with paths: frontmatter",
            "Commands (.claude/commands/**/*.md) → synced as skills to other tools",
            "Skills synced to .claude/skills/<name>/SKILL.md, agents to .claude/agents/<name>.md (Claude tool names and models only)",
            "MCP merged into .mcp.json (`mcpServers`, ${VAR} references; per-server options kept)",
            "Reads AGENTS.md natively only when no CLAUDE.md exists (conforme writes CLAUDE.md)",
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
            "Subagents synced to .cursor/agents/<name>.md (plain .md, no tools field, lowercase-hyphen name)",
            "MCP merged into .cursor/mcp.json (no `type` on remote servers, ${env:VAR} references)",
            "Reads AGENTS.md natively",
        ],
    );
    print_tool(
        "Devin Desktop (formerly Windsurf)",
        "devin",
        ".devin/rules/*.md",
        &[
            "Frontmatter: trigger (always_on|glob|model_decision|manual; `agent` is read as model_decision), description, globs",
            "Legacy .windsurf/rules/ and .windsurf/skills/ are read too, and conforme's old copies there are cleaned",
            "Skills synced to .devin/skills/<name>/SKILL.md (`triggers: [user]` for manual skills)",
            "MCP merged into .devin/mcp_config.json (no `type`, ${env:VAR} references)",
            "Reads AGENTS.md natively",
        ],
    );
    print_tool(
        "GitHub Copilot",
        "copilot",
        ".github/copilot-instructions.md + .github/instructions/*.instructions.md",
        &[
            "Frontmatter: applyTo (glob string), excludeAgent (optional)",
            "Glob rules → .github/instructions/{name}.instructions.md",
            "Skills synced to .github/skills/<name>/SKILL.md",
            "Agents synced to .github/agents/<name>.agent.md",
            "MCP merged into .vscode/mcp.json (`servers` key, ${env:VAR} references; VS Code `inputs`/`sandbox` kept)",
            "Reads AGENTS.md, CLAUDE.md, and GEMINI.md natively",
        ],
    );
    print_tool(
        "Kiro (AWS)",
        "kiro",
        ".kiro/steering/*.md",
        &[
            "Frontmatter: inclusion (always|fileMatch|auto|manual), fileMatchPattern, name, description",
            "Successor to Amazon Q CLI",
            "Skills synced to .kiro/skills/<name>/SKILL.md",
            "Agents synced to .kiro/agents/<name>.md with tools translated to Kiro tools and tags (read, grep, shell, @server/tool, …)",
            "MCP merged into .kiro/settings/mcp.json (autoApprove/disabledTools kept)",
            "Reads AGENTS.md natively",
        ],
    );
    print_tool(
        "Zoo Code (community fork of Roo Code)",
        "zoocode",
        ".roo/rules/*.md",
        &[
            "Plain Markdown — NO YAML frontmatter",
            "Files loaded alphabetically (use numeric prefixes: 00-, 01-)",
            "Skills synced to .roo/skills/<name>/SKILL.md",
            "Mode-specific rules in .roo/rules-{mode}/",
            "MCP merged into .roo/mcp.json (streamable-http for HTTP; alwaysAllow/disabledTools kept)",
            "Reads AGENTS.md natively",
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
        "AGENTS.md (native) + .agents/skills/ + .codex/config.toml",
        &[
            "Config at ~/.codex/config.toml (global) or .codex/config.toml (project)",
            "MCP merged into project .codex/config.toml as [mcp_servers.<name>] tables",
            "Skills synced to .agents/skills/<name>/SKILL.md",
            "Manual skills preserve their Codex agents/openai.yaml invocation policy",
        ],
    );
    print_tool(
        "OpenCode",
        "opencode",
        "AGENTS.md (native) + opencode.json",
        &[
            "Skills synced to .opencode/skills/<name>/SKILL.md (name, description, and manual invocation metadata)",
            "MCP merged into opencode.json under `mcp` key (type:local/remote, command as single array, key `environment`)",
            "Agents merged into opencode.json under `agent` key + per-agent .opencode/agents/<name>.md (model only as provider/model)",
            "Also scans .claude/skills/, .agents/skills/",
        ],
    );
    print_tool(
        "Gemini CLI",
        "gemini",
        "GEMINI.md + .gemini/settings.json",
        &[
            "Hierarchical: ~/.gemini/GEMINI.md → project → subdirs",
            "Skills synced to .gemini/skills/<name>/SKILL.md (name, description, and manual invocation metadata)",
            "Agents synced to .gemini/agents/<name>.md (kind:local frontmatter, tools translated to Gemini names)",
            "MCP merged into .gemini/settings.json (Gemini format: no type field, httpUrl for HTTP)",
            "Supports @file.md imports",
        ],
    );
    print_tool(
        "Zed AI",
        "zed",
        ".rules + .zed/settings.json",
        &[
            "Fallback chain: .rules → .cursorrules → .windsurfrules → .clinerules → .github/copilot-instructions.md → AGENT.md → AGENTS.md → CLAUDE.md → GEMINI.md",
            "MCP merged into .zed/settings.json (context_servers format, preserves existing settings)",
            "Skills synced to shared .agents/skills/<name>/SKILL.md",
            "Single .rules file, no frontmatter",
        ],
    );
    print_tool(
        "DeepSeek Harness (dsh)",
        "deepseek",
        "AGENTS.md (native), falls back to CLAUDE.md",
        &[
            "Default instructionFileCandidates: AGENTS.md then CLAUDE.md",
            "Local overlay candidates: AGENTS.local.md then CLAUDE.local.md",
            "Skills synced to .dsh/skills/<name>/SKILL.md (name + description, kebab-case names); flat <name>.md skills are read and never deleted",
            "Also scans the shared .agents/skills/ root",
            "MCP lives in the user-level cordis.patch.yml ($DSH_HOME) — not project-scoped",
        ],
    );
    print_tool(
        "Amp",
        "amp",
        "AGENTS.md (native), falls back to AGENT.md or CLAUDE.md",
        &[
            "Skills synced to .agents/skills/<name>/SKILL.md (shared format)",
            "MCP merged into .amp/settings.json (or .amp/settings.jsonc) under `amp.mcpServers` (preserves existing settings)",
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
        "Devin".underline(),
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
