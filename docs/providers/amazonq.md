# Amazon Q Developer

> AWS's AI coding assistant (IDE + CLI). Source: `--from amazonq`

## Official docs

- Project rules (IDE): https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/context-project-rules.html
- CLI user guide: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line.html
- Agent format reference (GitHub): https://github.com/aws/amazon-q-developer-cli/blob/main/docs/agent-format.md
- MCP overview: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/qdev-mcp.html
- MCP CLI config: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-mcp-config-CLI.html

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.amazonq/rules/*.md` | Plain markdown (NO frontmatter) |
| Agents | `.amazonq/cli-agents/<name>.json` | JSON: `{ "description", "model", "tools", "prompt", "resources", "useLegacyMcpJson" }` |
| MCP | `.amazonq/mcp.json` | JSON: `{ "mcpServers": { ... } }` (standard format) |

## Activation modes

No activation modes. All rules are plain markdown, auto-loaded. Users can toggle rules on/off per chat session via the UI.

## conforme adapter

- File: `src/adapters/amazonq.rs`
- ID: `amazonq`
- Capabilities: agents, MCP
- No activation modes, no skills
- General instructions -> `general.md`

## Notes

- Agent path is `.amazonq/cli-agents/` (NOT `.amazonq/agents/`)
- Global agents at `~/.aws/amazonq/cli-agents/<name>.json`
- Agents can be generated via `/agent generate` command
- Agent JSON supports: `tools`, `allowedTools`, `toolsSettings`, `toolAliases`, `mcpServers`, `resources` (glob patterns), `hooks`, `prompt`, `model`, `useLegacyMcpJson`
- conforme-generated agents include `resources: ["file://.amazonq/rules/**/*.md"]` (so they load the synced rules) and `useLegacyMcpJson: true` (so they pick up `.amazonq/mcp.json`)
- `read()` round-trips: it parses back `.amazonq/cli-agents/*.json` (agents) and `.amazonq/mcp.json` (MCP), not just the rules
- The AWS guide retired its `command-line-custom-agents*.html` pages (they now redirect to the guide index); the `aws/amazon-q-developer-cli` `agent-format.md` reference is the canonical agent schema
- `.amazonq/mcp.json` is AWS's **legacy** MCP location: an agent picks it up only with `"useLegacyMcpJson": true`, which is exactly why conforme sets that flag on every agent it generates. Agent-embedded `mcpServers` is the modern form
- IDE version migrating to Kiro format
- CLI has separate doc pages from IDE
