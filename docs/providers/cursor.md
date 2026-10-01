# Cursor

> AI code editor with .mdc rule files. Source: `--from cursor`

## Official docs

- Rules: https://cursor.com/docs/rules
- Skills: https://cursor.com/docs/skills
- Subagents: https://cursor.com/docs/subagents
- MCP: https://cursor.com/docs/mcp
- Changelog: https://cursor.com/changelog
- Changelog (v2.4 - skills/subagents): https://cursor.com/changelog/2-4
- Blog (agent best practices): https://cursor.com/blog/agent-best-practices
- Forum: https://forum.cursor.com

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.cursor/rules/*.mdc` | YAML frontmatter: `alwaysApply`, `globs`, `description` |
| Skills | `.cursor/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| Agents | `.cursor/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model` (`tools` not recognized; tool access inherited) |
| MCP | `.cursor/mcp.json` (merged) | JSON: `{ "mcpServers": { "<name>": { "type": "stdio", "command", "args" } } }`; remote: `url` + `headers`, no `type` |

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
- Subagent names use lowercase letters and hyphens; conforme sanitizes the `name` it writes (`Code Reviewer` → `code-reviewer`)
- A glob rule is written with `globs` and a `description`; Cursor's docs do not say how a rule carrying both is classified (unverified)
- `globs` is one comma-separated string ("Separate multiple patterns with commas"), so brace groups are expanded on write (`src/*.{ts,tsx}` → `src/*.ts, src/*.tsx`) and kept whole on read
- An agent-decision rule always carries a `description` (the rule name when the source has none): without one Cursor would treat it as manual
- `model` value is `inherit` or a Cursor model ID, optionally with `[key=value]` parameters (`fast` is no longer a model value). conforme copies the source's `model` as is; Cursor's docs do not say what happens with an id it does not know (unverified)
- Skills use the standard SKILL.md format; `name` and `description` are required, and `paths`, `disable-model-invocation`, `icon`, `color` and `metadata` are optional (conforme emits only `name` + `description`)
- Cursor also discovers skills from `.agents/skills/` and, for compatibility, `.claude/skills/` and `.codex/skills/`; subagents likewise from `.claude/agents/` and `.codex/agents/`. conforme writes the Cursor-native `.cursor/` locations
- Cursor walks the skills root recursively; conforme reads nested skill folders too (written back flat)
- Cursor reads AGENTS.md natively as fallback
- MCP uses the standard `mcpServers` JSON format. Local servers use `type: "stdio"` + `command`/`args`; per Cursor's MCP docs, remote servers need only `url` (+ optional `headers`/`auth`) and omit `type`, which is what conforme writes. `env` is written on stdio servers only
- `.cursor/mcp.json` is merged, not owned: per-server keys conforme never writes (`auth`, `envFile`) survive, and `remove`/`migrate` keep the file
- Cursor's interpolation is `${env:NAME}` (plus `${workspaceFolder}`, `${userHome}`, …). conforme writes `${env:VAR}` for a normalized `${VAR}` (a `:-default` is dropped) and reads `${env:VAR}` back as `${VAR}`; Cursor's predefined variables are left as they are
