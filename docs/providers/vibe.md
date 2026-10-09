# Mistral Vibe

> Mistral's open-source coding agent CLI (`vibe`). Source: `--from vibe`

## Official docs

- Overview: https://docs.mistral.ai/vibe/code/overview
- Configuration: https://docs.mistral.ai/vibe/code/cli/configuration
- Configuration reference: https://docs.mistral.ai/vibe/code/cli/configuration-reference
- Skills: https://docs.mistral.ai/vibe/code/cli/skills
- Agents: https://docs.mistral.ai/vibe/code/cli/agents
- MCP servers: https://docs.mistral.ai/vibe/code/cli/mcp-servers
- Repository: https://github.com/mistralai/mistral-vibe

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native), from the working directory up to the trusted root | Markdown |
| Skills | `.vibe/skills/<name>/SKILL.md`, then `.agents/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description`, `disable-model-invocation`, `user-invocable`, `allowed-tools` |
| Agents | `.vibe/agents/<name>.toml` | TOML: `agent_type` (`subagent` for a subagent; other files are modes), `description`, `instructions` or `system_prompt_id` (`.vibe/prompts/<id>.md`), `enabled_tools`, `model` |
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
- Subagents are written to `.vibe/agents/<name>.toml` (`agent_type = "subagent"`, `description`, `instructions`, `enabled_tools` translated to Vibe tool names: `read_file`, `grep`, `write_file`, `edit`, `bash`, `web_fetch`, `web_search`, `todo`, `task`, `skill`, `ask_user_question`). `model` is not written: Vibe resolves it against the user's own model list. An agent file that is not a subagent is a mode the user defined and is never cleaned
- MCP is merged into `.vibe/config.toml` (atomic write, comments and every other setting kept): a server is matched by `name`; stdio is `transport = "stdio"` with `command`, `args`, `env`; HTTP is `transport = "streamable-http"` with `url`; an `Authorization: Bearer ${VAR}` header becomes `auth = { type = "static", api_key_env = "VAR", api_key_header = "Authorization", api_key_format = "Bearer {token}" }`, other headers go to `auth.headers`; an OAuth `auth`, servers only the target lists and keys conforme does not write are kept, and `disabled` is reset
- `read()` round-trips skills, subagents (`instructions`, or the prompt file `system_prompt_id` names), MCP (static `auth` read back as `Bearer ${VAR}`, a command given as a list, disabled servers skipped) and `AGENTS.md`

## Notes

- **No `${VAR}` expansion.** A stdio server gets only the variables its `env` sets, verbatim, so `TOKEN = "${TOKEN}"` passes the literal text; only a bearer token is read from a variable (`api_key_env`). `sync` and `migrate` warn for each server with such a reference. A project `[[mcp_servers]]` entry replaces a user entry of the same name as a whole, so declare the server with its real value in `~/.vibe/config.toml` under another name
- `vibe` and `skill-creator` are the names of Vibe's built-in skills: a project skill with either name is skipped by Vibe (`sync` warns)
- An untrusted folder loads none of the project files: trust it once before checking the result
- Project subagents in `.vibe/agents` can be spawned since Vibe 2.25.8
- Vibe replaces characters outside `[a-zA-Z0-9_-]` in a server name with `_`; conforme matches servers by the name as written (known gap for names with other characters)
- Vibe's published docs lag its source: they still show `api_key_env` on the server entry rather than in `auth`, the tool name `search_replace` (now `edit`) and no `disable-model-invocation`. conforme follows the source (`vibe/core/config/models.py`, `vibe/core/tools/builtins/edit.py`, `vibe/core/skills/models.py`), and Vibe 2.26.0 loads what conforme writes

## Live check

Without a model call: load the project with Vibe's own config, skill and agent loaders (the Python of the installed package, with `VIBE_HOME` pointing at a directory whose `trusted_folders.toml` lists the project), and list the MCP servers, skills (with their model-invocation flag), subagents and `AGENTS.md` it sees.
