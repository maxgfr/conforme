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
- No activation modes, no agents
- Generates and reads project-scoped MCP config in `.codex/config.toml`
- Atomically merges MCP tables without replacing unrelated Codex settings, comments, target-only servers, or Codex-specific server options
- Preserves the shared `.codex/config.toml` during `remove codex` and `migrate --source codex` instead of deleting unrelated settings
- Watches the `.codex/` directory when Codex is configured as the source, including atomic saves and late creation of `config.toml`
- Reads AGENTS.md natively
- Skills in `.agents/skills/` (shared format used by Amp and others)
- `read()` round-trips AGENTS.md, skills (`.agents/skills/`), and MCP servers (`.codex/config.toml`)

## Notes

- **MCP supports both global and project-level** -- configured via TOML in `~/.codex/config.toml` (global) or `.codex/config.toml` (project, requires trust approval)
- MCP TOML format: `[mcp_servers.name]`; stdio uses `command`, `args`, and `env`, while HTTP uses `url` and `http_headers`
- Disabled servers (`enabled = false`) are omitted when migrating from Codex
- Fields outside conforme's normalized transport model (for example `auth`, `bearer_token_env_var`, `env_http_headers`, `cwd`, or `env_vars`) fail migration explicitly rather than silently losing behavior
- Also supports `AGENTS.override.md` for local overrides
- Project-level config at `.codex/config.toml`
- Custom agents at `~/.codex/agents/` (TOML format, global only)
- Codex has subagents (explorer, worker, default) but not user-defined project agents
