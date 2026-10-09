# OpenCode

> Open-source AI CLI tool. Source: `--from opencode`

**Last verified online:** 2026-10-09, against OpenCode v1.18.35 (https://github.com/anomalyco/opencode/releases)

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
- Models (`provider_id/model_id`): https://opencode.ai/docs/models

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native) | Markdown |
| Skills | `.opencode/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| Agents | `opencode.json` (`agent` key) + `.opencode/agents/<name>.md` (also `.opencode/agent/`) | JSON + Markdown frontmatter |
| MCP | `opencode.json` (`mcp` key): root `opencode.json(c)` and `.opencode/opencode.json(c)` | JSON: `{ "mcp": { "<name>": { "type": "local", "command": ["cmd", "...args"], "environment": {...} } } }` |

## Activation modes

No activation modes. Reads AGENTS.md natively (all content always-on).

## conforme adapter

- File: `src/adapters/opencode.rs`
- ID: `opencode`
- Capabilities: skills, agents, MCP
- No activation modes
- Reads AGENTS.md natively, falls back to CLAUDE.md, then the deprecated CONTEXT.md (first match wins; `packages/opencode/src/session/instruction.ts`)
- Writes MCP and agent definitions into the project's `opencode.json`, preserving any existing keys: the first that exists of `opencode.json`, `opencode.jsonc`, `.opencode/opencode.json`, `.opencode/opencode.jsonc`, else a new `.opencode/opencode.json`. OpenCode loads the `.opencode/` copy as project config (checked with `opencode debug config`); Kilo Code, which also loads the root `opencode.json` and refuses a project file holding `{env:VAR}`, never reads it, so both tools work side by side
- The `agent` key is shared with the user (overrides of the built-in `build`/`plan` agents, per-agent `permission`, agents written by hand): every entry the source does not define is kept. For a synced agent, `description`, `mode` and `prompt` are replaced, `model` only when the source has a `provider/model` value, and the user's own keys (`permission`, `temperature`, `steps`, a `model` the source cannot express, …) are kept. An agent removed from the source therefore stays in `opencode.json` until deleted by hand (its `.opencode/agents/<name>.md` is cleaned): conforme cannot tell an agent it wrote from one written by hand
- `read()` round-trips AGENTS.md plus skills (`.opencode/skills/`), agents (`.opencode/agents/`, `.opencode/agent/` and the `agent` key in `opencode.json`, read together as OpenCode loads them; the markdown files win on a name clash), and MCP (the `mcp` key of every project `opencode.json(c)`, the `.opencode/` one winning on a name clash); the file is parsed with conforme's AGENTS.md convention (instructions plus `## Rule:` sections), and its `## Skill:`/`## Agent:`/`## MCP:` sections are added to what this tool's own files hold (which win on a name clash). With this tool as the source, AGENTS.md *is* the source: sync never regenerates it (`generate_agents_md` does not apply) and `gitignore install` keeps it tracked. The file it reads (`AGENTS.md`, else `CLAUDE.md`, else `CONTEXT.md`) is its `source_files()`: no target writes it (the Claude Code target leaves an OpenCode source's `CLAUDE.md` alone), and `remove`/`migrate` never delete it

## Notes

- **MCP format is unique:** uses `"mcp"` key (not `"mcpServers"`), `type: local/remote` (not `stdio/http`)
- **Command is a single array:** `"command": ["npx", "-y", "server-name"]` (no separate `args` field)
- **Env var key is `"environment"`** (not `"env"`), and only local servers have it; conforme writes no `environment` on a remote server
- Variable references are `{env:VAR}` (unset → empty string); conforme writes `{env:VAR}` for a normalized `${VAR}` and reads it back
- MCP and `agent` blocks live inside `opencode.json` (or `opencode.jsonc`), at the project root or in `.opencode/`. OpenCode parses both as JSONC. conforme merges into the existing file so user-authored keys are preserved, and `remove opencode` / `migrate --source opencode` leave the file in place rather than deleting the user's settings with it
- The merge is JSONC-aware: comments outside the managed `mcp`/`agent` keys survive, and a file conforme cannot parse is left untouched (sync fails instead of rewriting it). Per-server keys conforme never emits (`cwd`, `timeout`, `oauth`, …) are kept; `enabled` is reset so a synced server is not left hidden
- An `mcp.<name>` entry that is only `{ "enabled": false }` toggles a server defined elsewhere (the global config): conforme skips it on read and keeps it untouched on write
- Overrides of the built-in agents (`build`, `plan`, `general`, `explore`, `compaction`, `title`, `summary`) and `agent` entries with neither a `prompt` nor a `description` only tune an agent defined elsewhere, so they are not read as agents, from `opencode.json` nor from `.opencode/agents/`. An agent named like a built-in one is not written to OpenCode either: it would override the built-in (a synced `build` agent demoted OpenCode's own Build agent to a subagent)
- A root `opencode.json` that holds `{env:VAR}` references makes Kilo Code refuse the project config; with Kilo detected, `sync` and `migrate` print a warning suggesting `.opencode/opencode.json`
- Agent `model` must be `provider/model`; OpenCode splits on `/`, so a bare id such as `gpt-4o` would resolve to provider `gpt-4o` with no model. conforme only writes `provider/model` values and otherwise leaves `model` out (OpenCode then uses its default)
- Agents and skills are always written with a `description` (a skill without one is not listed), falling back to the name
- `.opencode/agents/` and `.opencode/agent/` are scanned recursively upstream; conforme reads nested agents too (written back flat). OpenCode names a nested agent by its path (`agents/team/reviewer.md` is `team/reviewer`); conforme names it by the file (`reviewer`), so a nested agent is renamed on a round trip (known gap)
- Agents are written without `tools`/`permission`: OpenCode's permission model has no portable equivalent, so a synced agent can use every tool (deliberate; set `agent.<name>.permission` in `opencode.json`, which a sync keeps). The reverse is a known gap: with OpenCode as the source, an agent's `tools` map (`write: false`, …) and `permission` are not translated, so the other tools get the agent unrestricted
- A markdown agent's frontmatter `name` overrides its file name upstream; conforme names it by the file (known gap)
- `metadata.opencode/autoinvoke` has no reader on OpenCode's `dev` branch: conforme writes it as a hint that OpenCode is not known to read (a harmless string key), so a manual skill still needs a `permission.skill` deny entry
- Only `.opencode/agents/` is swept for orphans, plus the stale bundled files inside a skill folder conforme writes in `.opencode/skills/` (skill folders themselves are never swept). The top-level `.opencode/` also holds user-owned files (`package.json` for plugins, `commands/`, `tools/`, …) and is never cleaned
- Markdown agents additionally emitted to `.opencode/agents/<name>.md` (OpenCode discovers per-project agents from that directory)
- Skill frontmatter recognizes only `name`, `description`, `license`, `compatibility`, `metadata` — no `allowed-tools`
- OpenCode also discovers skills in `.claude/skills/` and `.agents/skills/`; conforme reads only `.opencode/skills/` and `.opencode/skill/`. When Claude Code or Codex is synced too, OpenCode therefore lists each synced skill twice (its own copy and the other tool's)
- Skill discovery is recursive (`{skill,skills}/**/SKILL.md`); conforme reads nested folders under `.opencode/skills/` and the singular `.opencode/skill/` too (written back flat)

## Manual skill invocation

Manual skills preserve `disable-model-invocation: true`, `metadata.opencode/autoinvoke: "false"`, and Codex `agents/openai.yaml` with `policy.allow_implicit_invocation: false` through synchronization. In AGENTS.md, use `<!-- invocation: manual -->` in the skill section. conforme writes `metadata.opencode/autoinvoke` as a hint that OpenCode is not known to read (no reader exists on its `dev` branch), so OpenCode needs the corresponding `permission.skill` deny entries; skill synchronization does not change user permissions.
