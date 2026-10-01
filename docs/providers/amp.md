# Amp

> Amp's AI coding agent (Amp Frontier Corporation, spun out of Sourcegraph on 2025-12-02). Source: `--from amp`

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
- CLI settings: https://ampcode.com/docs/cli/settings
- Plugins: https://ampcode.com/docs/customize/plugins
- Spin-out announcement: https://ampcode.com/news/amp-frontier-corporation

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native) | Markdown |
| Skills | `.agents/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` (shared Codex format) |
| MCP | `.amp/settings.json` or `.amp/settings.jsonc` | JSON(C): `{ "amp.mcpServers": { ... } }` |

## Activation modes

No activation modes. Reads AGENTS.md natively (all content always-on).
Also reads `AGENT.md` (singular) as fallback.

## conforme adapter

- File: `src/adapters/amp.rs`
- ID: `amp`
- Capabilities: skills, MCP
- No activation modes, no agents
- Skills use `.agents/skills/` (shared format with Codex)
- `read()` round-trips AGENTS.md plus skills (`.agents/skills/`) and MCP (`.amp/settings.json`); the file is parsed with conforme's AGENTS.md convention (instructions plus `## Rule:` sections), and its `## Skill:`/`## Agent:`/`## MCP:` sections are added to what this tool's own files hold (which win on a name clash). With this tool as the source, AGENTS.md *is* the source: sync never regenerates it (`generate_agents_md` does not apply) and `gitignore install` keeps it tracked

## Notes

- Skills can bundle MCP servers via `mcp.json` in skill directory
- Skills support `includeTools` with glob patterns to filter exposed tools
- Amp spawns subagents internally via the Task tool; custom subagents exist only as TypeScript plugins (`amp.createAgent`), not agent files, so conforme syncs no agents here
- Settings at `.amp/settings.json` under `amp.mcpServers` key. That file is Amp's whole workspace settings blob, so conforme **merges** the `amp.mcpServers` key into any existing file rather than overwriting it, and `remove amp` / `migrate --source amp` leave the file in place. The merge is JSONC-aware (comments outside the key survive, an unparsable file is left untouched) and keeps per-server keys conforme never emits
- Workspace MCP servers must be approved with `amp mcp approve` before Amp starts them; precedence is CLI flag > workspace > user > skills
- No `type` field in MCP entries — transport is inferred from the shape: stdio uses `command`/`args`/`env`, remote uses `url` (+ optional `headers`); conforme writes `env` on stdio servers only
- Workspace settings are "the nearest .amp/settings.json or .amp/settings.jsonc"; conforme merges into `settings.jsonc` when only that one exists (never creating a second file beside it), and treats both names as shared
- Environment references are `${VAR_NAME}` with no default form; a normalized `${VAR:-default}` is written as `${VAR}`
- User settings live at `~/.config/amp/settings.json`; workspace settings are the nearest `.amp/settings.json` searched upward
- Falls back to `AGENT.md` or `CLAUDE.md` if `AGENTS.md` not found
- Amp's docs moved from `ampcode.com/manual` to `ampcode.com/docs` (the old paths 301-redirect); the manual is now split into per-topic pages under `/docs/customize/`
- Skill discovery is first-match-by-name across: `~/.config/agents/skills/`, `~/.agents/skills/`, `~/.config/amp/skills/`, the project `.agents/skills/` and `.claude/skills/` (searched in the current directory and its parents), `~/.claude/skills/`, `~/.claude/plugins/cache/`, `amp.skills.path`, built-in skills, plugin-bundled `<plugin>:<skill>` skills, and the personal and workspace skill repositories (added 2026-08-11); `amp.skills.disableClaudeCodeSkills` / `disableGlobalAgentsSkills` turn some roots off. conforme writes the project `.agents/skills/`
- Each skills root is searched "recursively, up to five directories below"; conforme's `read()` does the same (nested skills are written back flat)
