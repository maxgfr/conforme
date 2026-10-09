# Cursor

> AI code editor with .mdc rule files. Source: `--from cursor`

**Last verified online:** 2026-10-09, against Cursor 3.24.9 (https://cursor.com/changelog)

## Official docs

- Rules: https://cursor.com/docs/rules
- Skills: https://cursor.com/docs/skills
- Subagents: https://cursor.com/docs/subagents
- MCP: https://cursor.com/docs/mcp
- Changelog: https://cursor.com/changelog
- Changelog (v2.4 - skills/subagents): https://cursor.com/changelog/2-4
- Blog (agent best practices): https://cursor.com/blog/agent-best-practices
- Forum: https://forum.cursor.com
- Rules help (`.cursorrules` legacy, `CLAUDE.md` always applied, rules identified by path): https://cursor.com/help/customization/rules
- CLI: what it loads (`AGENTS.md`, `CLAUDE.md`, `.cursor/rules`, `mcp.json`): https://cursor.com/docs/cli/using
- CLI parameters (`agent mcp list`, `list-tools`, `enable`): https://cursor.com/docs/cli/reference/parameters
- Security and workspace trust: https://cursor.com/docs/agent/security
- CLI changelog (`user-invocable: false` in v2026.07.06; skill and subagent scans skip hidden dot-directories since v2026.08.11): https://cursor.com/docs/cli/changelog
- Customizing Cursor (index of rules, skills, subagents, hooks, plugins): https://cursor.com/docs/customize-cursor

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
- A glob rule is written with `globs` and a `description`; a rule with non-empty `globs` is file-scoped whatever its description, and conforme reads it that way
- `globs` is one comma-separated string ("Separate multiple patterns with commas"), so brace groups are expanded on write (`src/*.{ts,tsx}` → `src/*.ts, src/*.tsx`) and kept whole on read. On read a YAML list is accepted too; an empty `globs` scopes nothing, so the rule falls through to its `description` (agent-decision) or to manual
- An agent-decision rule always carries a `description` (the rule name when the source has none): without one Cursor would treat it as manual
- `model` value is `inherit` or a Cursor model ID, optionally with `[key=value]` parameters (`fast` is no longer a model value). conforme leaves out another host's alias (`sonnet`, `opus`, `haiku`, `fable`, `pro`, `flash`, `flash-lite`) and any `provider/model` id, and copies other values as they are. Cursor documents a fallback to "a compatible model" when a model is blocked or unavailable, but not what happens with an id it does not know (unverified)
- Skills use the standard SKILL.md format; `name` and `description` are required, and `paths`, `disable-model-invocation`, `icon`, `color` and `metadata` are optional (https://cursor.com/docs/skills). conforme emits `name` + `description`, and for a manual skill `disable-model-invocation: true` plus `metadata: {opencode/autoinvoke: "false"}`, both keys Cursor documents. It does not write `paths` (skill scoping) nor `user-invocable` (CLI v2026.07.06, https://cursor.com/docs/cli/changelog)
- Cursor discovers skills from `.cursor/skills/` and `.agents/skills/` and, for compatibility, `.claude/skills/` and `.codex/skills/`; subagents likewise from `.claude/agents/` and `.codex/agents/`. conforme writes the Cursor-native `.cursor/` locations, so a skill synced to several of those directories (Claude Code, Codex) may be loaded twice by Cursor
- With Cursor as the source conforme reads only `.cursor/skills/`, and `gitignore install` with `source = "cursor"` ignores the `.agents/skills/` Codex output although Cursor loads it (known gaps, design pending)
- A root `.cursorrules` makes conforme detect Cursor and, with Cursor as the source, is read into the instructions after `general.mdc`: Cursor calls it "legacy and will be deprecated" (https://cursor.com/help/customization/rules) but still loads it as an always-applied rule (Cursor CLI 2026.10.01, `loadCursorRulesRule` in its shipped `index.js`: `frontmatter:{alwaysApply:!0}`). It is a `source_files()` entry, so no target writes it and `remove`/`migrate` never delete it
- Cursor also applies a root `CLAUDE.md` to every conversation, "regardless of any `alwaysApply`": with the Claude Code target synced too, the instructions reach Cursor twice (in `CLAUDE.md` and in `general.mdc`). Cursor identifies rules by their full path, so same-named rules in different folders all apply; conforme writes them back flat, one file per name
- Cursor walks the skills root recursively; conforme reads nested skill folders too (written back flat)
- Cursor reads `AGENTS.md` natively at the root and in subdirectories, combined with the parent ones; it is loaded alongside rules, not as a fallback
- MCP uses the standard `mcpServers` JSON format. Local servers use `type: "stdio"` + `command`/`args`; per Cursor's MCP docs, remote servers need only `url` (+ optional `headers`/`auth`) and omit `type`, which is what conforme writes. `env` is written on stdio servers only
- `.cursor/mcp.json` is merged, not owned: per-server keys conforme never writes (`auth`, `envFile`) survive, and `remove`/`migrate` keep the file
- Cursor's interpolation is `${env:NAME}` (plus `${workspaceFolder}`, `${userHome}`, …). conforme writes `${env:VAR}` for a normalized `${VAR}` (a `:-default` is dropped) and reads `${env:VAR}` back as `${VAR}`; Cursor's predefined variables are left as they are
