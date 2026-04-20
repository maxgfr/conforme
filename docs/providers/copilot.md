# GitHub Copilot

> GitHub's AI coding assistant. Source: `--from copilot`

## Official docs

- Custom instructions: https://docs.github.com/copilot/customizing-copilot/adding-custom-instructions-for-github-copilot
- Custom agents config: https://docs.github.com/en/copilot/reference/custom-agents-configuration
- CLI skills: https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/create-skills
- Cloud agent skills: https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/create-skills
- MCP tutorial: https://docs.github.com/en/copilot/tutorials/enhance-agent-mode-with-mcp
- Hooks config: https://docs.github.com/en/copilot/reference/hooks-configuration
- CLI hooks: https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-hooks
- CLI overview: https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/overview
- Features: https://docs.github.com/en/copilot/get-started/features

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions (global) | `.github/copilot-instructions.md` | Plain markdown (always applied) |
| Instructions (per-file) | `.github/instructions/<name>.instructions.md` | YAML frontmatter: `applyTo` (glob) |
| Prompts (skills) | `.github/prompts/<name>.prompt.md` | YAML frontmatter: `description`, `tools`, `agent` |
| Agents | `.github/agents/<name>.agent.md` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| MCP | `.vscode/mcp.json` | JSON: `{ "servers": { ... } }` (**not** `mcpServers`) |

## Activation modes

| Mode | Implementation |
|------|---------------|
| Always | Content in `.github/copilot-instructions.md` (main file) |
| GlobMatch | `.github/instructions/<name>.instructions.md` with `applyTo: "**/*.ts"` |
| AgentDecision | Content in main file (no native agent-decision mode) |
| Manual | Content in main file (no native manual mode) |

## conforme adapter

- File: `src/adapters/copilot.rs`
- ID: `copilot`
- Capabilities: activation_modes, skills, agents, MCP
- Always/AgentDecision/Manual rules -> inlined in `copilot-instructions.md`
- GlobMatch rules -> separate `.instructions.md` files

## Notes

- MCP uses `"servers"` key (NOT `"mcpServers"`) -- unique among all tools
- MCP supports `env` on stdio and `headers` on HTTP transports (conforme emits both when set)
- Prompts have optional `agent` field (values: `ask`, `edit`, `agent`, `plan`, or custom agent name)
- Additional optional fields on instructions: `name`, `description`, `excludeAgent`
- Additional optional fields on agents: `handoffs`, `mcp-servers`, `target`, `user-invocable`, `disable-model-invocation`, `metadata`
- `handoffs` is not supported on the Copilot cloud agent on GitHub.com (CLI only)
- Copilot now has hooks support (CLI and cloud agent) -- not synced by conforme
