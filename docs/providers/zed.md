# Zed AI

> High-performance editor with AI agent. Source: `--from zed`

## Official docs

- Rules: https://zed.dev/docs/ai/rules
- Skills: https://zed.dev/docs/ai/skills
- MCP (context servers): https://zed.dev/docs/ai/mcp
- MCP extensions: https://zed.dev/docs/extensions/mcp-extensions
- AI configuration: https://zed.dev/docs/ai/configuration
- Agent panel: https://zed.dev/docs/ai/agent-panel
- Agent settings: https://zed.dev/docs/ai/agent-settings
- External agents: https://zed.dev/docs/ai/external-agents
- Tool permissions: https://zed.dev/docs/ai/tool-permissions
- All settings: https://zed.dev/docs/reference/all-settings

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.rules` | Single plain markdown file (no frontmatter) |
| Skills | `.agents/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` (shared `.agents/skills/` location) |
| MCP | `.zed/settings.json` | JSON: `{ "context_servers": { "<name>": { "command", "args" } } }` |

## Activation modes

No activation modes. Single `.rules` file, all content always-on.

Fallback chain: `.rules` -> `.cursorrules` -> `.windsurfrules` -> `.clinerules` -> `.github/copilot-instructions.md` -> `AGENT.md` -> `AGENTS.md` -> `CLAUDE.md` -> `GEMINI.md`

## conforme adapter

- File: `src/adapters/zed.rs`
- ID: `zed`
- Capabilities: skills, MCP
- No activation modes, no agents
- All rules merged into single `.rules` file
- Skills synced to the shared `.agents/skills/<name>/SKILL.md` location; `read()` reads them back

## Notes

- **MCP format is unique:**
  - Uses `"context_servers"` key (not `"mcpServers"`)
  - No `"type"` field
  - Flat shape: stdio uses `command`/`args`/`env`, remote uses `url`/`headers` (no `source` wrapper, no nested command object — that is an older, superseded Zed schema)
  - `.zed/settings.json` holds the user's entire Zed configuration, so conforme **merges** the `context_servers` key into any existing file rather than overwriting it
- Zed has "Agent Profiles" but configured via settings, not project files
- Zed **skills** are a documented feature: `SKILL.md` folders under `<worktree>/.agents/skills/` (project) and `~/.agents/skills/` (global). conforme syncs skills to the shared project `.agents/skills/` path (the same location Codex/Amp use) with `name` + `description` frontmatter
- Empty config generates `.rules` with just `\n`
