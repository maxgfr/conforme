# Kiro (AWS)

> AWS's AI IDE, successor to Amazon Q CLI. Source: `--from kiro`

## Official docs

- Steering: https://kiro.dev/docs/steering/
- Skills: https://kiro.dev/docs/skills/
- Powers (bundles): https://kiro.dev/docs/powers/
- Creating powers: https://kiro.dev/docs/powers/create/
- MCP (IDE): https://kiro.dev/docs/mcp/configuration/
- MCP (CLI): https://kiro.dev/docs/cli/mcp/configuration/
- Getting started: https://kiro.dev/docs/getting-started/first-project/
- Powers marketplace: https://kiro.dev/powers/

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules (steering) | `.kiro/steering/*.md` | YAML frontmatter: `inclusion`, `fileMatchPattern`, `name`, `description` |
| Skills | `.kiro/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| Agents | `.kiro/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| MCP | `.kiro/settings/mcp.json` | JSON: `{ "mcpServers": { ... } }` (standard format with `disabled` field) |

## Activation modes

| Mode | Frontmatter |
|------|------------|
| Always | `inclusion: always` |
| GlobMatch | `inclusion: fileMatch` + `fileMatchPattern: ["**/*.ts"]` (YAML array) |
| AgentDecision | `inclusion: auto` + `name` + `description` |
| Manual | `inclusion: manual` + `name` |

## conforme adapter

- File: `src/adapters/kiro.rs`
- ID: `kiro`
- Capabilities: activation_modes, skills, agents, MCP
- General instructions -> `general.md` with `inclusion: always`
- `fileMatchPattern` accepts both string and YAML array

## Notes

- Kiro has a rich hook system (preToolUse, postToolUse, agentSpawn, userPromptSubmit)
- Hooks are part of "Powers" (bundles of steering + skills + hooks + MCP)
- Manual rules should include `name` for slash-command display (`#steering-file-name`)
- `auto` (agent-decision) rules must include both `name` and `description`
- Kiro reads AGENTS.md natively
- CLI agent JSON format differs from IDE markdown format
