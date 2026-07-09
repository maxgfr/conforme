# Claude Code

> Anthropic's CLI agent. Source: `--from claude`

## Official docs

- Overview: https://code.claude.com/docs/en/overview
- Rules (CLAUDE.md + .claude/rules/): https://code.claude.com/docs/en/memory
- Skills: https://code.claude.com/docs/en/skills
- Subagents: https://code.claude.com/docs/en/sub-agents
- MCP servers: https://code.claude.com/docs/en/mcp
- Hooks: https://code.claude.com/docs/en/hooks
- Hooks guide: https://code.claude.com/docs/en/hooks-guide
- Settings: https://code.claude.com/docs/en/settings
- Changelog: https://code.claude.com/docs/en/changelog

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `CLAUDE.md` | Markdown (always-active rules inlined) |
| Rules (glob) | `.claude/rules/*.md` | YAML frontmatter: `paths` (glob array) |
| Skills | `.claude/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description`, `allowed-tools` |
| Commands | `.claude/commands/*.md` | YAML frontmatter: `description`, `allowed-tools`, `model` |
| Agents | `.claude/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model`, `tools`, `color`, `permissionMode` |
| MCP | `.mcp.json` | JSON: `{ "mcpServers": { "<name>": { "type": "stdio", "command", "args" } } }` |
| Hooks | `.claude/settings.json` | JSON: `{ "hooks": { "PreToolUse": [...], "PostToolUse": [...] } }` |
| Settings | `.claude/settings.json` | JSON: `{ "permissions": { "allow": [...], "deny": [...] }, "model": "sonnet" }` |

## Activation modes

| Mode | Implementation |
|------|---------------|
| Always | No frontmatter, content in CLAUDE.md |
| GlobMatch | `.claude/rules/<name>.md` with `paths: [**/*.ts]` |
| AgentDecision | `.claude/rules/<name>.md` without `paths` (always loaded) |
| Manual | Same as AgentDecision (no native manual mode) |

## conforme adapter

- File: `src/adapters/claude.rs`
- ID: `claude`
- Capabilities: rules, skills, agents, MCP
- Read: CLAUDE.md + .claude/rules/ + .claude/skills/ + .claude/commands/ + .claude/agents/ + .mcp.json
- Write: CLAUDE.md + .claude/rules/ + .claude/skills/ + .claude/agents/ + .mcp.json

## Notes

- Commands (`.claude/commands/*.md`) are read as skills when Claude is source, propagated to other tools as SKILL.md
- Hooks and permissions are Claude-specific, not synced to other tools
- `allowed-tools` accepts a space- or comma-separated string (or a YAML list); conforme writes the space-separated form `"Read Bash Write"` and parses both on read
- Rules without `paths` frontmatter are always-active (no agent-decision/manual distinction)
- A project `CLAUDE.md` may also live at `./.claude/CLAUDE.md`; conforme reads/writes the root `./CLAUDE.md` (and still detects the project via a `.claude/` directory)
- MCP: `type: "stdio"` is optional in `.mcp.json` (transport is inferred from `command`). HTTP transport accepts `"http"` (and the `"streamable-http"` alias); the older `"sse"` transport is deprecated and the `"ws"` (WebSocket) transport is also parsed on read — conforme maps all remote transports to its HTTP variant (`url` + `headers`)
- `tools` (subagents) and `allowed-tools` (skills/commands) accept a space-separated string, a comma-separated string, or a YAML list; conforme parses all three forms on read
- Subagent `color` (`red`/`blue`/`green`/`yellow`/`purple`/`orange`/`pink`/`cyan`) and `permissionMode` (`default`/`acceptEdits`/`plan`/`bypassPermissions`) are preserved on the Claude read→write round-trip (and carried through AGENTS.md as `<!-- color: -->` / `<!-- permission-mode: -->` comments); they are Claude-specific and not mapped to other tools
