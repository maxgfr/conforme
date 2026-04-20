# Windsurf

> Codeium's AI IDE (Cascade). Source: `--from windsurf`

## Official docs

- Rules/Memories: https://docs.windsurf.com/windsurf/cascade/memories
- AGENTS.md: https://docs.windsurf.com/windsurf/cascade/agents-md
- Skills: https://docs.windsurf.com/windsurf/cascade/skills
- MCP: https://docs.windsurf.com/windsurf/cascade/mcp
- Hooks: https://docs.windsurf.com/windsurf/cascade/hooks
- Workflows: https://docs.windsurf.com/windsurf/cascade/workflows
- Changelog: https://windsurf.com/changelog

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.windsurf/rules/*.md` | YAML frontmatter: `trigger`, `description`, `globs` |
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
- ID: `windsurf`
- Capabilities: activation_modes, skills, MCP
- No agents support
- General instructions -> `general.md` with `trigger: always_on`

## Notes

- Character limits: 6,000 per rule, 12,000 total across active rules
- Skills added in Wave 1.13.107 (January 2026)
- No agents/subagents support
- Reads AGENTS.md natively
- Windsurf's canonical MCP config is user-global at `~/.codeium/windsurf/mcp_config.json`; conforme additionally writes a project-level `.windsurf/mcp.json` as a best-effort. HTTP servers use `serverUrl` (not `url`) and no `type` field is emitted
- Windsurf also has hooks (cascade hooks) and workflows but they are tool-specific, not synced
