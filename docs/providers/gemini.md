# Gemini CLI

> Google's AI CLI tool. Source: `--from gemini`

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

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `GEMINI.md` | Single markdown file (all rules merged) |
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
- All rules merged into single GEMINI.md
- Empty config -> no file generated (avoids empty GEMINI.md)
- `read()` round-trips instructions plus skills (`.gemini/skills/`), agents (`.gemini/agents/`), and MCP (`.gemini/settings.json`)

## Notes

- **Skills frontmatter: `name` and `description`** -- the only keys Gemini's loader reads. For a manual skill conforme also writes `disable-model-invocation` and `metadata` (shared with the other tools reading the same skill); Gemini ignores them, and has no manual-only mechanism
- **MCP format differs from standard:**
  - No `type` field (neither `stdio` nor `http`)
  - HTTP servers use `httpUrl` (not `url`)
  - Headers supported for HTTP
  - `.gemini/settings.json` is the general Gemini settings file (theme, `context.fileName`, …), so conforme **merges** the `mcpServers` key into any existing file rather than overwriting it, and `remove gemini` / `migrate --source gemini` leave the file in place
  - The merge is JSONC-aware (comments outside `mcpServers` survive; an unparsable file is left untouched and sync fails) and keeps Gemini-only per-server options conforme never emits (`trust`, `timeout`, `includeTools`, `excludeTools`, `cwd`, …). A `type` written by `gemini mcp add -t http|sse` is dropped, since it would contradict the `httpUrl` conforme writes
  - Settings strings expand `$VAR`, `${VAR}` and `${VAR:-default}`; conforme keeps `${VAR}` and reads a bare `$VAR` back as `${VAR}`
  - `url` is Gemini's SSE key and `httpUrl` its streamable-HTTP key. conforme maps every remote server to streamable HTTP, so an SSE-only server is written with the wrong key (known gap). The Gemini source calls `httpUrl` deprecated in favour of `url` + `type`, but the docs still present `httpUrl` as current
  - The source is authoritative for *which* servers exist: a server present only in `.gemini/settings.json` (for example one added with `gemini mcp add -s project`) is replaced on sync. This is deliberate — the same rule as every other JSON MCP target
- Project MCP servers and skills load only in trusted folders (since v0.59, "filter mcpServers in restricted mode")
- Skills and agents need a non-empty `description` (a skill without one is silently skipped, an agent is rejected); conforme always writes one, falling back to the name
- Agent frontmatter includes `kind: local`. `kind` is **optional** upstream and already defaults to `local` (the other accepted value is `remote`); conforme emits it explicitly so the transport is unambiguous on a round-trip. Only `name` and `description` are required
- The agent schema is strict and rejects the whole agent when `tools` holds an unknown name. conforme translates common tool names from other hosts (`Read` → `read_file`, `Grep` → `grep_search`, `Bash` → `run_shell_command`, `Edit` → `replace`, `WebFetch` → `web_fetch`, …), respells MCP tools (`mcp__github__list_issues` → `mcp_github_list_issues`, `mcp__github` / Kiro `@github` → `mcp_github_*`; Gemini rejects any name starting with `mcp__`), keeps Gemini built-ins (including the `tracker_*` tools), `*` and valid `mcp_<server>_<tool>` names, and drops anything else; a restricted list in which nothing translates becomes `read_file` rather than every tool
- Agent `model` is passed to Gemini's API unchanged unless it is an alias, so conforme writes only `inherit`, `auto`, `pro`, `flash`, `flash-lite` or `gemini-*`; another vendor's id is left out and the agent uses the parent model
- Agent files whose name starts with `_` are skipped by Gemini, and by conforme's `read()` too, so a draft is not propagated
- Agent names must match `^[a-z0-9-_]+$`; conforme's name sanitizer writes kebab-case ASCII (accents folded, other characters separate words)
- Agent frontmatter also supports `display_name`, `temperature`, `max_turns`, `timeout_mins`, and `mcp_servers`; `model` defaults to `inherit`. Files whose name starts with `_` are skipped
- Hierarchical: `~/.gemini/GEMINI.md` -> project -> subdirs
- Supports `@file.md` imports in GEMINI.md
- `GEMINI.md` is only the *default* context file name: `context.fileName` in `settings.json` accepts a name or a list (e.g. `["AGENTS.md", "GEMINI.md"]`). Gemini CLI does **not** read `AGENTS.md` unless configured to, which is why conforme writes `GEMINI.md`
- The former Google Cloud page (`docs.cloud.google.com/gemini/docs/codeassist/gemini-cli`) now 404s; the `google-gemini/gemini-cli` repository docs are the canonical reference
