# Amazon Q Developer

> AWS's AI coding assistant (IDE + CLI). Source: `--from amazonq`

## Official docs

- Project rules (IDE): https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/context-project-rules.html
- CLI user guide: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line.html (now a stub: "The Q CLI has become the Kiro CLI")
- Agent format reference (GitHub, unmaintained): https://github.com/aws/amazon-q-developer-cli/blob/main/docs/agent-format.md
- MCP overview: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/qdev-mcp.html
- MCP CLI config: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-mcp-config-CLI.html
- Kiro CLI migration guide: https://kiro.dev/docs/cli/migrating-from-q-developer/

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

### Upstream status: the Q CLI is now the Kiro CLI

AWS retired the standalone Q Developer CLI. `command-line.html` is now a one-line
stub pointing at the Kiro user guide, and the `aws/amazon-q-developer-cli`
repository carries a deprecation notice ("no longer being actively maintained and
will only receive critical security fixes"). The IDE side is unaffected:
`.amazonq/rules/*.md` is still the documented project-rules location.

The `.amazonq/` paths conforme writes keep working. Per the Kiro migration guide,
the Kiro CLI still reads a project's `.amazonq/` folder for rules, agents and MCP
settings; when a project has both, `.kiro/` wins and newly saved agents go to
`.kiro/`. So a project synced by conforme to both `amazonq` and `kiro` is resolved
in Kiro's favour, which is the intended precedence — keep both targets enabled
only if the project still opens in the Amazon Q IDE plugin.
- The AWS guide retired its `command-line-custom-agents*.html` pages (they now redirect to the guide index); the `aws/amazon-q-developer-cli` `agent-format.md` reference is the canonical agent schema
- `.amazonq/mcp.json` is AWS's **legacy** MCP location: an agent picks it up only with `"useLegacyMcpJson": true`, which is exactly why conforme sets that flag on every agent it generates. Agent-embedded `mcpServers` is the modern form
- IDE version migrating to Kiro format
- CLI has separate doc pages from IDE
