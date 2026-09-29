---
trigger: always_on
---

# CLAUDE.md

## Project overview

conforme is a Rust CLI that synchronizes AI coding agent configurations across 12 tools. It reads config from a source tool (Claude Code, Cursor, etc.) or AGENTS.md, and propagates to all other tool-specific config files.

## Build & test

```bash
cargo build --release
cargo test
cargo clippy -- -D warnings    # lint — MUST pass before pushing
cargo fmt -- --check           # format check
conforme check                 # verify AI configs are in sync (dogfooding)
```

## Architecture

```
src/
  main.rs           — Entry point, dispatches subcommands
  cli.rs            — CLI arg parsing (clap derive)
  config.rs         — NormalizedConfig, NormalizedRule, NormalizedSkill, NormalizedAgent, NormalizedMcpServer, ActivationMode
  markdown.rs       — AGENTS.md parser (sections → rules via ## Rule: headings)
  frontmatter.rs    — gray_matter wrapper for YAML frontmatter parsing/serialization
  lib.rs            — Library crate re-exports (adapters, config, etc.)
  sync.rs           — Core sync engine: init, sync, check, status, remove commands
  detect.rs         — Tool detection (which tools present in project)
  hash.rs           — SHA-256 content hashing for change detection
  json_settings.rs  — JSONC-safe merge of conforme's keys into user-owned JSON settings files
                       (in-place edit keeps comments; unparsable file is refused, never overwritten;
                       merge_server_entries keeps per-server keys conforme does not own)
  hook.rs           — Git pre-commit hook install/uninstall (like Husky)
  project_config.rs  — .conformerc.toml parser (source, only, exclude, clean options)
  validate.rs        — Config validation (duplicate names, empty content, invalid globs)
  watch.rs           — File watcher for auto-sync (notify + debounce)
  help_ai.rs        — Detailed help about all supported tools and formats
  mcp.rs            — MCP config generation/parsing per tool:
                       - Codex: project `.codex/config.toml`, atomically merged with comment/settings preservation; strict safe parser — merge_codex_mcp_toml / parse_codex_mcp_toml
                       - Standard mcpServers: Claude, Kiro, Cursor
                       - Zoo Code: mcpServers, HTTP uses type "streamable-http" (not "http") — generate_zoocode_mcp_json
                       - Claude .mcp.json parsing accepts http/https, sse, streamable-http, and ws transports (all mapped to the HTTP variant)
                       - Copilot: "servers" key (env + headers supported)
                       - OpenCode: "mcp" key merged into opencode.json, type local/remote, command as array, `environment` key
                       - Zed: "context_servers" key
                       - Gemini: mcpServers, no type field, httpUrl for HTTP
                       - Amp: "amp.mcpServers" key merged into .amp/settings.json — build_amp_mcp_object
                       - parse_mcp_json reads back mcpServers / servers / context_servers / amp.mcpServers
                       - OpenCode needs its own inverse (parse_opencode_mcp_object / parse_opencode_agent_object)
  skills.rs         — Skills (SKILL.md) and agents generation per tool; shared read helpers
                       preserve manual_invocation and Codex policy sidecars across sync
                       (read_skills_from_dir, read_agents_from_dir, parse_frontmatter_tool_list)
                       used by adapters to round-trip skills/agents on read()
  adapters/
    mod.rs          — AiToolAdapter trait + registry + shared write_if_changed +
                       collect_rule_files (recursive rules-dir scan, sorted by base name)
    claude.rs       — Claude Code: CLAUDE.md (or .claude/CLAUDE.md when only that exists)
                       + .claude/rules/**/*.md, read recursively (paths: frontmatter)
    cursor.rs       — Cursor: .cursor/rules/**/*.mdc, read recursively (alwaysApply/globs/description); subagents at .cursor/agents/*.md
    windsurf.rs     — Windsurf (now Devin Desktop): .devin/{rules,skills} when .devin/ exists, else .windsurf/{rules,skills} (trigger/description/globs)
    copilot.rs      — GitHub Copilot: .github/copilot-instructions.md (applyTo); skills at .github/skills/<name>/SKILL.md; MCP merged into .vscode/mcp.json
    codex.rs        — OpenAI Codex CLI: reads AGENTS.md natively
    opencode.rs     — OpenCode: reads AGENTS.md natively
    zoocode.rs      — Zoo Code (community fork of Roo Code): .roo/rules/**/*.md, read recursively (plain Markdown)
    gemini.rs       — Gemini CLI: GEMINI.md
    zed.rs          — Zed AI: .rules file
    kiro.rs         — Kiro (AWS): .kiro/steering/*.md (inclusion/fileMatchPattern)
    amp.rs          — Amp (Sourcegraph): reads AGENTS.md natively
    deepseek.rs     — DeepSeek Harness (dsh): reads AGENTS.md natively; skills at .dsh/skills/<name>/SKILL.md
tests/
  integration.rs    — CLI integration tests (assert_cmd + tempfile)
  roundtrip.rs      — Write→read round-trip tests for every adapter (rules, skills, agents, MCP)
  error_cases.rs    — Edge cases, MCP sync, agents sync, activation modes
docs/
  providers/        — One doc per supported tool with official URLs, config format, adapter notes
```

## Keeping docs in sync

**IMPORTANT**: When adding/removing adapters, commands, or changing behavior, update ALL of:
1. `README.md` — supported tools table, CLI commands, activation mapping table
2. `src/help_ai.rs` — the `conforme help-ai` output must list all tools with correct formats
3. `src/cli.rs` — the `after_help` examples and `long_about` tool count
4. `src/project_config.rs` — if adding new config options
5. `src/validate.rs` — if adding new validation rules
6. `src/watch.rs` — if changing watched file patterns
7. `docs/providers/<tool>.md` — provider-specific documentation
8. This `CLAUDE.md` — architecture section and test count

## Key concepts

### AGENTS.md convention

conforme uses `## Rule: <name>` headings and HTML comments for activation metadata:

```markdown
# Instructions
General instructions.

## Rule: TypeScript
<!-- activation: glob **/*.ts,**/*.tsx -->
Content.
```

Activation modes: `always`, `glob <patterns>`, `agent-decision`, `manual`.
If no activation comment: defaults to `always`.

Also supports `## Skill:`, `## Agent:`, and `## MCP:` sections:

```markdown
## Skill: deploy
<!-- description: Deploy the app -->
<!-- invocation: manual -->
<!-- tools: Bash -->
Run npm run deploy.

## Agent: reviewer
<!-- description: Code review -->
<!-- model: gpt-4o -->
<!-- tools: codebase -->
<!-- color: cyan -->
<!-- permission-mode: plan -->
Review for bugs.

## MCP: filesystem
<!-- command: npx -->
<!-- args: -y, @mcp/server-filesystem -->
```

### Adapter categories

**Per-rule adapters** (have frontmatter or per-file rules):
- Claude, Cursor, Windsurf, Copilot, Kiro, Zoo Code

**Single-file adapters** (merge all content into one file):
- Codex, OpenCode, Gemini, Zed, Amp, DeepSeek Harness

### Adapter mapping

| Activation | Claude | Cursor | Windsurf | Copilot | Kiro |
|---|---|---|---|---|---|
| Always | in CLAUDE.md | `alwaysApply: true` | `trigger: always_on` | in main file | `inclusion: always` |
| GlobMatch | `paths: [globs]` | `globs:` | `trigger: glob` | `applyTo:` | `inclusion: fileMatch` |
| AgentDecision | no frontmatter | `description:` | `trigger: model_decision` | in main file | `inclusion: auto` |
| Manual | no frontmatter | `alwaysApply: false` | `trigger: manual` | in main file | `inclusion: manual` |

### MCP key mapping per tool

| Tool | JSON key | Notes |
|---|---|---|
| Claude, Kiro, Cursor | `mcpServers` | Standard format with `type: stdio/http` |
| Zoo Code | `mcpServers` (inside `.roo/mcp.json`) | HTTP uses `type: streamable-http` (not `http`), no `env` on remote servers; merged (keeps Zoo's `alwaysAllow`/`disabledTools`) |
| Copilot | `servers` (inside `.vscode/mcp.json`) | VS Code format; supports `env` + `headers`; merged (keeps `inputs`/`sandbox`) |
| Windsurf | _(none)_ | Cascade only reads the user-global `~/.config/devin/mcp_config.json`; nothing project-scoped is generated |
| OpenCode | `mcp` (inside `opencode.json`) | `type: local/remote`; `command` is a single array; env key is `environment`; merged (preserves user keys) |
| Zed | `context_servers` (inside `.zed/settings.json`) | No type field; merged into existing settings (preserves theme/keybindings/etc.) |
| Gemini | `mcpServers` (inside `.gemini/settings.json`) | No type field, uses `httpUrl` for HTTP; merged into existing settings |
| Amp | `amp.mcpServers` (inside `.amp/settings.json`) | Dotted key; no type field; merged into existing settings |
| DeepSeek Harness | _(none)_ | MCP is a user-level `cordis.patch.yml` plugin entry under `$DSH_HOME`; nothing project-scoped is generated |
| Codex | `[mcp_servers.<name>]` (inside `.codex/config.toml`) | TOML; atomic merge preserves unrelated settings, comments, target-only servers, and Codex-specific options; shared file is never deleted wholesale |

### Rules-directory discovery

Claude Code, Cursor and Zoo Code all scan their rules directory **recursively**
(`.claude/rules/frontend/react.md`, `.cursor/rules/backend/rpc.mdc`, …).
`adapters::collect_rule_files` implements that scan for all three, sorting by
base name (case-insensitive, so Zoo's `00-`/`01-` prefixes keep their meaning)
then by full path. Nested rules are written back flat, one file per rule name.
`clean_orphans` stays non-recursive, so hand-authored files in subdirectories are
never deleted.

### Orphan cleanup and shared files

`managed_directories()` returns `ManagedDir`s: each names the file suffix conforme
writes there (`.md`, `.mdc`, `.instructions.md`, `.agent.md`), and orphan cleanup
only deletes top-level files with that suffix. Skills directories are
`ManagedDir::subdirs` (conforme only writes `<name>/SKILL.md` folders), so no
top-level file there is ever swept. Files a tool accepts but conforme never
writes (Kiro `.json` agents, dsh flat `<name>.md` skills, plain `.md` Copilot
agents) therefore survive a sync.

Every JSON file conforme merges into (`opencode.json`, `.zed/settings.json`,
`.gemini/settings.json`, `.amp/settings.json`, `.vscode/mcp.json`, `.roo/mcp.json`)
goes through `json_settings`, and together with `.codex/config.toml` is declared by
`is_shared_file()` so `remove`/`migrate` never delete it wholesale.

### Sync algorithm

1. Parse AGENTS.md → NormalizedConfig
2. For each detected adapter: generate expected files, write if changed
3. Change detection uses SHA-256 content hashing

### Source-based flow

conforme can read config from any tool, not just AGENTS.md:

1. `--from` CLI flag (highest priority)
2. `source` in `.conformerc.toml`
3. AGENTS.md fallback (backward compatible)

Configure via `.conformerc.toml`:
```toml
source = "claude"
only = ["cursor", "copilot"]
exclude = ["zed"]
generate_agents_md = true
clean = true
```

### Pre-commit hook

`conforme hook install` installs a git pre-commit hook that runs `conforme check`.
Works alongside existing hooks (appends/removes its own block).

## CLI commands

```
conforme init [--force]                    # Create AGENTS.md + sync to tools
conforme sync [--dry-run] [--only tools]   # AGENTS.md → all tool configs
conforme check                             # Exit 0 if in sync, 1 if not
conforme status                            # Show detected tools + sync state
conforme remove <tools>                    # Remove generated config files for tools
conforme hook install                      # Install git pre-commit hook
conforme hook uninstall                    # Remove git pre-commit hook
conforme help-ai                           # Show all supported tools + formats
conforme diff                              # Show diff between expected and actual
conforme add rule|skill|agent|mcp          # Add section to AGENTS.md
conforme watch                             # Watch source and auto-sync
conforme sync --from <tool>                # Use specific tool as source
conforme sync --no-clean                   # Don't clean orphan files
conforme migrate --source X --output Y    # Migrate config between tools
```

## Skills

This project uses Claude Code skills in `.claude/skills/`:

- **verify-providers** — Audit all 12 provider adapters: upstream product still alive, official docs vs adapter code, safety invariants (managed dirs, shared settings files, round-trip, blank output), link check; then fix with regression tests

## MCP servers (.mcp.json)

This project has a `.mcp.json` with **Context7** configured. Use it to get up-to-date documentation for any library or framework when working on adapter logic.

**When verifying or updating adapter formats**, use Context7 to check the latest docs for any AI coding tool.

## Dogfooding

This project uses conforme on itself (`source = "claude"` in `.conformerc.toml`).
CI runs `conforme check` to ensure all tool configs stay in sync.
The pre-commit hook enforces sync locally.

## Versioning

Managed by semantic-release. The `.version-hook.sh` script updates `Cargo.toml` during release.

## CI/CD

- **ci.yml**: build, unit tests, integration tests, clippy, fmt, audit, macOS smoke test, `conforme check`
- **release.yml**: matrix build (linux-x64, linux-arm64, macos-x64, macos-arm64, windows-x64, windows-arm64), semantic-release, upload binaries
- Homebrew formula in `maxgfr/homebrew-tap`
