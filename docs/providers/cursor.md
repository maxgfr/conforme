# Cursor

> AI code editor with .mdc rule files. Source: `--from cursor`

## Official docs

- Rules: https://cursor.com/docs/rules
- Skills: https://cursor.com/docs/context/skills
- Changelog (v2.4 - skills/subagents): https://cursor.com/changelog/2-4
- Blog (agent best practices): https://cursor.com/blog/agent-best-practices
- Forum: https://forum.cursor.com

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.cursor/rules/*.mdc` | YAML frontmatter: `alwaysApply`, `globs`, `description` |
| Skills | `.cursor/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| Agents | `.cursor/agents/<name>.mdc` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| MCP | `.cursor/mcp.json` | JSON: `{ "mcpServers": { "<name>": { "type": "stdio", "command", "args" } } }` |

## Activation modes

| Mode | Frontmatter |
|------|------------|
| Always | `alwaysApply: true` |
| GlobMatch | `globs: "**/*.ts, **/*.tsx"` + `alwaysApply: false` |
| AgentDecision | `description: "..."` + `alwaysApply: false` |
| Manual | `alwaysApply: false` (no globs, no description) |

## conforme adapter

- File: `src/adapters/cursor.rs`
- ID: `cursor`
- Capabilities: activation_modes, skills, agents, MCP
- General instructions -> `general.mdc` with `alwaysApply: true`
- File extension is `.mdc` (not `.md`)

## Notes

- `.mdc` is Cursor's custom markdown format (same as `.md` with YAML frontmatter)
- Skills use the standard SKILL.md format with `name` and `description` only
- Cursor reads AGENTS.md natively as fallback
- MCP uses standard `mcpServers` JSON format with `type` field
