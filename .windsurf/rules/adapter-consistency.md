---
description: adapter-consistency
globs: src/adapters/**, src/mcp.rs, src/skills.rs, docs/providers/**
trigger: glob
---

- Every adapter change MUST be reflected in its `docs/providers/<tool>.md`
- Every MCP format change MUST update the corresponding `generate_*_mcp_json` function AND its unit test
- When adding a new adapter, update ALL of: README.md tables, src/help_ai.rs, src/cli.rs tool count, CLAUDE.md architecture section
- Provider docs must list all official documentation URLs for the tool
- Test round-trips: `read()` output fed into `generate()` should produce identical files
- MCP JSON keys per tool: Claude/Windsurf/Kiro/RooCode/AmazonQ/Gemini = `mcpServers`, Copilot = `servers`, OpenCode = `mcp`, Zed = `context_servers`, Amp = `amp.mcpServers`
