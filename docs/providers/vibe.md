# Mistral Vibe

> Mistral's open-source coding agent CLI (`vibe`). Source: `--from vibe`

**Last verified online:** 2026-11-09, against Mistral Vibe 3.1.0 (https://github.com/mistralai/mistral-vibe/releases)

## Official docs

- Overview: https://docs.mistral.ai/vibe/code/overview
- Configuration: https://docs.mistral.ai/vibe/code/cli/configuration
- Configuration reference: https://docs.mistral.ai/vibe/code/cli/configuration-reference
- Skills: https://docs.mistral.ai/vibe/code/cli/skills
- Agents: https://docs.mistral.ai/vibe/code/cli/agents
- MCP servers: https://docs.mistral.ai/vibe/code/cli/mcp-servers
- Repository: https://github.com/mistralai/mistral-vibe
- Safety, approvals and trusted folders (`vibe --trust`): https://docs.mistral.ai/vibe/code/safety-approvals-permissions
- Commands (`/mcp`, `/reload`): https://docs.mistral.ai/vibe/code/cli/commands-shortcuts
- Admin config (precedence: admin, CLI, env, project, user): https://docs.mistral.ai/vibe/code/cli/admin-config
- VS Code extension agents (same `.vibe/agents`): https://docs.mistral.ai/vibe/code/vs-code-extension/agents
- CLI, VS Code and web share agents, skills and MCP: https://docs.mistral.ai/vibe/code/choose-cli-vscode-web-sessions
- Changelog: https://github.com/mistralai/mistral-vibe/blob/main/CHANGELOG.md
- Plugins (`.vibe/plugins/<plugin>/`, not written): https://docs.mistral.ai/vibe/code/cli/plugins
- Hooks (`hooks.toml`, not written): https://docs.mistral.ai/vibe/code/cli/hooks
- Docs index (the sitemap times out): https://docs.mistral.ai/llms.txt

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native), from the working directory up to the trusted root; since 2.25.8 an `AGENTS.md` in a subdirectory is also surfaced when a file there is read | Markdown |
| Skills | `.vibe/skills/<name>/SKILL.md`, then `.agents/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description`, `disable-model-invocation`, `user-invocable`, `allowed-tools` |
| Agents | `.vibe/agents/<name>.toml` | TOML: `agent_type` (`subagent` for a subagent; other files are modes), `description`, `instructions` or `system_prompt_id` (`.vibe/prompts/<id>.md`), `enabled_tools`; any other key overrides the config (Vibe's own profiles use `active_model`) |
| MCP | `.vibe/config.toml`, `[[mcp_servers]]` | TOML array: `name`, `transport` (`stdio` / `http` / `streamable-http`), `command`, `args`, `env`, `url`, `auth` |

Project files load only in a folder the user trusted (`~/.vibe/trusted_folders.toml`, or the prompt Vibe shows on first run).

## Activation modes

None: `AGENTS.md` is always loaded.

## conforme adapter

- File: `src/adapters/vibe.rs`
- ID: `vibe`
- Capabilities: skills, agents, MCP
- Detected by a `.vibe/` directory
- Reads `AGENTS.md` natively: with Vibe as the source that file is the source, never regenerated
- Skills are written to `.vibe/skills/<name>/SKILL.md` with bundled files and the `.conforme` marker; on read, the shared `.agents/skills/` is used only when `.vibe/skills/` has no skill, so a Codex or Zed copy is not taken for Vibe's own
- Subagents are written to `.vibe/agents/<name>.toml` (`agent_type = "subagent"`, `description`, `instructions`, `enabled_tools` translated to Vibe tool names: `read_file`, `grep`, `write_file`, `edit`, `bash`, `web_fetch`, `web_search`, `todo`, `task`, `skill`, `ask_user_question`). `model` is not written: Vibe resolves it against the user's own model list. An agent file that is not a subagent is a mode the user defined and is never cleaned. An agent named like a built-in one (`ask`, `accept-edits`, `smart-approve`, `auto-approve`, `explore`, `lean`, `BuiltinAgentName` in `vibe/core/agents/models.py`) is neither written nor read, and such a file is never cleaned: Vibe lets a custom file replace the built-in agent ("Custom agent '%s' overrides builtin agent", `vibe/core/agents/registry.py`), so an `accept-edits` subagent, the default start agent, would stop Vibe starting (`sync` warns). `main` is not written either: "The name `main` is reserved, so don't name an agent file `main.toml`" (https://docs.mistral.ai/vibe/code/cli/agents), and Vibe 3.0.3+ refuses the file ("'main' is reserved for hook matching; rename the agent file", seen with 3.1.0). Vibe 3 has no built-in `plan` agent ("Vibe has no built-in `plan` agent", same page; `REMOVED_BUILTIN_AGENTS`), so a synced `plan` subagent is written and loads
- MCP is merged into `.vibe/config.toml` (atomic write, comments and every other setting kept): a server is matched by `name`; stdio is `transport = "stdio"` with `command`, `args`, `env`; Vibe shell-splits a string `command` (`shlex.split`, `vibe/core/config/models.py`), so one that would not come back whole (a path with a space) is written as a one-item list, and a string `command` is split the same way on read; HTTP is `transport = "streamable-http"` with `url`; an `Authorization: Bearer ${VAR}` header becomes `auth = { type = "static", api_key_env = "VAR", api_key_header = "Authorization", api_key_format = "Bearer {token}" }`, other headers go to `auth.headers`; an OAuth `auth`, servers only the target lists and keys conforme does not write are kept, and `disabled` is reset
- `read()` round-trips skills, subagents (`instructions`, or the prompt file `system_prompt_id` names), MCP (static `auth` read back as `Bearer ${VAR}`, a command given as a list, disabled servers skipped) and `AGENTS.md`
- The source decides which `[[mcp_servers]]` exist: one that left the source is dropped, one with `disabled = true` is kept; an inline `mcp_servers = []` is turned into `[[mcp_servers]]` tables
- A header holding exactly one `${VAR}` (when there is no `Authorization: Bearer ${VAR}`), with no `$`, `{` or `}` around it, becomes a static `auth` with `api_key_env`, `api_key_header` and `api_key_format` set to the value with `{token}` for the reference (`${VAR}` → `"{token}"`, `Token ${VAR}` → `"Token {token}"`), so a custom `api_key_format` read from Vibe is written back. Vibe parses `api_key_format` as a Python format string whose only field may be `{token}` (`_validate_api_key_format`, https://github.com/mistralai/mistral-vibe/blob/main/vibe/core/config/models.py, checked 2026-11-11), which is why text holding a brace is left as a literal header
- An explicit `auth.headers` entry wins over the header `api_key_env` would build (`http_headers`, `vibe/core/config/models.py:338-348` at v2.26.1); conforme reads it the same way
- Re-checked 2026-11-09 with Vibe 3.1.0's own loaders (`build_default_orchestrator`, `SkillManager`): both servers, every skill, and `model_invocable = False` on a manual skill
- Verified 2026-11-04 with Vibe 3.1.0's own loaders (`build_default_orchestrator`, `SkillManager`, `AgentRegistry`, `load_project_docs`): the stdio server's `argv()` and `env`, the remote server's static `auth` with `api_key_env = "API_TOKEN"`, every skill (the manual one with `model_invocable` false), the synced subagents and `AGENTS.md`

## Notes

- **No `${VAR}` expansion.** A stdio server gets its `env`, verbatim, on top of `HOME`, `LOGNAME`, `PATH`, `SHELL`, `TERM` and `USER` only (Vibe passes `env` to the MCP SDK's `StdioServerParameters`, whose `get_default_environment()` keeps just those, `mcp/client/stdio/__init__.py`), so `TOKEN = "${TOKEN}"` passes the literal text; only a bearer token is read from a variable (`api_key_env`). `sync` and `migrate` warn for each server with such a reference. A project `[[mcp_servers]]` entry replaces a user entry of the same name as a whole, so declare the server with its real value in `~/.vibe/config.toml` under another name
- A legacy SSE server is written as `transport = "streamable-http"` with a warning: Vibe's `http` and `streamable-http` are the same transport (Mistral Vibe 2.26.1 `vibe/core/tools/mcp/registry.py:309-312`), and conforme writes no SSE one, so it connects only if the server also speaks streamable HTTP
- `vibe` and `skill-creator` are the names of Vibe's built-in skills; since 3.0.4 a project skill of either name loads and wins ("A user skill named vibe or skill-creator is no longer silently skipped", CHANGELOG 3.0.4; `vibe/core/skills/manager.py`: "a local skill wins"; a project `skill-creator` seen loaded with 3.1.0), so conforme no longer warns
- Tool names (unconfirmed): Vibe 3 runs every session on its Unified Harness, which has no `grep` tool ("there is no grep tool, and the agent searches through the shell") and starts subagents with `agents.spawn` rather than `task` (`TOOLS_WITHOUT_EFFECT`, `vibe/core/config/_setting_keys.py`, which names the `[tools.*]` settings without effect). Whether `grep` and `task` in a subagent's `enabled_tools` still select anything is decided by the Rust harness and not documented; Vibe's own built-in `explore` subagent still lists `grep`, so conforme keeps writing both
- Skill `description` is 1 to 1024 characters (`vibe/core/skills/models.py`, `max_length=1024`): a longer one makes Vibe skip the skill, and `validate` warns
- Vibe honours the Codex sidecar `agents/openai.yaml` beside a skill (`allow_implicit_invocation`, `vibe/core/skills/manager.py`): an existing one in `.vibe/skills/<name>/` is kept in step with the source, never created
- Project `.vibe/` directories load only at the trusted working directory, while `.vibe/config.toml` is found by walking up; Vibe also reads `.vibe/plugins/`, `.vibe/tools/`, `.vibe/hooks.toml` and `.vibe/prompts/`, which conforme does not write
- An untrusted folder loads none of the project files: trust it once before checking the result
- Project subagents in `.vibe/agents` can be spawned since Vibe 2.25.8
- Vibe replaces characters outside `[a-zA-Z0-9_-]` in a server name with `_`, strips leading and trailing `_` / `-` and truncates it to 256 characters (`vibe/core/config/models.py:198-199` at v2.26.1); conforme matches servers by the name as written (known gap for names with other characters)
- Vibe's published docs now match its source on what conforme writes: `api_key_env` in `[mcp_servers.auth]` (the same keys directly in `[[mcp_servers]]` are accepted, "Don't combine them with an `[mcp_servers.auth]` table", https://docs.mistral.ai/vibe/code/cli/mcp-servers), `disable-model-invocation` ("set to `true` to stop the agent from loading the skill on its own", https://docs.mistral.ai/vibe/code/cli/skills) and "`search_replace` is now `edit`" (https://docs.mistral.ai/vibe/code/safety-approvals-permissions)

## Live check

Without a model call: load the project with Vibe's own config, skill and agent loaders (the Python of the installed package, with `VIBE_HOME` pointing at a directory whose `trusted_folders.toml` lists the project), and list the MCP servers, skills (with their model-invocation flag), subagents and `AGENTS.md` it sees.
