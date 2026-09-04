# Amp (Sourcegraph)

> Sourcegraph's AI coding agent. Source: `--from amp`

## Official docs

- Owner's Manual: https://ampcode.com/docs
- AGENTS.md spec: https://ampcode.com/news/AGENT.md
- AGENTS.md canonical: https://ampcode.com/docs/customize/agents-md
- Skills: https://ampcode.com/docs/customize/skills
- MCP: https://ampcode.com/docs/customize/mcp
- Globs in AGENTS.md: https://ampcode.com/news/globs-in-AGENTS.md
- Skills with MCP lazy loading: https://ampcode.com/news/lazy-load-mcp-with-skills
- Workspace settings: https://ampcode.com/news/cli-workspace-settings
- How to build an agent: https://ampcode.com/notes/how-to-build-an-agent
- News/changelog: https://ampcode.com/chronicle
- SDK: https://ampcode.com/docs/sdk

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native) | Markdown |
| Skills | `.agents/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` (shared Codex format) |
| MCP | `.amp/settings.json` | JSON: `{ "amp.mcpServers": { ... } }` |

## Activation modes

No activation modes. Reads AGENTS.md natively (all content always-on).
Also reads `AGENT.md` (singular) as fallback.

## conforme adapter

- File: `src/adapters/amp.rs`
- ID: `amp`
- Capabilities: skills, MCP
- No activation modes, no agents
- Skills use `.agents/skills/` (shared format with Codex)
- `read()` round-trips AGENTS.md plus skills (`.agents/skills/`) and MCP (`.amp/settings.json`)

## Notes

- Skills can bundle MCP servers via `mcp.json` in skill directory
- Skills support `includeTools` with glob patterns to filter exposed tools
- Amp has custom commands in `.agents/commands/<name>.md` (not synced by conforme)
- Amp spawns subagents internally via Task tool but does not support user-defined agent files
- Settings at `.amp/settings.json` under `amp.mcpServers` key. That file is Amp's whole workspace settings blob, so conforme **merges** the `amp.mcpServers` key into any existing file rather than overwriting it
- No `type` field in MCP entries — transport is inferred from the shape: stdio uses `command`/`args`, remote uses `url` (+ optional `headers`)
- User settings live at `~/.config/amp/settings.json`; workspace settings are the nearest `.amp/settings.json` searched upward
- Falls back to `AGENT.md` or `CLAUDE.md` if `AGENTS.md` not found
- Amp's docs moved from `ampcode.com/manual` to `ampcode.com/docs` (the old paths 301-redirect); the manual is now split into per-topic pages under `/docs/customize/`
- Skill discovery order puts `~/.config/agents/skills/`, `~/.agents/skills/` and `~/.config/amp/skills/` ahead of the project roots; conforme writes the project `.agents/skills/`, which Amp searches in the current directory and its parents
