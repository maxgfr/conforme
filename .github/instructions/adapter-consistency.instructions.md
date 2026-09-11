---
applyTo: src/adapters/**, src/mcp.rs, src/skills.rs, docs/providers/**
---

- Every adapter change MUST be reflected in its `docs/providers/<tool>.md`
- Every MCP format change MUST update the corresponding `generate_*_mcp_json` function AND its unit test
- When adding a new adapter, update ALL of: README.md tables, src/help_ai.rs, src/cli.rs tool count, CLAUDE.md architecture section
- Provider docs must list all official documentation URLs for the tool
- Test round-trips: `read()` output fed into `generate()` should produce identical files
- MCP JSON keys per tool: Claude/Kiro/RooCode/AmazonQ/Gemini/Cursor/Continue.dev = `mcpServers`, Copilot = `servers`, OpenCode = `mcp` (inside `opencode.json`), Zed = `context_servers`, Amp = `amp.mcpServers`
- OpenCode MCP specifics: `command` is a single array `[cmd, ...args]`, env key is `environment` (not `env`), servers live inside `opencode.json` at project root (conforme merges — never clobber user-authored keys)
- Windsurf has NO project-level MCP file (Cascade only reads `~/.codeium/windsurf/mcp_config.json`); never generate `.windsurf/mcp.json`
- Never add a user-authored directory (e.g. `.github/prompts/`) to `managed_directories()`: orphan cleanup deletes every file there that conforme did not generate
- An adapter must generate no files for an empty config and never a blank file (guarded by `test_no_adapter_writes_blank_files`)
- An adapter whose upstream tool is retired implements `deprecation_notice()`; `status` and `sync` surface it
- Any adapter that merges into a user-owned settings file (`opencode.json`, `.zed/settings.json`, `.gemini/settings.json`, `.amp/settings.json`, `.codex/config.toml`) MUST implement `is_shared_file()` so `remove` and `migrate` never delete it wholesale
- Amp MCP specifics: dotted `amp.mcpServers` key, no `type` field, merged into `.amp/settings.json` (never clobber user settings)
- Cursor subagents: `.md` extension (not `.mdc`); no `tools` frontmatter field — tool access is inherited from the parent agent
- Copilot skills: `.github/skills/<name>/SKILL.md` (NOT `.github/prompts/*.prompt.md` — prompt files are a separate VS Code feature)
- Any adapter whose `generate()` writes skills, agents, or MCP MUST read them back in `read()`, or `--from <tool>` silently drops them
