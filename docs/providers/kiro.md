# Kiro (AWS)

> AWS's AI IDE, successor to Amazon Q CLI. Source: `--from kiro`

## Official docs

- Steering: https://kiro.dev/docs/steering/
- Skills: https://kiro.dev/docs/skills/
- Powers (bundles): https://kiro.dev/docs/powers/
- Creating powers: https://kiro.dev/docs/powers/create/
- MCP (IDE and CLI): https://kiro.dev/docs/mcp/configuration/
- Agent configuration reference: https://kiro.dev/docs/custom-agents/configuration-reference/
- Changelog: https://kiro.dev/changelog/
- Getting started: https://kiro.dev/docs/getting-started/first-project/
- Powers marketplace: https://kiro.dev/powers/
- Custom agents: https://kiro.dev/docs/custom-agents/creating/
- What's new in CLI 3.0: https://kiro.dev/docs/cli/v3/
- CLI 3.0 agent config: https://kiro.dev/docs/cli/v3/agent-config/
- Built-in tools: https://kiro.dev/docs/reference/built-in-tools/
- Workflows: https://kiro.dev/docs/workflows/

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules (steering) | `.kiro/steering/*.md` | YAML frontmatter: `inclusion`, `fileMatchPattern`, `name`, `description` |
| Skills | `.kiro/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` (required); `license`, `compatibility`, `metadata` (optional) |
| Agents | `.kiro/agents/<name>.md` | YAML frontmatter: `description`, `model`, `tools` (no `name` — Kiro derives it from the file path) |
| MCP | `.kiro/settings/mcp.json` (merged) | JSON: `{ "mcpServers": { ... } }`; per-server `disabled`, `autoApprove`, `disabledTools`, `oauth` are Kiro's and survive a sync |

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
- "Powers" now follow the Agent Plugins spec: a `plugin.json` plus `skills/` and `mcp.json` (steering under `dev.kiro/`)
- `.kiro/agents/` accepts a Markdown **or** a JSON file, and the filename becomes the agent name. conforme writes Markdown. `name` is a recognized frontmatter key, but since the filename already supplies it conforme omits it to avoid two competing sources of truth
- JSON agents (`.kiro/agents/<name>.json`) are neither read nor written by conforme, and orphan cleanup and `migrate --source kiro` only remove `.md` files there, so they are never deleted
- Agent `tools` accepts Kiro tags (`read`, `write`, `shell`, `web`, `subagent`, `knowledge`, `todo_list` — only on engines that provide it, `@builtin`, `@mcp`, `@server[/tool]`, `*`), individual built-in tools (`glob`, `grep`, `web_search`, `web_fetch`, `aws`, `code`, `delegate`, `thinking`, `todo`, …) and their legacy aliases (`fs_read`, `fs_write`, `execute_bash`, `use_aws`, …). conforme keeps all of those, translates common names from other hosts (`Read` → `read`, `Grep` → `grep`, `Edit`/`Write` → `write`, `Bash` → `shell`, `WebFetch` → `web_fetch`, `TodoWrite`/`write_todos` → `todo`, `mcp__github__list_issues` → `@github/list_issues`, Gemini `mcp_*` → `@mcp`, …), and drops anything else; a restricted list in which nothing translates becomes `read` rather than every tool. Kiro's docs disagree on the todo tool's name (`todo` vs `todo_list`); conforme writes `todo` and accepts both on read
- Agent `model`: the V3 agent-config page says an unknown model stays selected and every request fails, while the configuration reference says it falls back to the default model with a warning; conforme leaves out another host's alias (`sonnet`, `opus`, `haiku`, `fable`, `pro`, `flash`, `flash-lite`), any `provider/model` id and `inherit`, and the agent uses Kiro's default model; other values are copied as they are
- Agents and skills are always written with a `description`, falling back to the name
- Kiro CLI 3.0 also allows MCP servers inline inside an agent config, and adds `oauth` and `disabledTools` per server; conforme keeps to the shared `.kiro/settings/mcp.json`
- Steering `name` is the "Identifier for the steering file. Used for display and matching" (example `name: api-design`), defined for `auto` inclusion only; conforme writes the file stem (`api-design`), not the rule's display title, and also writes it on manual rules
- A manual rule is pulled into chat with `#steering-file-name`, which names the file, not `name`
- `auto` (agent-decision) rules must include both `name` and `description`; conforme falls back to the rule name when the source has no description
- `.kiro/settings/mcp.json` is merged, not owned: Kiro's own per-server state survives, `disabled` is reset so a synced server runs, a file conforme cannot parse is left untouched, and `remove kiro` / `migrate --source kiro` keep the file. Environment references are `${VAR}` (only variables approved in Kiro's settings expand); a `:-default` is dropped
- Kiro documents no manual-only skill switch: the `disable-model-invocation` and `metadata` keys conforme writes for a manual skill are ignored, and Kiro may still load it on its own
- Kiro always includes a root `AGENTS.md`, without inclusion modes, so in a project that keeps `AGENTS.md` its rules reach Kiro twice and glob/manual rules are always on there. Kiro CLI V3 supports all four steering inclusion modes; CLI V1/V2 load only `always` steering
- When Kiro is a target, orphan cleanup removes every top-level `.md` in `.kiro/steering/` that the source did not generate, including the `product.md`/`tech.md`/`structure.md` Kiro's "Generate Steering Docs" creates; keep those in the source or set `clean = false`
- Kiro reads AGENTS.md natively
- CLI agent JSON format differs from IDE markdown format
- The CLI and the IDE share the same MCP paths and schema (`.kiro/settings/mcp.json` workspace, `~/.kiro/settings/mcp.json` global); precedence is agent config > workspace > global
- Remote MCP servers additionally accept `headers`, `oauth` and `oauthScopes`; conforme emits only `url` + `headers`
- conforme writes `type: stdio` / `type: http` in `.kiro/settings/mcp.json`. The mcp.json docs do not list `type`, but Kiro accepts it: it appears in Kiro's agent-config MCP examples, and Kiro's own `kirodotdev/powers` repository uses `"type": "http"` / `"stdio"` in its `mcp.json` files
