# Windsurf

> Codeium's AI IDE (Cascade). Source: `--from windsurf`

## Official docs

- Rules/Memories: https://docs.devin.ai/desktop/cascade/memories
- AGENTS.md: https://docs.devin.ai/desktop/cascade/agents-md
- Skills: https://docs.devin.ai/desktop/cascade/skills
- MCP: https://docs.devin.ai/desktop/cascade/mcp
- Hooks: https://docs.devin.ai/desktop/cascade/hooks
- Workflows: https://docs.devin.ai/desktop/cascade/workflows
- Changelog: https://docs.devin.ai/desktop/changelog

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.devin/rules/*.md` (preferred) or `.windsurf/rules/*.md` (legacy) | YAML frontmatter: `trigger`, `description`, `globs` |
| Skills | `.windsurf/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| MCP | `.windsurf/mcp.json` (project, best-effort) / `~/.codeium/windsurf/mcp_config.json` (global) | JSON: `{ "mcpServers": { ... } }` — stdio uses `command`/`args`, HTTP uses `serverUrl`, no `type` field |

## Activation modes

| Mode | Frontmatter |
|------|------------|
| Always | `trigger: always_on` |
| GlobMatch | `trigger: glob` + `globs: "**/*.ts, **/*.tsx"` |
| AgentDecision | `trigger: model_decision` + `description: "..."` |
| Manual | `trigger: manual` |

## conforme adapter

- File: `src/adapters/windsurf.rs`
- Detection: `.windsurf/`, `.devin/`, or a root `.windsurfrules`
- ID: `windsurf`
- Capabilities: activation_modes, skills, MCP
- No agents support
- General instructions -> `general.md` with `trigger: always_on`
- `read()` round-trips rules plus skills (`.windsurf/skills/`) and MCP (`.windsurf/mcp.json`)

## Notes

- Character limits: the global rules file (`~/.codeium/windsurf/memories/global_rules.md`) is limited to 6,000 characters; workspace rules (`.windsurf/rules/*.md`) are limited to 12,000 characters **per file**
- Skills added in Wave 1.13.107 (January 2026)
- No agents/subagents support
- Reads AGENTS.md natively
- Windsurf's canonical MCP config is user-global at `~/.codeium/windsurf/mcp_config.json`; conforme additionally writes a project-level `.windsurf/mcp.json` as a best-effort. HTTP servers use `serverUrl` (not `url`) and no `type` field is emitted
- Windsurf also has hooks (cascade hooks) and workflows but they are tool-specific, not synced
- Following Windsurf's acquisition by Cognition, the docs now live at `docs.devin.ai/desktop/cascade/*` (the old `docs.windsurf.com/*` URLs 307-redirect there). The preferred rules path is `.devin/rules/*.md`, which takes precedence, with `.windsurf/rules/*.md` (and root `.windsurfrules`) supported as a legacy fallback. conforme follows the same precedence: it writes and reads `.devin/rules/` when the project has a `.devin/` directory, and keeps `.windsurf/rules/` otherwise. After a migration to `.devin/`, the legacy `.windsurf/rules/` directory is also cleaned of orphans so Cascade never sees two divergent copies of the same rule set
- Skills and MCP stay under `.windsurf/` — the Cascade docs still document `.windsurf/skills/<name>/SKILL.md` and `~/.codeium/windsurf/mcp_config.json`
- The documented `globs` example is a single pattern (`globs: **/*.test.ts`); conforme writes multiple patterns as a comma-separated string, the historical Windsurf form
