# Mistral Vibe

> Mistral's open-source coding agent CLI (`vibe`). Source: `--from vibe`

**Last verified online:** 2026-10-09, against Mistral Vibe 2.26.1 (https://github.com/mistralai/mistral-vibe/releases)

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
- Subagents are written to `.vibe/agents/<name>.toml` (`agent_type = "subagent"`, `description`, `instructions`, `enabled_tools` translated to Vibe tool names: `read_file`, `grep`, `write_file`, `edit`, `bash`, `web_fetch`, `web_search`, `todo`, `task`, `skill`, `ask_user_question`). `model` is not written: Vibe resolves it against the user's own model list. An agent file that is not a subagent is a mode the user defined and is never cleaned. An agent named like a built-in one (`ask`, `plan`, `accept-edits`, `smart-approve`, `auto-approve`, `explore`, `lean`) is neither written nor read, and such a file is never cleaned: Vibe lets a custom file replace the built-in agent ("Custom agent '%s' overrides builtin agent", `vibe/core/agents/registry.py`), so a synced `plan` would remove the Plan mode and an `accept-edits` subagent, the default start agent, would stop Vibe starting (`sync` warns)
- MCP is merged into `.vibe/config.toml` (atomic write, comments and every other setting kept): a server is matched by `name`; stdio is `transport = "stdio"` with `command`, `args`, `env`; Vibe shell-splits a string `command` (`shlex.split`, `vibe/core/config/models.py`), so one that would not come back whole (a path with a space) is written as a one-item list, and a string `command` is split the same way on read; HTTP is `transport = "streamable-http"` with `url`; an `Authorization: Bearer ${VAR}` header becomes `auth = { type = "static", api_key_env = "VAR", api_key_header = "Authorization", api_key_format = "Bearer {token}" }`, other headers go to `auth.headers`; an OAuth `auth`, servers only the target lists and keys conforme does not write are kept, and `disabled` is reset
- `read()` round-trips skills, subagents (`instructions`, or the prompt file `system_prompt_id` names), MCP (static `auth` read back as `Bearer ${VAR}`, a command given as a list, disabled servers skipped) and `AGENTS.md`

## Notes

- **No `${VAR}` expansion.** A stdio server gets its `env`, verbatim, on top of `HOME`, `LOGNAME`, `PATH`, `SHELL`, `TERM` and `USER` only (Vibe passes `env` to the MCP SDK's `StdioServerParameters`, whose `get_default_environment()` keeps just those, `mcp/client/stdio/__init__.py`), so `TOKEN = "${TOKEN}"` passes the literal text; only a bearer token is read from a variable (`api_key_env`). `sync` and `migrate` warn for each server with such a reference. A project `[[mcp_servers]]` entry replaces a user entry of the same name as a whole, so declare the server with its real value in `~/.vibe/config.toml` under another name
- A legacy SSE server is written as `transport = "streamable-http"` with a warning: Vibe's `http` and `streamable-http` are the same transport (Mistral Vibe 2.26.1 `vibe/core/tools/mcp/registry.py:309-312`), and conforme writes no SSE one, so it connects only if the server also speaks streamable HTTP
- `vibe` and `skill-creator` are the names of Vibe's built-in skills: a project skill with either name is skipped by Vibe (`sync` warns)
- Skill `description` is 1 to 1024 characters (`vibe/core/skills/models.py`, `max_length=1024`): a longer one makes Vibe skip the skill, and `validate` warns
- Vibe honours the Codex sidecar `agents/openai.yaml` beside a skill (`allow_implicit_invocation`, `vibe/core/skills/manager.py`): an existing one in `.vibe/skills/<name>/` is kept in step with the source, never created
- Project `.vibe/` directories load only at the trusted working directory, while `.vibe/config.toml` is found by walking up; Vibe also reads `.vibe/plugins/`, `.vibe/hooks.toml` and `.vibe/prompts/`, which conforme does not write
- An untrusted folder loads none of the project files: trust it once before checking the result
- Project subagents in `.vibe/agents` can be spawned since Vibe 2.25.8
- Vibe replaces characters outside `[a-zA-Z0-9_-]` in a server name with `_`; conforme matches servers by the name as written (known gap for names with other characters)
- Vibe's published docs lag its source: they still show `api_key_env` on the server entry rather than in `auth`, the tool name `search_replace` (now `edit`) and no `disable-model-invocation`. conforme follows the source (`vibe/core/config/models.py`, `vibe/core/tools/builtins/edit.py`, `vibe/core/skills/models.py`), and Vibe 2.26.0 loads what conforme writes

## Live check

Without a model call: load the project with Vibe's own config, skill and agent loaders (the Python of the installed package, with `VIBE_HOME` pointing at a directory whose `trusted_folders.toml` lists the project), and list the MCP servers, skills (with their model-invocation flag), subagents and `AGENTS.md` it sees.
