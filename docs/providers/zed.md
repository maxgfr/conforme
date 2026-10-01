# Zed AI

> High-performance editor with AI agent. Source: `--from zed`

## Official docs

- Rules: https://zed.dev/docs/ai/instructions
- Skills: https://zed.dev/docs/ai/skills
- MCP (context servers): https://zed.dev/docs/ai/mcp
- MCP extensions: https://zed.dev/docs/extensions/mcp-extensions
- AI configuration: https://zed.dev/docs/ai/quick-start
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
  - `.zed/settings.json` holds the user's entire Zed configuration, so conforme **merges** the `context_servers` key into any existing file rather than overwriting it, and `remove zed` / `migrate --source zed` leave the file in place
  - Zed settings are JSONC. The merge edits only `context_servers` in place, so comments and trailing commas elsewhere survive; per-server keys conforme never emits are kept, and a file conforme cannot parse is left untouched (sync fails). A hand-set `"enabled": false` is reset, so `check` never passes while Zed hides a synced server
  - Zed's remote variant has no `env`, so `env` is written on stdio servers only
  - Zed expands no variable references (none documented or found in its source): values are copied as they are, so a `${VAR}` from another tool stays literal
- Project skills and project MCP servers only take effect in a trusted worktree (project MCP servers auto-start only when the project is trusted)
- Zed has "Agent Profiles" but configured via settings, not project files
- The old `zed.dev/docs/ai/rules` page is gone (404): as of Zed v1.4.0 reusable rules were replaced by **Skills** and always-on rules by **Instructions**. `.rules` itself still works as a project instruction file — it is just documented on the Instructions page now
- Personal instructions live at `~/.config/zed/AGENTS.md` (macOS/Linux) or `%APPDATA%\Zed\AGENTS.md`; project instruction files override them
- Zed **skills** are a documented feature: `SKILL.md` folders under `<worktree>/.agents/skills/` (project) and `~/.agents/skills/` (global), only one level deep. conforme syncs skills to the shared project `.agents/skills/` path (the same location Codex/Amp use) with `name` + `description` frontmatter; names are kebab-case ASCII of at most 64 characters, as Zed requires
- Zed plans to deprecate MCP server extensions in favour of the official MCP registry; `context_servers` entries in settings are unaffected
- An empty config generates no `.rules` file at all (no blank file is dropped into the repository)
