# OpenCode

> Open-source AI CLI tool. Source: `--from opencode`

**Last verified online:** 2026-10-20, against OpenCode 2 2.1.0 (https://opencode.ai/v2/docs/migrate-v1/, Homebrew `opencode-v2` 2.1.0). conforme targets OpenCode 2 only; OpenCode 1 (v1.19.x) is no longer a target.

## Official docs

- Instructions: https://opencode.ai/v2/docs/instructions/
- Skills: https://opencode.ai/v2/docs/skills/
- Agents: https://opencode.ai/v2/docs/agents/
- MCP servers: https://opencode.ai/v2/docs/mcp-servers/
- Config: https://opencode.ai/v2/docs/config/
- Migrating from V1: https://opencode.ai/v2/docs/migrate-v1/
- CLI: https://opencode.ai/v2/docs/cli/
- Commands: https://opencode.ai/v2/docs/commands/
- Tools: https://opencode.ai/v2/docs/tools/
- Permissions: https://opencode.ai/v2/docs/permissions/
- Models (`provider/model#variant`): https://opencode.ai/v2/docs/models/
- Repository: https://github.com/anomalyco/opencode (formerly `sst/opencode`, which redirects)

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native) | Markdown |
| Skills | `.opencode/skills/<name>/SKILL.md` (also `.opencode/skill/`, `.claude/skills/`, `.agents/skills/`) | YAML frontmatter: `name`, `description`, `disable-model-invocation`, `metadata.opencode/autoinvoke` |
| Agents | `.opencode/agents/<name>.md` (also `agent/`, `mode/`, `modes/`), or the `agents` key of `opencode.json` | Markdown frontmatter: `description`, `mode`, `model`; the body is the system prompt |
| MCP | `mcp.servers` in `opencode.json(c)`: root and `.opencode/` | JSON: `{ "mcp": { "servers": { "<name>": { "type": "local", "command": ["cmd", "...args"], "environment": {...} } } } }` |

## Activation modes

No activation modes. Reads AGENTS.md natively (all content always-on).

## conforme adapter

- File: `src/adapters/opencode.rs`
- ID: `opencode`
- Capabilities: skills, agents, MCP
- No activation modes
- Reads `AGENTS.md` only: OpenCode 2 "currently only discovers AGENTS.md" and dropped OpenCode 1's `CLAUDE.md` / `CONTEXT.md` fallbacks (https://opencode.ai/v2/docs/migrate-v1/#instruction-files). With OpenCode as the source, `AGENTS.md` is its `source_files()`
- Writes MCP servers in OpenCode 2's native shape, under `mcp.servers`, into `.opencode/opencode.jsonc` when it exists, else `.opencode/opencode.json` (`.jsonc` wins over `.json` in the same directory, and `.opencode/` over the root), preserving every other key, JSONC comments included. `$schema` stays `https://opencode.ai/config.json`, the URL the V2 docs use
- Never writes the root `opencode.json(c)`: Kilo Code also loads it and refuses a project config holding `{env:VAR}` ("environment references are not allowed in project config", checked with Kilo 7.8.15 `kilo config check`), which conforme writes for OpenCode; it never reads `.opencode/`. Kilo 7.8.15 does normalize `mcp.servers` and `agents` there (`kilo debug config`), so the shape itself is not the problem
- Agents are written as `.opencode/agents/<name>.md` only (`description`, `mode: subagent`, `model` as `provider/model`, the body as system prompt), OpenCode 2's preferred location, whose frontmatter is already its native shape. Nothing goes into the `agents` / `agent` keys
- Upgrading from an earlier conforme, which wrote OpenCode 1's shape into the first existing of the root `opencode.json(c)` and `.opencode/opencode.json(c)`: a sync with servers in the source moves every expressible `mcp.<name>` server of every project config file to `mcp.servers` of the target, converted (`enabled: false` → `disabled: true`, a numeric `timeout` → `timeout.catalog` and `timeout.execution`, OAuth `clientId`/`clientSecret`/`callbackPort`/`redirectUri` → snake_case; https://opencode.ai/v2/docs/migrate-v1/#mcp-servers), a native entry of the same name winning; the usual merge then drops a server the source lacks unless it is disabled, and keeps the per-server keys conforme does not own. In `agent.<name>` entries of an agent the source defines, the keys conforme wrote (`description`, `mode`, `prompt`, and `model` when the source has a `provider/model` one) are removed and the user's own (`steps`, `permission`, `temperature`, …) kept: OpenCode 2 still applies them to the markdown agent (seen with 2.1.0: `opencode debug agents` shows the markdown `system` with the root `agent.reviewer.steps`). An entry left empty goes, then an empty `agent` or `mcp` key. An entry for an agent that has left the source goes with its file, provided it still has that file and holds only conforme's keys. A `{ "enabled": false }` toggle and every other key are left alone
- `.opencode/agents/<name>.md` named like a built-in agent (`build`, `plan`, `general`, `explore`, `compaction`, `title`, `summary`) is the user's override: never read, written nor swept
- `read()` round-trips AGENTS.md plus skills (`.opencode/skills/`, `.opencode/skill/`), agents (`.opencode/agents/`, `.opencode/agent/`, then the `agents` key, prompt in `system`, and OpenCode 1's `agent` key, prompt in `prompt`, which OpenCode 2 still loads; the markdown files win on a name clash; a `disabled` / `disable` entry is skipped) and MCP (`mcp.servers` and OpenCode 1 `mcp.<name>` entries of every project `opencode.json(c)`, later files winning, a native entry winning over an OpenCode 1 one in the same file). AGENTS.md is parsed with conforme's convention (instructions plus `## Rule:` sections), and its `## Skill:`/`## Agent:`/`## MCP:` sections are added to what this tool's own files hold (which win on a name clash). With this tool as the source, AGENTS.md *is* the source: sync never regenerates it and `gitignore install` keeps it tracked
- Editor variables (`${workspaceFolder}`) are written verbatim, not as `{env:workspaceFolder}`; `sync` warns that OpenCode does not resolve them, and that `${VAR:-default}` loses its default in `{env:VAR}`
- A server with `disabled: true` (or OpenCode 1's `enabled: false`) is not read when OpenCode is the source, and is kept in the file when it is a target

## Notes

- **MCP format is unique:** servers under `mcp.servers` (not `mcpServers`; "V2 does not place server names directly under `mcp`"), `type: local/remote` (not `stdio/http`), `disabled: true` instead of OpenCode 1's `enabled: false`
- **Command is a single array:** `"command": ["npx", "-y", "server-name"]` (no separate `args` field)
- **Env var key is `"environment"`** (not `"env"`), and only local servers have it; conforme writes no `environment` on a remote server
- Variable references are `{env:VAR}`; conforme writes `{env:VAR}` for a normalized `${VAR}` and reads it back
- A remote server "uses the MCP Streamable HTTP transport" (https://opencode.ai/v2/docs/mcp-servers/#remote); the 2.1.0 binary holds `StreamableHTTPClientTransport` and no `SSEClientTransport`. A legacy SSE server is written as `type: "remote"` and `sync` warns that it connects only if the server also speaks streamable HTTP; it reads back as streamable HTTP (no SSE marker)
- Per-server keys conforme never emits (`cwd`, `timeout`, `oauth`, `codemode`, `protocol`, …) are kept, and so are `mcp.timeout` and any other member of `mcp`; `disabled` is reset so a synced server is not left hidden. A file conforme cannot parse is left untouched (sync fails instead of rewriting it), and `remove opencode` / `migrate --source opencode` leave `opencode.json(c)` in place
- An OpenCode 1 `mcp.<name>` entry that is only `{ "enabled": false }` toggles a server defined elsewhere: conforme skips it on read and keeps it on write (OpenCode 2 ignores it with a warning: "An enabled-only V1 MCP entry without a type")
- Overrides of the built-in agents and `agents` / `agent` entries with neither a prompt nor a `description` only tune an agent defined elsewhere, so they are not read as agents. An agent named like a built-in one is not written either: it would override the built-in. V2 has no built-in `scout` agent
- Agent `model` must be `provider/model` (optionally `#variant`); conforme only writes `provider/model` values and otherwise leaves `model` out (OpenCode then uses the parent session's model)
- Agents and skills are always written with a `description` (a skill without one is not advertised), falling back to the name
- `.opencode/agents/` and `.opencode/agent/` are scanned recursively upstream; OpenCode names a nested agent by its path (`agents/team/reviewer.md` is `team/reviewer`); conforme names it by the file (`reviewer`), so a nested agent is renamed on a round trip (known gap). Agents in `mode/` / `modes/` (primary agents) are not read (known gap)
- Agents are written without `permissions`: OpenCode's permission model has no portable equivalent, so a synced agent can use every tool (deliberate; set `agents.<name>.permissions`, or OpenCode 1's `agent.<name>.permission`, in `opencode.json`, which a sync keeps). The reverse is a known gap: with OpenCode as the source, an agent's permissions are not translated
- Only `.opencode/agents/` is swept for orphans, plus the stale bundled files inside a skill folder conforme writes in `.opencode/skills/` (skill folders themselves are never swept). The top-level `.opencode/` also holds user-owned files (`package.json`, `commands/`, `plugins/`, …) and is never cleaned
- Skill frontmatter V2 reads: `name` (a display label; the folder name is the ID), `description`, `metadata.opencode/autoinvoke` and `disable-model-invocation`; `license` and `compatibility` are accepted and ignored. No `allowed-tools`
- OpenCode also discovers skills in `.claude/skills/` and `.agents/skills/`; conforme reads only `.opencode/skills/` and `.opencode/skill/`. When Claude Code or Codex is synced too, the same skill exists in several roots; "the source registered later supplies the skill", and project `.opencode/skills` is registered after both (seen with 2.1.0: `opencode api skill.list` lists each synced skill once, from `.opencode/skills/`). OpenCode ships built-in `opencode` and `report` skills that a synced skill of that name would override
- Skill discovery is recursive (`SKILL.md` at any depth); conforme reads nested folders under `.opencode/skills/` and the singular `.opencode/skill/` too (written back flat). Flat `skills/<name>.md` files are not read (known gap)

## Manual skill invocation

A manual skill is written with `disable-model-invocation: true`, which OpenCode 2 reads "for the same effect as `opencode/autoinvoke: false`": the skill is left out of the model's list and can still be loaded by ID (`@name` in a prompt). conforme no longer writes `metadata.opencode/autoinvoke`; it still reads `metadata.opencode/autoinvoke: false` as manual. To stop OpenCode from loading a skill at all, add a `permissions` rule `{ "action": "skill", "resource": "<name>", "effect": "deny" }`; skill synchronization does not change user permissions.

## Live check

OpenCode 2's debug commands talk to a background service and reject `--standalone`. With an isolated `HOME`/`XDG_*` (and `opencode service set port <free port>` there, since another service may hold the default port), in the project:

```bash
opencode debug config                      # the config sources and their parsed content
opencode debug agents                      # every agent: `reviewer` with `mode: subagent` and its `system`
opencode mcp list                          # each server and its connection status
opencode api skill.list -H "x-opencode-directory:$PWD"   # every skill, `autoinvoke: false` on a manual one
opencode service stop                      # stop the isolated service afterwards
```

The first call for a new directory may return an empty agent list while the project loads; run it again. OpenCode 2 has no `debug skill` command.
