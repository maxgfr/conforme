# OpenAI Codex CLI

> OpenAI's AI CLI agent. Source: `--from codex`

**Last verified online:** 2026-10-09, against Codex CLI rust-v0.162.0 (https://github.com/openai/codex/releases)

## Official docs

- AGENTS.md guide: https://learn.chatgpt.com/docs/agent-configuration/agents-md
- Skills: https://learn.chatgpt.com/docs/build-skills
- MCP configuration: https://learn.chatgpt.com/docs/extend/mcp
- CLI reference: https://learn.chatgpt.com/docs/developer-commands
- CLI features: https://learn.chatgpt.com/docs/codex/cli
- Config basics: https://learn.chatgpt.com/docs/config-file/config-basic
- Advanced config: https://learn.chatgpt.com/docs/config-file/config-advanced
- Config reference: https://learn.chatgpt.com/docs/config-file/config-reference
- Subagents: https://learn.chatgpt.com/docs/agent-configuration/subagents
- Changelog: https://learn.chatgpt.com/docs/changelog
- GitHub: https://github.com/openai/codex
- Annotated `config.toml` (`[mcp_servers.*]`, `project_doc_fallback_filenames`, `trust_level`): https://learn.chatgpt.com/docs/config-file/config-sample
- Approvals and security (`trust_level = "untrusted"` disables project config): https://learn.chatgpt.com/docs/agent-approvals-security
- Environment variables (`CODEX_HOME`): https://learn.chatgpt.com/docs/config-file/environment-variables
- Custom prompts (deprecated for skills, user-level only): https://learn.chatgpt.com/docs/custom-prompts
- Command rules (Starlark `.codex/rules/*.rules`, not instruction rules): https://learn.chatgpt.com/docs/agent-configuration/rules

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native) | Markdown |
| Skills | `.agents/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| MCP | `~/.codex/config.toml` (global) or `.codex/config.toml` (project) | TOML: `[mcp_servers.<name>]` (NOT JSON) |

## Activation modes

No activation modes. Reads AGENTS.md natively (all content always-on).

## conforme adapter

- File: `src/adapters/codex.rs`
- ID: `codex`
- Capabilities: skills, MCP
- No activation modes, no agents (project `.codex/agents/*.toml` exists upstream but is not generated)
- Generates and reads project-scoped MCP config in `.codex/config.toml`
- Atomically merges MCP tables without replacing unrelated Codex settings, comments, target-only servers, or Codex-specific server options
- Preserves the shared `.codex/config.toml` during `remove codex` and `migrate --source codex` instead of deleting unrelated settings
- Watches the `.codex/` directory when Codex is configured as the source, including atomic saves and late creation of `config.toml`
- Reads AGENTS.md natively
- Skills in `.agents/skills/` (shared format, also read by Zed, Gemini CLI, OpenCode, Kilo Code and Mistral Vibe)
- `read()` round-trips AGENTS.md, skills (`.agents/skills/`), and MCP servers (`.codex/config.toml`); the file is parsed with conforme's AGENTS.md convention (instructions plus `## Rule:` sections), and its `## Skill:`/`## Agent:`/`## MCP:` sections are added to what this tool's own files hold (which win on a name clash). With this tool as the source, AGENTS.md *is* the source: sync never regenerates it (`generate_agents_md` does not apply), `gitignore install` keeps it tracked, and `remove`/`migrate` never delete it (`source_files()`)

## Notes

- **MCP supports both global and project-level** -- configured via TOML in `~/.codex/config.toml` (global) or `.codex/config.toml` (project, requires trust approval)
- MCP TOML format: `[mcp_servers.name]`; stdio uses `command`, `args`, `env` and `env_vars`, while HTTP uses `url`, `http_headers`, `env_http_headers` and `bearer_token_env_var`
- Codex has no `${VAR}` interpolation: it forwards variables by name. conforme writes a stdio `NAME=${NAME}` as `env_vars = ["NAME"]` (other `env` values are literal), `Authorization: Bearer ${VAR}` as `bearer_token_env_var = "VAR"`, a header that is exactly `${VAR}` as `env_http_headers.<Header> = "VAR"`, and other headers as `http_headers`; all of them read back to `${VAR}`. `env_vars` object entries with `source = "remote"` are Codex-only and kept by the merge. A reference Codex cannot express (`NAME=${OTHER}`, a `${VAR}` mixed into other text) is written literally, without a warning (known gap)
- `env` on an HTTP server is dropped, as in every JSON shape without `env` on remote servers
- Disabled servers (`enabled = false`) are omitted when migrating from Codex
- Codex tuning keys are accepted on read and ignored (the merge keeps them): `enabled`, `required`, `startup_timeout_sec`, `startup_timeout_ms`, `tool_timeout_sec`, `enabled_tools`, `disabled_tools`, `default_tools_approval_mode`, `tools`, `scopes`, the `[oauth]` client table (`client_id`, `client_secret`, `callback_port`, …), `oauth_resource`, `startup_readiness`, `supports_parallel_tool_calls`, `tool_input_schema_max_bytes`, `omit_tools_from`, the legacy `name`, and `environment_id = "local"` (a server bound to another environment is a remote executor no other tool has, so `--from codex` still fails on it; Codex's docs call that key `experimental_environment`, its source only `environment_id`, which conforme follows)
- Fields outside conforme's normalized transport model (for example `auth`, `cwd`, `http_headers_helper`, or `env_vars` entries with `source = "remote"`) fail migration explicitly rather than silently losing behavior
- When a server switches from HTTP to stdio, the merge drops every key Codex rejects on a stdio server ("X is not supported for stdio"): `url`, `http_headers`, `env_http_headers`, `http_headers_helper`, `bearer_token_env_var`, `bearer_token`, `auth`, `oauth`, `oauth_resource`. Switching to HTTP drops `command`, `args`, `env`, `cwd`, `env_vars`, and the legacy `bearer_token` Codex rejects on HTTP too (`throw_if_set("streamable_http", "bearer_token", ..)`, `codex-rs/config/src/mcp_types.rs`)
- Skills are always written with a `description`; the Codex skill parser rejects a skill without one, so an empty description falls back to the name. It caps only the name (64 characters), not the description (`codex-rs/skills/src/parser.rs`)
- Codex reads at most `project_doc_max_bytes` (32 KiB by default) of instruction files and truncates the rest ("project doc exceeds remaining budget; truncating", `codex-rs/core/src/agents_md.rs`); conforme does not warn about a larger `AGENTS.md` (known gap)
- Codex reads at most one file per directory: `AGENTS.override.md` when it exists, otherwise `AGENTS.md`. The override is a personal, local file, so conforme's `read()` deliberately uses the shared `AGENTS.md` as the source and never propagates the override; conforme writes neither file
- Skills are searched recursively, up to 6 levels below `.agents/skills/`; conforme reads nested skill folders too (written back flat). Skill names longer than 64 characters are rejected upstream; `sanitize_name` caps them
- The Codex source also scans `<project>/.codex/skills`, which its docs do not mention; conforme does not use it
- Project-level config at `.codex/config.toml`
- Custom agents are TOML files in `~/.codex/agents/` **and** project-scoped `.codex/agents/*.toml` (required keys `name`, `description`, `developer_instructions`). conforme does not generate Codex agents yet (known gap)
- `developers.openai.com/codex/*` now 308-redirects to `learn.chatgpt.com/docs/*`

## Manual skill invocation

Manual skills preserve `disable-model-invocation: true`, `metadata.opencode/autoinvoke: "false"`, and Codex `agents/openai.yaml` with `policy.allow_implicit_invocation: false` through synchronization. In AGENTS.md, use `<!-- invocation: manual -->` in the skill section. conforme writes `metadata.opencode/autoinvoke` as a hint that OpenCode is not known to read (no reader exists on its `dev` branch), so OpenCode needs the corresponding `permission.skill` deny entries; skill synchronization does not change user permissions.
