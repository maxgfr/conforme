# Amazon Q Developer

> AWS's AI coding assistant (IDE + CLI). Source: `--from amazonq`

## Official docs

- Project rules (IDE): https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/context-project-rules.html
- Project rules (CLI, now Kiro): https://kiro.dev/docs/cli/steering/
- Custom agents overview: https://kiro.dev/docs/cli/custom-agents/
- Defining agents: https://kiro.dev/docs/cli/custom-agents/creating/
- Agent config reference: https://github.com/aws/amazon-q-developer-cli/blob/main/docs/agent-format.md
- Agent examples: https://kiro.dev/docs/cli/custom-agents/
- MCP overview: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/qdev-mcp.html
- MCP CLI config: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-mcp-config-CLI.html

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.amazonq/rules/*.md` | Plain markdown (NO frontmatter) |
| Agents | `.amazonq/cli-agents/<name>.json` | JSON: `{ "description", "model", "tools", "prompt" }` |
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
- IDE version migrating to Kiro format
- CLI has separate doc pages from IDE
