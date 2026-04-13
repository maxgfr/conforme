# Amazon Q Developer

> AWS's AI coding assistant (IDE + CLI). Source: `--from amazonq`

## Official docs

- Project rules (IDE): https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/context-project-rules.html
- Project rules (CLI): https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-project-rules.html
- Custom agents overview: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-custom-agents-overview.html
- Defining agents: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-custom-agents-defining.html
- Agent config reference: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-custom-agents-configuration.html
- Agent examples: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-custom-agents-examples.html
- MCP overview: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/qdev-mcp.html
- MCP CLI config: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-mcp-config-CLI.html
- Rules validation: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/rules-validation.html

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
- Agent JSON supports: `tools`, `permissions`, `mcp-servers`, `resources` (glob patterns), `hooks`, `prompt`, `model`
- IDE version migrating to Kiro format
- CLI has separate doc pages from IDE
