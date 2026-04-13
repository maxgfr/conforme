# Zed AI

> High-performance editor with AI agent. Source: `--from zed`

## Official docs

- Rules: https://zed.dev/docs/ai/rules
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
| MCP | `.zed/settings.json` | JSON: `{ "context_servers": { "<name>": { "command", "args" } } }` |

## Activation modes

No activation modes. Single `.rules` file, all content always-on.

Fallback chain: `.rules` -> `.cursorrules` -> `.windsurfrules` -> `.clinerules` -> `.github/copilot-instructions.md` -> `AGENT.md` -> `AGENTS.md` -> `CLAUDE.md` -> `GEMINI.md`

## conforme adapter

- File: `src/adapters/zed.rs`
- ID: `zed`
- Capabilities: MCP only
- No activation modes, no skills, no agents
- All rules merged into single `.rules` file

## Notes

- **MCP format is unique:**
  - Uses `"context_servers"` key (not `"mcpServers"`)
  - No `"type"` field
- Zed has "Agent Profiles" but configured via settings, not project files
- Skills support is planned (GitHub issue #49057) but not yet implemented
- Empty config generates `.rules` with just `\n`
