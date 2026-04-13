# Roo Code / Cline

> VS Code AI extension (fork of Cline). Source: `--from roocode`

## Official docs

- Custom instructions (rules): https://docs.roocode.com/features/custom-instructions
- Skills: https://docs.roocode.com/features/skills
- MCP overview: https://docs.roocode.com/features/mcp/overview
- Using MCP: https://docs.roocode.com/features/mcp/using-mcp-in-roo
- Custom modes: https://docs.roocode.com/features/custom-modes
- FAQ: https://docs.roocode.com/faq

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.roo/rules/*.md` | Plain markdown (NO frontmatter) |
| Skills | `.roo/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` |
| MCP | `.roo/mcp.json` | JSON: `{ "mcpServers": { ... } }` (with `alwaysAllow` field) |

## Activation modes

No activation modes. All rules are always-on, loaded alphabetically.

Mode-specific rules go in `.roo/rules-{modeSlug}/` directories (e.g., `.roo/rules-code/`, `.roo/rules-architect/`).

## conforme adapter

- File: `src/adapters/roocode.rs`
- ID: `roocode`
- Capabilities: skills, MCP
- No activation modes, no agents
- Uses numeric prefixes for ordering: `00-general.md`, `01-rule-name.md`
- Glob/agent-decision info stored as HTML comments (`<!-- Intended scope: ... -->`)

## Notes

- Plain markdown only -- no YAML frontmatter in rules
- Skills support `modes` field (optional array to restrict to specific modes like "code", "architect")
- `.roo/` overrides `.agents/` at same level for skills discovery
- Custom "modes" are different from "agents" -- configured via VS Code settings, not file-based
- Reads AGENTS.md natively
- Also detects `.roorules` and `.clinerules` files
