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
    println!(
        "Generated from a tool source, AGENTS.md holds only instructions and rules: the tools \
         that load it get skills, agents and MCP servers in their own files."
    );
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
            "Legacy SSE servers written as type \"sse\", read back as SSE",
            "Reads AGENTS.md and .claude/AGENTS.md natively when no CLAUDE.md exists; conforme then reads both as Claude's source and never regenerates them",
        ],
    );
    print_tool(
        "Cursor",
        "cursor",
        ".cursor/rules/*.mdc",
        &[
            "Frontmatter: alwaysApply (bool), globs (string; a list is also read), description (string)",
            "4 rule types: Always, Auto Attached (globs), Agent Requested (description), Manual",
            "Skills synced to .cursor/skills/<name>/SKILL.md",
            "Subagents synced to .cursor/agents/<name>.md (plain .md, no tools field, lowercase-hyphen name)",
            "MCP merged into .cursor/mcp.json (no `type` on remote servers, ${env:VAR} references)",
            "Legacy SSE servers written as type \"sse\" (the IDE ignores type)",
            "Reads AGENTS.md natively (root and subdirectories)",
        ],
    );
    print_tool(
        "Devin Desktop (formerly Windsurf)",
        "devin",
        ".devin/rules/*.md",
        &[
            "Frontmatter: trigger (always_on|glob|model_decision|manual; `agent` is read as model_decision), description, globs",
            "Legacy .windsurf/rules/ and .windsurf/skills/ are read too, and conforme's old copies there are cleaned",
            "global_rules.md (.devin/, else .windsurf/) and the root .windsurfrules are read into the instructions",
            "Skills synced to .devin/skills/<name>/SKILL.md (`triggers: [user]` for manual skills)",
            "MCP merged into .devin/mcp_config.json (no `type`, ${env:VAR} references)",
            "Legacy SSE servers written as url + transport \"sse\"",
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
            "Detected from .github/copilot-instructions.md or a .github/instructions/, agents/ or skills/ directory",
            "MCP merged into .vscode/mcp.json (`servers` key, ${env:VAR} references; VS Code `inputs`/`sandbox` kept) and .github/mcp.json (`mcpServers`, read by Copilot CLI; the cloud agent's servers live in the repository settings on GitHub.com)",
            "Legacy SSE servers written as type \"sse\" in .vscode/mcp.json and .github/mcp.json",
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
            "Legacy SSE servers written as a bare url (no type), read back as streamable HTTP",
            "Reads AGENTS.md natively",
        ],
    );
    print_tool(
        "Zoo Code (community fork of Roo Code)",
        "zoocode",
        ".roo/rules/*.md",
        &[
            "Plain Markdown — NO YAML frontmatter",
            "Files loaded alphabetically (use numeric prefixes: 00-, 01-; a 2-3 digit prefix is stripped on read)",
            "Detected by .roo/ or .roorules (not .clinerules); .roorules is read when .roo/rules/ is empty",
            "Skills synced to .roo/skills/<name>/SKILL.md",
            "Mode-specific rules in .roo/rules-{mode}/",
            "MCP merged into .roo/mcp.json (streamable-http for HTTP; alwaysAllow/disabledTools kept)",
            "Legacy SSE servers written as type \"sse\" (read back as SSE)",
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
        "AGENTS.md (native) + .agents/skills/ + .codex/agents/ + .codex/config.toml",
        &[
            "Config at ~/.codex/config.toml (global) or .codex/config.toml (project)",
            "MCP merged into project .codex/config.toml as [mcp_servers.<name>] tables",
            "No ${VAR} expansion: references written as env_vars, bearer_token_env_var, env_http_headers",
            "Skills synced to .agents/skills/<name>/SKILL.md",
            "Manual skills preserve their Codex agents/openai.yaml invocation policy",
            "Agents synced to .codex/agents/<name>.toml (name, description, developer_instructions; loaded only in a trusted project; built-in default, worker and explorer are never written)",
            "Legacy SSE servers are written as streamable HTTP with a warning (Codex has no SSE transport)",
        ],
    );
    print_tool(
        "OpenCode",
        "opencode",
        "AGENTS.md (native) + .opencode/opencode.json",
        &[
            "Skills synced to .opencode/skills/<name>/SKILL.md (name, description, and manual invocation metadata)",
            "MCP merged into the project's opencode.json under `mcp` key (type:local/remote, command as single array, key `environment`, {env:VAR}): an existing root opencode.json(c), else .opencode/opencode.json, which Kilo Code does not read",
            "Legacy SSE servers written as type remote (read back as streamable HTTP)",
            "Agents merged into the same file under `agent` key + per-agent .opencode/agents/<name>.md (model only as provider/model)",
            "Agents read from .opencode/agents/, .opencode/agent/ and the `agent` key together (markdown wins); built-in agent names (build, plan, general, …) are neither written nor read",
            "Skills read from .opencode/skills/ and .opencode/skill/ only; OpenCode also loads .claude/skills/ and .agents/skills/, so a skill synced there too appears twice",
        ],
    );
    print_tool(
        "Gemini CLI",
        "gemini",
        "GEMINI.md + .gemini/settings.json",
        &[
            "Hierarchical: ~/.gemini/GEMINI.md → project → subdirs",
            "context.fileName in .gemini/settings.json is honoured on read; when it names AGENTS.md, that file is the source",
            "Instructions written to the first context.fileName entry other than AGENTS.md (GEMINI.md by default); none when it names only AGENTS.md",
            "Skills synced to .gemini/skills/<name>/SKILL.md (name, description, and manual invocation metadata)",
            "Agents synced to .gemini/agents/<name>.md (kind:local frontmatter, tools translated to Gemini names)",
            "MCP merged into .gemini/settings.json (Gemini format: httpUrl for HTTP)",
            "Legacy SSE servers written as url + type \"sse\" (read back as SSE)",
            "Supports @file.md imports",
        ],
    );
    print_tool(
        "Zed AI",
        "zed",
        ".rules + .zed/settings.json",
        &[
            "Fallback chain: .rules → .cursorrules → .windsurfrules → .clinerules → .github/copilot-instructions.md → AGENT.md → AGENTS.md → CLAUDE.md → GEMINI.md",
            "MCP merged into .zed/settings.json (context_servers format, preserves existing settings and extension servers)",
            "Legacy SSE servers written as streamable HTTP with a warning (Zed has no SSE transport)",
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
            "The shared .agents/skills/ root is read only when .dsh/skills/ has no skill",
            "MCP lives in the user-level cordis.patch.yml ($DSH_HOME) — not project-scoped",
        ],
    );
    print_tool(
        "Mistral Vibe",
        "vibe",
        "AGENTS.md (native) + .vibe/",
        &[
            "Project files load only in a folder trusted in ~/.vibe/trusted_folders.toml",
            "Skills synced to .vibe/skills/<name>/SKILL.md (the shared .agents/skills/ is read only when .vibe/skills/ has none); `vibe` and `skill-creator` are reserved names",
            "Subagents synced to .vibe/agents/<name>.toml (agent_type = \"subagent\", enabled_tools translated to Vibe tools); other agent files are the user's modes and are kept; a built-in name (ask, plan, accept-edits, smart-approve, auto-approve, explore, lean) is never written",
            "MCP merged into .vibe/config.toml as [[mcp_servers]] (stdio / streamable-http); `Authorization: Bearer ${VAR}` becomes a static auth reading VAR, any other ${VAR} is passed as written; a command Vibe would shell-split wrongly is written as a list",
            "Legacy SSE servers written as streamable-http with a warning (Vibe has no SSE transport)",
        ],
    );
    print_tool(
        "Kilo Code",
        "kilo",
        "AGENTS.md (native, else CLAUDE.md, else CONTEXT.md) + .kilo/",
        &[
            "OpenCode fork: the Kilo CLI and the VS Code extension read the same files",
            ".kilo/rules/*.md and the legacy .kilocode/ locations are read",
            "Skills synced to .kilo/skills/<name>/SKILL.md, subagents to .kilo/agents/<name>.md (OpenCode format; built-in names code, ask, debug, orchestrator, … skipped)",
            "MCP merged into .kilo/kilo.jsonc (or the existing kilo.json/kilo.jsonc) under `mcp`, OpenCode shape, with no variable reference: Kilo refuses {env:VAR} in a project config, a local server inherits the environment",
            "Legacy SSE servers written as type remote (read back as streamable HTTP)",
            "A root opencode.json holding {env:VAR} makes Kilo refuse the project config; conforme warns about it",
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
