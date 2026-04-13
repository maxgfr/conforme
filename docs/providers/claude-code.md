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
| Agents | `.claude/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model`, `tools` |
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
- `allowed-tools` uses space-separated format: `"Read Bash Write"`
- Rules without `paths` frontmatter are always-active (no agent-decision/manual distinction)
