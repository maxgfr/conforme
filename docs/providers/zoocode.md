# Zoo Code

> VS Code AI extension, the community fork of Roo Code. Source: `--from zoocode`

Roo Code was shut down on 2026-05-15 (its repository, `RooCodeInc/Roo-Code`, is
archived; the docs homepage points users to Zoo Code). Zoo Code continues the
same extension and reads exactly the same `.roo/` files, so conforme targets it
under the `zoocode` id. The former `roocode` id no longer exists.

## Official docs

- Custom instructions (rules): https://docs.zoocode.dev/features/custom-instructions
- Skills: https://docs.zoocode.dev/features/skills
- MCP overview: https://docs.zoocode.dev/features/mcp/overview
- Using MCP: https://docs.zoocode.dev/features/mcp/using-mcp-in-roo
- MCP transports: https://docs.zoocode.dev/features/mcp/server-transports
- Custom modes: https://docs.zoocode.dev/features/custom-modes
- FAQ: https://docs.zoocode.dev/faq
- Release notes: https://docs.zoocode.dev/update-notes
- Source: https://github.com/Zoo-Code-Org/Zoo-Code
- Roo Code shutdown notice: https://roocodeinc.github.io/Roo-Code/

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.roo/rules/*.md` | Plain markdown (NO frontmatter) |
| Skills | `.roo/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` (both required) |
| MCP | `.roo/mcp.json` | JSON: `{ "mcpServers": { ... } }`; HTTP servers use `type: "streamable-http"` (a bare `"http"` is not accepted; legacy alias is `"sse"`) |

## Activation modes

No activation modes. All rules are always-on, loaded alphabetically.

Mode-specific rules go in `.roo/rules-{modeSlug}/` directories (e.g., `.roo/rules-code/`, `.roo/rules-architect/`).

## conforme adapter

- File: `src/adapters/zoocode.rs`
- ID: `zoocode`
- Capabilities: skills, MCP
- No activation modes, no agents
- Uses numeric prefixes for ordering: `00-general.md`, `01-rule-name.md`
- Glob/agent-decision info stored as HTML comments (`<!-- Intended scope: ... -->`)
- `read()` round-trips rules plus skills (`.roo/skills/`) and MCP (`.roo/mcp.json`)

## Notes

- Plain markdown only -- no YAML frontmatter in rules
- `.roo/rules/` is read **recursively** and files are sorted by base name only (case-insensitive), which is what makes the `00-`/`01-` prefixes meaningful; conforme reads nested rules the same way. Zoo reads every file type there, conforme reads back only `.md`
- Mode-specific *rules* are controlled via directory placement (`.roo/rules-{modeSlug}/`), not a frontmatter field; mode-specific skills live in `.roo/skills-{modeSlug}/`, which conforme does not generate
- Skills are discovered from project `.roo/skills/` and `.agents/skills/`, plus the global `~/.roo/skills/` and `~/.agents/skills/`
- Custom "modes" are distinct from agents/subagents, but they ARE file-based: a project-level `.roomodes` (YAML or JSON) file at the workspace root (plus a global `custom_modes.yaml`)
- MCP: conforme emits `type: "streamable-http"` for HTTP servers (via `build_zoocode_servers_object`); stdio servers use `command`/`args`/`env`
- Remote (`sse`/`streamable-http`) entries carry no `env`: Zoo's schema requires it to be absent there and rejects the server otherwise
- Zoo writes its own per-server state (`alwaysAllow`, `disabledTools`) into `.roo/mcp.json`, so conforme **merges** the `mcpServers` key: those keys (and any other key conforme does not emit, such as `timeout`, `cwd`, `watchPaths`) survive a sync, `disabled` is reset so a synced server is re-enabled, a file conforme cannot parse (JSONC is accepted) is left untouched, and `remove zoocode` / `migrate --source zoocode` keep the file
- Skills require `name` (equal to the folder name) and `description`; conforme always writes a description, falling back to the name
- Skill manual-invocation keys conforme writes (`disable-model-invocation`, `metadata`, `agents/openai.yaml`) are ignored by Zoo, which has no manual-only mechanism
- Reads AGENTS.md natively
- Also detects `.roorules` and `.clinerules` files (and the legacy `.roorules-{modeSlug}` mode-specific files)
