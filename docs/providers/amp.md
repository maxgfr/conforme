# Amp (Sourcegraph)

> Sourcegraph's AI coding agent. Source: `--from amp`

## Official docs

- Owner's Manual: https://ampcode.com/manual
- AGENTS.md spec: https://ampcode.com/news/AGENT.md
- AGENTS.md canonical: https://ampcode.com/AGENT.md
- Globs in AGENTS.md: https://ampcode.com/news/globs-in-AGENTS.md
- Skills with MCP lazy loading: https://ampcode.com/news/lazy-load-mcp-with-skills
- Workspace settings: https://ampcode.com/news/cli-workspace-settings
- How to build an agent: https://ampcode.com/notes/how-to-build-an-agent
- News/changelog: https://ampcode.com/news
- SDK: https://ampcode.com/manual/sdk

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native) | Markdown |
| Skills | `.agents/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` (shared Codex format) |
| MCP | `.amp/settings.json` | JSON: `{ "mcpServers": { ... } }` (standard format) |

## Activation modes

No activation modes. Reads AGENTS.md natively (all content always-on).
Also reads `AGENT.md` (singular) as fallback.

## conforme adapter

- File: `src/adapters/amp.rs`
- ID: `amp`
- Capabilities: skills, MCP
- No activation modes, no agents
- Skills use `.agents/skills/` (shared format with Codex)

## Notes

- Skills can bundle MCP servers via `mcp.json` in skill directory
- Skills support `includeTools` with glob patterns to filter exposed tools
- Amp has custom commands in `.agents/commands/<name>.md` (not synced by conforme)
- Amp spawns subagents internally via Task tool but does not support user-defined agent files
- Settings at `.amp/settings.json` under `amp.mcpServers` key
- Falls back to `AGENT.md` or `CLAUDE.md` if `AGENTS.md` not found
