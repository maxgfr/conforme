# Gemini CLI

> Google's AI CLI tool. Source: `--from gemini`

**Last verified online:** 2026-10-09, against Gemini CLI v0.63.0 (https://github.com/google-gemini/gemini-cli/releases)

Since 2026-06-18 Gemini CLI serves only paid Gemini API keys and Gemini Code Assist Standard or Enterprise licences; Google AI Pro, Ultra and free users moved to **Antigravity CLI** (https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli). Antigravity reads the same `GEMINI.md` and `AGENTS.md`, but its project skills live in `.agents/skills/` and its MCP servers in `.agents/mcp_config.json` (remote key `serverUrl`, not `url`/`httpUrl`) (https://antigravity.google/docs/cli/gcli-migration). conforme has an Antigravity target, see [antigravity.md](antigravity.md): it writes `.agents/rules/`, `.agents/skills/`, `.agents/agents/` and `.agents/mcp_config.json`

## Official docs

- GEMINI.md: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/gemini-md.md
- Configuration: https://github.com/google-gemini/gemini-cli/blob/main/docs/reference/configuration.md
- Skills: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/skills.md
- Skills tutorial: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/tutorials/skills-getting-started.md
- Creating skills: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/creating-skills.md
- Trusted folders: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/trusted-folders.md
- Subagents: https://github.com/google-gemini/gemini-cli/blob/main/docs/core/subagents.md
- MCP: https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md
- MCP tutorial: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/tutorials/mcp-setup.md
- Custom commands: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/custom-commands.md
- Skills repo: https://github.com/google-gemini/gemini-skills
- Repository: https://github.com/google-gemini/gemini-cli
- CLI reference (`gemini mcp list`, `gemini skills list`): https://geminicli.com/docs/cli/cli-reference/
- Commands (`/memory list`, `/agents list`, `/skills list`, `/mcp list`): https://geminicli.com/docs/reference/commands/
- Remote agents (`kind: remote`): https://geminicli.com/docs/core/remote-agents/
- Memory imports (`@file.md` in GEMINI.md): https://geminicli.com/docs/reference/memport/
- Settings schema (`context.fileName`, `mcpServers`): https://raw.githubusercontent.com/google-gemini/gemini-cli/main/schemas/settings.schema.json
- Transition to Antigravity CLI (2026-05-19): https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli
- Antigravity CLI migration (what moves where): https://antigravity.google/docs/cli/gcli-migration

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `GEMINI.md` (or the first `context.fileName` entry other than `AGENTS.md`) | Single markdown file (all rules merged) |
| Skills | `.gemini/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` ONLY |
| Agents | `.gemini/agents/<name>.md` | YAML frontmatter: `name`, `description`, `kind: local`, `tools`, `model` |
| MCP | `.gemini/settings.json` | JSON: `{ "mcpServers": { ... } }` (no `type` field, `httpUrl` for HTTP) |

## Activation modes

No activation modes. Single GEMINI.md file, all content always-on.

## conforme adapter

- File: `src/adapters/gemini.rs`
- ID: `gemini`
- Capabilities: skills, agents, MCP
- No activation modes
- All rules merged into a single context file: the first `context.fileName` entry other than `AGENTS.md` (`GEMINI.md` by default); none is written when `context.fileName` names only `AGENTS.md`
- Empty config -> no file generated (avoids empty GEMINI.md)
- `read()` round-trips instructions (the files `context.fileName` names, `GEMINI.md` by default) plus skills (`.gemini/skills/`), agents (`.gemini/agents/`), and MCP (`.gemini/settings.json`)

## Notes

- **Skills frontmatter: `name` and `description`** -- the only keys Gemini's loader reads. For a manual skill conforme also writes `disable-model-invocation` and `metadata` (shared with the other tools reading the same skill); Gemini ignores them, and has no manual-only mechanism
- **MCP format differs from standard:**
  - No `type` field (neither `stdio` nor `http`)
  - HTTP servers use `httpUrl` (not `url`)
  - Headers supported for HTTP
  - `.gemini/settings.json` is the general Gemini settings file (theme, `context.fileName`, …), so conforme **merges** the `mcpServers` key into any existing file rather than overwriting it, and `remove gemini` / `migrate --source gemini` leave the file in place
  - The merge is JSONC-aware (comments outside `mcpServers` survive; an unparsable file is left untouched and sync fails) and keeps Gemini-only per-server options conforme never emits (`trust`, `timeout`, `includeTools`, `excludeTools`, `cwd`, …). A `type` written by `gemini mcp add -t http` is dropped on a streamable HTTP server, since it would contradict the `httpUrl` conforme writes; an SSE server keeps the `type: "sse"` conforme writes
  - Settings strings expand `$VAR`, `${VAR}` and `${VAR:-default}`; conforme keeps `${VAR}` and reads a bare `$VAR` back as `${VAR}`
  - `httpUrl` is deprecated upstream in favour of `url` (+ `type`) but still accepted; a `url` without `type` is now streamable HTTP with an SSE fallback. conforme keeps writing `httpUrl` for remote servers (deliberate deviation): older Gemini versions read a bare `url` as SSE
  - A legacy SSE server is written as `url` + `type: "sse"` and read back as SSE (Gemini CLI v0.63.0 reads `type === 'sse'` on a `url` server, `packages/core/src/tools/mcp-client.ts:2239-2250`); `httpUrl` stays for streamable HTTP
  - The source is authoritative for *which* servers exist: a server present only in `.gemini/settings.json` (for example one added with `gemini mcp add -s project`) is replaced on sync. This is deliberate — the same rule as every other JSON MCP target
- In an untrusted folder Gemini CLI ignores the whole `.gemini/settings.json` (MCP servers and `context.fileName` included), project skills and project agents (`packages/cli/src/config/settings.ts`: `const safeWorkspace = isTrusted ? workspace : {}`; https://github.com/google-gemini/gemini-cli/blob/v0.63.0/docs/cli/trusted-folders.md)
- A project agent is registered only once the user acknowledges it, by a hash of its content (`packages/core/src/agents/registry.ts`, `agentLoader.ts`): every sync that changes an agent file makes Gemini ask again (not in Gemini's own docs)
- A skill needs a string `description` (an empty one loads, `skillLoader.ts`); an agent needs a non-empty one and is rejected otherwise. conforme always writes one, falling back to the name. Skill discovery goes one directory deep (`SKILL.md` and `*/SKILL.md`)
- Agent frontmatter includes `kind: local`. `kind` is **optional** upstream and already defaults to `local` (the other accepted value is `remote`); conforme emits it explicitly so the transport is unambiguous on a round-trip. Only `name` and `description` are required
- The agent schema is strict and rejects the whole agent when `tools` holds an unknown name. conforme translates common tool names from other hosts (`Read` → `read_file`, `Grep` → `grep_search`, `Bash` → `run_shell_command`, `Edit` → `replace`, `WebFetch` → `web_fetch`, …), respells MCP tools (`mcp__github__list_issues` → `mcp_github_list_issues`, `mcp__github` / Kiro `@github` → `mcp_github_*`; Gemini rejects any name starting with `mcp__`), maps Kiro `@mcp` to `mcp_*` (every MCP tool), keeps Gemini built-ins (including the `tracker_*` tools), `*`, `mcp_*` and valid `mcp_<server>_<tool>` names, and drops anything else; a restricted list in which nothing translates becomes `read_file` rather than every tool
- Agent `model` is passed to Gemini's API unchanged unless it is an alias, so conforme writes only `inherit`, `auto`, `pro`, `flash`, `flash-lite`, `gemini-*` or `gemma-*`; another vendor's id is left out and the agent uses the parent model
- Agent files whose name starts with `_` are skipped by Gemini, and by conforme's `read()` too, so a draft is not propagated
- Remote (A2A) agents — `kind: remote`, an agent card (`agent_card_url` / `agent_card_json`) without `kind` (the remote schema defaults `kind` to `remote`, `agentLoader.ts`), or a file whose frontmatter is a YAML list of remote agents — are neither read nor cleaned as orphans, like `_` drafts
- Agent names must match `^[a-z0-9-_]+$`; conforme's name sanitizer writes kebab-case ASCII (accents folded, other characters separate words)
- Agent frontmatter also supports `display_name`, `temperature`, `max_turns`, `timeout_mins`, and `mcp_servers`; `model` defaults to `inherit`. Files whose name starts with `_` are skipped
- Gemini also reads skills from `.agents/skills/`, an alias that takes precedence over `.gemini/skills/`. conforme does not read it when Gemini is the source (known gap: the directory is shared with Codex and Zed, and a skill kept only there is not synced). When Codex or Zed is synced too, each skill exists in both roots and Gemini CLI prints "Skill conflict detected: … is overriding the same skill" for every one of them at startup (seen with Gemini CLI 0.63.0); the `.agents/skills` copy wins and works, the warning is cosmetic (known gap, design pending)
- Hierarchical: `~/.gemini/GEMINI.md` -> project -> subdirs
- Supports `@file.md` imports in GEMINI.md
- `GEMINI.md` is only the *default* context file name: `context.fileName` in `settings.json` accepts a name or a list (e.g. `["AGENTS.md", "GEMINI.md"]`). Gemini CLI does **not** read `AGENTS.md` unless configured to. conforme's `read()` honours the project `context.fileName`; when it names `AGENTS.md`, that file is read with the AGENTS.md convention and, with Gemini as the source, is never regenerated nor gitignored (`reads_agents_md`). With Gemini as the source, the context files it reads (`source_files`) are never written by another target, deleted by `remove`/`migrate`, nor gitignored
- As a target conforme writes the instructions to the first `context.fileName` entry other than `AGENTS.md` (`GEMINI.md` by default), and writes none when it names only `AGENTS.md` (Gemini then reads `AGENTS.md`, which is what `migrate --output gemini` writes). `gitignore install` ignores the context file conforme writes, not a fixed `GEMINI.md`
- The former Google Cloud page (`docs.cloud.google.com/gemini/docs/codeassist/gemini-cli`) now 404s; the `google-gemini/gemini-cli` repository docs are the canonical reference
