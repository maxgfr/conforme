# Gemini CLI

> Google's AI CLI tool. Source: `--from gemini`

## Official docs

- GEMINI.md: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/gemini-md.md
- Configuration: https://github.com/google-gemini/gemini-cli/blob/main/docs/get-started/configuration.md
- Skills: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/skills.md
- Skills tutorial: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/tutorials/skills-getting-started.md
- Subagents: https://github.com/google-gemini/gemini-cli/blob/main/docs/core/subagents.md
- MCP: https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md
- MCP tutorial: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/tutorials/mcp-setup.md
- Custom commands: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/custom-commands.md
- Skills repo: https://github.com/google-gemini/gemini-skills
- Google Cloud docs: https://docs.cloud.google.com/gemini/docs/codeassist/gemini-cli

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

## Notes

- **Skills frontmatter: ONLY `name` and `description`** -- Gemini docs say "do not include any other fields"
- **MCP format differs from standard:**
  - No `type` field (neither `stdio` nor `http`)
  - HTTP servers use `httpUrl` (not `url`)
  - Headers supported for HTTP
- Agent frontmatter includes `kind: local` (required)
- Agent frontmatter also supports `temperature`, `max_turns`
- Hierarchical: `~/.gemini/GEMINI.md` -> project -> subdirs
- Supports `@file.md` imports in GEMINI.md
