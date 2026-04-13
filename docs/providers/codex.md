# OpenAI Codex CLI

> OpenAI's AI CLI agent. Source: `--from codex`

## Official docs

- AGENTS.md guide: https://developers.openai.com/codex/guides/agents-md
- Skills: https://developers.openai.com/codex/skills
- MCP configuration: https://developers.openai.com/codex/mcp
- CLI reference: https://developers.openai.com/codex/cli/reference
- CLI features: https://developers.openai.com/codex/cli/features
- Config basics: https://developers.openai.com/codex/config-basic
- Advanced config: https://developers.openai.com/codex/config-advanced
- Config reference: https://developers.openai.com/codex/config-reference
- Changelog: https://developers.openai.com/codex/changelog
- GitHub: https://github.com/openai/codex

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native) | Markdown |
| Skills | `.agents/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| MCP | `~/.codex/config.toml` (global) | TOML: `[mcp_servers.<name>]` (NOT project-level JSON) |

## Activation modes

No activation modes. Reads AGENTS.md natively (all content always-on).

## conforme adapter

- File: `src/adapters/codex.rs`
- ID: `codex`
- Capabilities: skills
- No activation modes, no agents, no project-level MCP
- Reads AGENTS.md natively
- Skills in `.agents/skills/` (shared format used by Amp and others)

## Notes

- **MCP is global only** -- configured via TOML in `~/.codex/config.toml`, not project-level JSON
- MCP TOML format: `[mcp_servers.name]` with `command`, `bearer_token_env_var`, `startup_timeout_sec`, `tool_timeout_sec`, `enabled`
- Also supports `AGENTS.override.md` for local overrides
- Project-level config at `.codex/config.toml`
- Custom agents at `~/.codex/agents/` (TOML format, global only)
- Codex has subagents (explorer, worker, default) but not user-defined project agents
