# OpenAI Codex CLI

> OpenAI's AI CLI agent. Source: `--from codex`

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
- Skills in `.agents/skills/` (shared format used by Amp and others)
- `read()` round-trips AGENTS.md, skills (`.agents/skills/`), and MCP servers (`.codex/config.toml`); the file is parsed with conforme's AGENTS.md convention (instructions plus `## Rule:` sections), and its `## Skill:`/`## Agent:`/`## MCP:` sections are added to what this tool's own files hold (which win on a name clash). With this tool as the source, AGENTS.md *is* the source: sync never regenerates it (`generate_agents_md` does not apply) and `gitignore install` keeps it tracked

## Notes

- **MCP supports both global and project-level** -- configured via TOML in `~/.codex/config.toml` (global) or `.codex/config.toml` (project, requires trust approval)
- MCP TOML format: `[mcp_servers.name]`; stdio uses `command`, `args`, and `env`, while HTTP uses `url` and `http_headers`
- Codex has no `${VAR}` interpolation: it forwards variables by name (`env_vars`, `bearer_token_env_var`, `env_http_headers`). Values are written literally
- Disabled servers (`enabled = false`) are omitted when migrating from Codex
- Fields outside conforme's normalized transport model (for example `auth`, `bearer_token_env_var`, `env_http_headers`, `cwd`, or `env_vars`) fail migration explicitly rather than silently losing behavior
- When a server switches from HTTP to stdio, the merge drops every key Codex rejects on a stdio server ("X is not supported for stdio"): `url`, `http_headers`, `env_http_headers`, `http_headers_helper`, `bearer_token_env_var`, `bearer_token`, `auth`, `oauth`, `oauth_resource`. Switching to HTTP drops `command`, `args`, `env`, `cwd`, `env_vars`
- Skills are always written with a `description`; the Codex skill parser rejects a skill without one, so an empty description falls back to the name
- Codex reads at most one file per directory: `AGENTS.override.md` when it exists, otherwise `AGENTS.md`. The override is a personal, local file, so conforme's `read()` deliberately uses the shared `AGENTS.md` as the source and never propagates the override; conforme writes neither file
- Skills are searched recursively, up to 6 levels below `.agents/skills/`; conforme reads nested skill folders too (written back flat). Skill names longer than 64 characters are rejected upstream; `sanitize_name` caps them
- The Codex source also scans `<project>/.codex/skills`, which its docs do not mention; conforme does not use it
- Project-level config at `.codex/config.toml`
- Custom agents are TOML files in `~/.codex/agents/` **and** project-scoped `.codex/agents/*.toml` (required keys `name`, `description`, `developer_instructions`). conforme does not generate Codex agents yet (known gap)
- `developers.openai.com/codex/*` now 308-redirects to `learn.chatgpt.com/docs/*`

## Manual skill invocation

Manual skills preserve `disable-model-invocation: true`, OpenCode V2 `metadata.opencode/autoinvoke: "false"`, and Codex `agents/openai.yaml` with `policy.allow_implicit_invocation: false` through synchronization. In AGENTS.md, use `<!-- invocation: manual -->` in the skill section. OpenCode V1 still needs the corresponding `permission.skill` deny entries; skill synchronization does not change user permissions.
