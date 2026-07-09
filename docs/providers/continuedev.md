# Continue.dev

> Open-source AI coding assistant. Source: `--from continue`

## Official docs

- Rules (deep dive): https://docs.continue.dev/customize/deep-dives/rules
- Rules (overview): https://docs.continue.dev/customize/rules
- MCP (deep dive): https://docs.continue.dev/customize/deep-dives/mcp
- MCP tools: https://docs.continue.dev/customize/mcp-tools
- Agent mode: https://docs.continue.dev/ide-extensions/agent/how-to-customize
- Config reference: https://docs.continue.dev/reference
- Customization overview: https://docs.continue.dev/customize/overview
- Changelog: https://changelog.continue.dev/

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.continue/rules/*.md` | YAML frontmatter: `name`, `globs` (array), `alwaysApply`, `description` |
| MCP | `.continue/mcpServers/mcp.json` | JSON: `{ "mcpServers": { ... } }` |

## Activation modes

| Mode | Frontmatter |
|------|------------|
| Always | `alwaysApply: true` |
| GlobMatch | `globs: ["**/*.ts", "**/*.tsx"]` (YAML array, not comma-separated) |
| AgentDecision | `description: "..."` (agent decides based on description) |
| Manual | `alwaysApply: false` (no globs, no description) |

## conforme adapter

- File: `src/adapters/continuedev.rs`
- ID: `continue`
- Capabilities: activation_modes, MCP
- No skills support, no agents support
- General instructions -> `general.md` with `alwaysApply: true` + `name: General`
- `name` field is REQUIRED in frontmatter

## Notes

- `globs` uses YAML array format `["**/*.ts"]`, not comma-separated string
- Has a `regex` frontmatter field for content-based matching (not yet supported by conforme)
- A Continue "agent" is the whole assistant (models + rules + MCP tools) defined in `config.yaml`, not a per-file subagent — Continue has no `.continue/agents/*.md` directory, so conforme sets `agents: false`
- MCP: written to `.continue/mcpServers/mcp.json`. Continue only accepts the `stdio`, `sse`, and `streamable-http` transport types — it does **not** recognize a bare `http`, so conforme emits `type: "streamable-http"` for HTTP servers. Can also be configured via `config.yaml` under a `mcpServers:` block
- Rules can have `regex` field for content-based pattern matching
