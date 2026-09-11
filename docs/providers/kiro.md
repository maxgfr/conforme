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
- Custom agents: https://kiro.dev/docs/custom-agents/creating/
- What's new in CLI 3.0: https://kiro.dev/docs/cli/v3/

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules (steering) | `.kiro/steering/*.md` | YAML frontmatter: `inclusion`, `fileMatchPattern`, `name`, `description` |
| Skills | `.kiro/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` (required); `license`, `compatibility`, `metadata` (optional) |
| Agents | `.kiro/agents/<name>.md` | YAML frontmatter: `description`, `model`, `tools` (no `name` — Kiro derives it from the file path) |
| MCP | `.kiro/settings/mcp.json` | JSON: `{ "mcpServers": { ... } }` (Kiro also supports optional `disabled`/`autoApprove` per server; conforme does not emit them) |

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
- `read()` round-trips steering plus skills (`.kiro/skills/`), agents (`.kiro/agents/`), and MCP (`.kiro/settings/mcp.json`)

## Notes

- Kiro has a rich hook system (preToolUse, postToolUse, agentSpawn, userPromptSubmit). As of Kiro CLI 3.0 hooks are standalone `.kiro/hooks/*.json` files rather than blocks embedded in an agent config
- Hooks are part of "Powers" (bundles of steering + skills + hooks + MCP)
- `.kiro/agents/` accepts a Markdown **or** a JSON file, and the filename becomes the agent name. conforme writes Markdown. `name` is a recognized frontmatter key, but since the filename already supplies it conforme omits it to avoid two competing sources of truth
- Kiro CLI 3.0 also allows MCP servers inline inside an agent config, and adds `oauth` and `disabledTools` per server; conforme keeps to the shared `.kiro/settings/mcp.json`
- Manual rules should include `name` for slash-command display (`#steering-file-name`)
- `auto` (agent-decision) rules must include both `name` and `description`
- Kiro reads AGENTS.md natively
- CLI agent JSON format differs from IDE markdown format
- The CLI and the IDE share the same MCP paths and schema (`.kiro/settings/mcp.json` workspace, `~/.kiro/settings/mcp.json` global); precedence is agent config > workspace > global
- Remote MCP servers additionally accept `headers`, `oauth` and `oauthScopes`; conforme emits only `url` + `headers`
