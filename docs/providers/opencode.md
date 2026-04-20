# OpenCode

> Open-source AI CLI tool. Source: `--from opencode`

## Official docs

- Rules: https://opencode.ai/docs/rules/
- Skills: https://opencode.ai/docs/skills/
- Agents: https://opencode.ai/docs/agents/
- MCP servers: https://opencode.ai/docs/mcp-servers/
- Config: https://opencode.ai/docs/config/
- CLI: https://opencode.ai/docs/cli/
- Commands: https://opencode.ai/docs/commands/
- Tools: https://opencode.ai/docs/tools/
- Permissions: https://opencode.ai/docs/permissions/

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native) | Markdown |
| Skills | `.opencode/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| Agents | `opencode.json` (`agent` key) + `.opencode/agents/<name>.md` | JSON + Markdown frontmatter |
| MCP | `opencode.json` (`mcp` key) | JSON: `{ "mcp": { "<name>": { "type": "local", "command": ["cmd", "...args"], "environment": {...} } } }` |

## Activation modes

No activation modes. Reads AGENTS.md natively (all content always-on).

## conforme adapter

- File: `src/adapters/opencode.rs`
- ID: `opencode`
- Capabilities: skills, agents, MCP
- No activation modes
- Reads AGENTS.md natively, falls back to CLAUDE.md
- Writes MCP and agent definitions into `opencode.json` at the project root, preserving any existing keys

## Notes

- **MCP format is unique:** uses `"mcp"` key (not `"mcpServers"`), `type: local/remote` (not `stdio/http`)
- **Command is a single array:** `"command": ["npx", "-y", "server-name"]` (no separate `args` field)
- **Env var key is `"environment"`** (not `"env"`)
- MCP and `agent` blocks live inside `opencode.json` (or `opencode.jsonc`) at the project root. conforme merges into any existing `opencode.json` so user-authored keys are preserved
- Markdown agents additionally emitted to `.opencode/agents/<name>.md` (OpenCode discovers per-project agents from that directory)
- Skill frontmatter recognizes only `name`, `description`, `license`, `compatibility`, `metadata` — no `allowed-tools`
- Skills also discovered from `.agents/skills/` and `.claude/skills/`
