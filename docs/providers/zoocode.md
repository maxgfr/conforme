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
- Release notes (Roo-era, stops at 3.50.0): https://docs.zoocode.dev/update-notes
- Releases: https://github.com/Zoo-Code-Org/Zoo-Code/releases
- Roo to Zoo migration (settings export/import only; project `.roo/` files are read as they are): https://docs.zoocode.dev/roo-to-zoo-migration
- Source: https://github.com/Zoo-Code-Org/Zoo-Code
- Roo Code shutdown notice: https://roocodeinc.github.io/Roo-Code/

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Rules | `.roo/rules/*.md` | Plain markdown (NO frontmatter) |
| Skills | `.roo/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` (both required) |
| MCP | `.roo/mcp.json` | JSON: `{ "mcpServers": { ... } }`; HTTP servers use `type: "streamable-http"` (a bare `"http"` is not accepted; `"sse"` is the separate, older SSE transport) |

## Activation modes

No activation modes. All rules are always-on, loaded alphabetically.

Mode-specific rules go in `.roo/rules-{modeSlug}/` directories (e.g., `.roo/rules-code/`, `.roo/rules-architect/`).

## conforme adapter

- File: `src/adapters/zoocode.rs`
- ID: `zoocode`
- Capabilities: skills, MCP
- No activation modes, no agents
- Uses numeric prefixes for ordering: `00-general.md`, `01-rule-name.md`; the 2–3 digit order prefix is stripped on read (`01-security.md` is the rule `security`; `2024-plan.md` stays `2024-plan`)
- Glob/agent-decision info stored as HTML comments (`<!-- Intended scope: ... -->`); the `Intended scope` comment is read back as the rule's globs
- `.roorules` is read as the instructions when `.roo/rules/` is missing or empty; with Zoo as the source it is a `source_files()` entry, so no target writes it and `remove`/`migrate` never delete it. Zoo itself reads `.roorules` (and `.clinerules`) only when no `.roo/rules` directory — the global `~/.roo/rules` included — has files; conforme checks only the project's
- `read()` round-trips rules plus skills (`.roo/skills/`) and MCP (`.roo/mcp.json`)

## Notes

- Plain markdown only -- no YAML frontmatter in rules
- `.roo/rules/` is read **recursively** and files are sorted by base name only (case-insensitive), which is what makes the `00-`/`01-` prefixes meaningful; conforme reads nested rules the same way. Zoo reads every file there except cache and temporary ones (`.bak`, `.log`, `.lock`, `.swp`, `.tmp`, `.DS_Store`, …), so `.txt` and other extensions are rules too; conforme reads back only `.md` (known gap); orphan cleanup and `migrate --source zoocode` leave `.txt` rules in place
- Mode-specific *rules* are controlled via directory placement (`.roo/rules-{modeSlug}/`), not a frontmatter field; mode-specific skills live in `.roo/skills-{modeSlug}/`, which conforme does not generate
- Skills are discovered from project `.roo/skills/` and `.agents/skills/`, plus the global `~/.roo/skills/` and `~/.agents/skills/`
- Custom "modes" are distinct from agents/subagents, but they ARE file-based: a project-level `.roomodes` (YAML or JSON) file at the workspace root (plus a global `custom_modes.yaml`)
- MCP: conforme emits `type: "streamable-http"` for HTTP servers (via `build_zoocode_servers_object`); stdio servers use `command`/`args`/`env`
- Remote (`sse`/`streamable-http`) entries carry no `env`: Zoo's schema requires it to be absent there and rejects the server otherwise
- Zoo writes its own per-server state (`alwaysAllow`, `disabledTools`) into `.roo/mcp.json`, so conforme **merges** the `mcpServers` key: those keys (and any other key conforme does not emit, such as `timeout`, `cwd`, `watchPaths`) survive a sync, `disabled` is reset so a synced server is re-enabled, a file conforme cannot parse (JSONC is accepted) is left untouched, and `remove zoocode` / `migrate --source zoocode` keep the file
- Skills require `name` (equal to the folder name, `^[a-z0-9]+(?:-[a-z0-9]+)*$`, at most 64 characters) and `description` (1–1024 characters); conforme always writes a description, falling back to the name, sanitizes names to that form, and `validate` warns about a longer description
- Zoo's skill loader also reads `mode` / `modeSlugs` (restrict a skill to some modes); conforme does not carry them, so a Zoo skill synced back is available in every mode (known gap)
- Zoo loads a root `AGENTS.md` next to `.roo/rules/` unless `useAgentRules` is off, so in a project that keeps an `AGENTS.md` the instructions reach the model twice
- conforme maps every remote server to its normalized HTTP transport and writes `streamable-http`; an SSE-only server read from another tool is therefore written with the wrong transport (known gap)
- Environment references are written `${env:VAR}` (Zoo's syntax) and read back as `${VAR}`
- Skill manual-invocation keys conforme writes (`disable-model-invocation`, `metadata`, `agents/openai.yaml`) are ignored by Zoo, which has no manual-only mechanism
- Reads AGENTS.md natively
- Detection: `.roo/` or a root `.roorules`. `.clinerules` is Cline's file, which Zoo reads only as a legacy fallback, so it does not make conforme detect Zoo Code (the legacy `.roorules-{modeSlug}` mode-specific files are not read either)
