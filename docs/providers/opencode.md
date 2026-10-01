# OpenCode

> Open-source AI CLI tool. Source: `--from opencode`

## Official docs

- Rules: https://opencode.ai/docs/rules/
- Skills: https://opencode.ai/docs/skills/
- Agents: https://opencode.ai/docs/agents/
- MCP servers: https://opencode.ai/docs/mcp-servers/
- Config: https://opencode.ai/docs/config/
- CLI: https://opencode.ai/docs/cli/
- Commands: https://opencode.ai/docs/commands/
- Tools: https://opencode.ai/docs/tools/
- Permissions: https://opencode.ai/docs/permissions/
- Repository: https://github.com/anomalyco/opencode (formerly `sst/opencode`, which redirects)

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native) | Markdown |
| Skills | `.opencode/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| Agents | `opencode.json` (`agent` key) + `.opencode/agents/<name>.md` | JSON + Markdown frontmatter |
| MCP | `opencode.json` (`mcp` key) | JSON: `{ "mcp": { "<name>": { "type": "local", "command": ["cmd", "...args"], "environment": {...} } } }` |

## Activation modes

No activation modes. Reads AGENTS.md natively (all content always-on).

## conforme adapter

- File: `src/adapters/opencode.rs`
- ID: `opencode`
- Capabilities: skills, agents, MCP
- No activation modes
- Reads AGENTS.md natively, falls back to CLAUDE.md
- Writes MCP and agent definitions into `opencode.json` at the project root, preserving any existing keys
- The `agent` key is shared with the user (overrides of the built-in `build`/`plan` agents, per-agent `permission`, agents written by hand): every entry the source does not define is kept. For a synced agent, `description`, `mode` and `prompt` are replaced, `model` only when the source has a `provider/model` value, and the user's own keys (`permission`, `temperature`, `steps`, a `model` the source cannot express, …) are kept. An agent removed from the source therefore stays in `opencode.json` until deleted by hand (its `.opencode/agents/<name>.md` is cleaned): conforme cannot tell an agent it wrote from one written by hand
- `read()` round-trips AGENTS.md plus skills (`.opencode/skills/`), agents (`.opencode/agents/*.md`, falling back to the `agent` key in `opencode.json`), and MCP (the `mcp` key in `opencode.json`); the file is parsed with conforme's AGENTS.md convention (instructions plus `## Rule:` sections), and its `## Skill:`/`## Agent:`/`## MCP:` sections are added to what this tool's own files hold (which win on a name clash). With this tool as the source, AGENTS.md *is* the source: sync never regenerates it (`generate_agents_md` does not apply) and `gitignore install` keeps it tracked

## Notes

- **MCP format is unique:** uses `"mcp"` key (not `"mcpServers"`), `type: local/remote` (not `stdio/http`)
- **Command is a single array:** `"command": ["npx", "-y", "server-name"]` (no separate `args` field)
- **Env var key is `"environment"`** (not `"env"`), and only local servers have it; conforme writes no `environment` on a remote server
- Variable references are `{env:VAR}` (unset → empty string); conforme writes `{env:VAR}` for a normalized `${VAR}` and reads it back
- MCP and `agent` blocks live inside `opencode.json` (or `opencode.jsonc`) at the project root. OpenCode parses both as JSONC. conforme merges into any existing `opencode.json` so user-authored keys are preserved, and `remove opencode` / `migrate --source opencode` leave the file in place rather than deleting the user's settings with it
- The merge is JSONC-aware: comments outside the managed `mcp`/`agent` keys survive, and a file conforme cannot parse is left untouched (sync fails instead of rewriting it). Per-server keys conforme never emits (`cwd`, `timeout`, `oauth`, …) are kept; `enabled` is reset so a synced server is not left hidden
- `opencode.jsonc` is neither read nor written by conforme (known gap): a project that only has `opencode.jsonc` gets a separate `opencode.json`
- Agent `model` must be `provider/model`; OpenCode splits on `/`, so a bare id such as `gpt-4o` would resolve to provider `gpt-4o` with no model. conforme only writes `provider/model` values and otherwise leaves `model` out (OpenCode then uses its default)
- Agents and skills are always written with a `description` (a skill without one is not listed), falling back to the name
- `.opencode/agents/` is scanned recursively upstream; conforme reads nested agents too (written back flat). OpenCode names a nested agent by its path (`agents/team/reviewer.md` is `team/reviewer`); conforme names it by the file (`reviewer`), so a nested agent is renamed on a round trip (known gap)
- Agents are written without `tools`/`permission`: OpenCode's permission model has no portable equivalent, so a synced agent can use every tool (deliberate; set `agent.<name>.permission` in `opencode.json`, which a sync keeps)
- `metadata.opencode/autoinvoke` has no reader on OpenCode's `dev` branch (unverified); it is a harmless string key
- Only `.opencode/agents/` is swept for orphans. The top-level `.opencode/` also holds user-owned files (`package.json` for plugins, `commands/`, `tools/`, …) and is never cleaned
- Markdown agents additionally emitted to `.opencode/agents/<name>.md` (OpenCode discovers per-project agents from that directory)
- Skill frontmatter recognizes only `name`, `description`, `license`, `compatibility`, `metadata` — no `allowed-tools`
- Skills also discovered from `.agents/skills/` and `.claude/skills/`
- Skill discovery is recursive (`{skill,skills}/**/SKILL.md`); conforme reads nested folders under `.opencode/skills/` and the singular `.opencode/skill/` too (written back flat)

## Manual skill invocation

Manual skills preserve `disable-model-invocation: true`, OpenCode V2 `metadata.opencode/autoinvoke: "false"`, and Codex `agents/openai.yaml` with `policy.allow_implicit_invocation: false` through synchronization. In AGENTS.md, use `<!-- invocation: manual -->` in the skill section. OpenCode V1 still needs the corresponding `permission.skill` deny entries; skill synchronization does not change user permissions.
