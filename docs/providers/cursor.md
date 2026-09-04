# Cursor

> AI code editor with .mdc rule files. Source: `--from cursor`

## Official docs

- Rules: https://cursor.com/docs/rules
- Skills: https://cursor.com/docs/skills
- Subagents: https://cursor.com/docs/subagents
- MCP: https://cursor.com/docs/mcp
- Changelog (v2.4 - skills/subagents): https://cursor.com/changelog/2-4
- Blog (agent best practices): https://cursor.com/blog/agent-best-practices
- Forum: https://forum.cursor.com

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.cursor/rules/*.mdc` | YAML frontmatter: `alwaysApply`, `globs`, `description` |
| Skills | `.cursor/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| Agents | `.cursor/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model` (`tools` not recognized; tool access inherited) |
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
- `read()` round-trips rules, skills (`.cursor/skills/`), subagents (`.cursor/agents/`), and MCP (`.cursor/mcp.json`)

## Notes

- `.mdc` is Cursor's custom markdown format used for **rules only** (same as `.md` with YAML frontmatter). A plain `.md` file in `.cursor/rules/` is ignored by Cursor
- `.cursor/rules/` may be organised in subdirectories; conforme reads nested `.mdc` rules too (written back flat, one file per rule name)
- Subagents use plain `.md` (not `.mdc`) — per Cursor's v2.4 docs
- Cursor subagents recognize only `name`, `description`, `model`, `readonly`, `is_background`. No `tools` field — tool access is inherited from the parent agent
- `model` value must be `inherit`, `fast`, or a Cursor-recognized model identifier
- Skills use the standard SKILL.md format; `name` and `description` are required, and `paths`, `disable-model-invocation`, `icon`, `color` and `metadata` are optional (conforme emits only `name` + `description`)
- Cursor also discovers skills from `.agents/skills/` and subagents from `.claude/agents/` and `.codex/agents/`; conforme writes the Cursor-native `.cursor/` locations
- Cursor reads AGENTS.md natively as fallback
- MCP uses the standard `mcpServers` JSON format. Local servers use `type: "stdio"` + `command`/`args`; per Cursor's MCP docs, remote servers need only `url` (+ optional `headers`/`auth`) and omit `type`. conforme emits a `type: "http"` field on remote servers, which Cursor ignores
