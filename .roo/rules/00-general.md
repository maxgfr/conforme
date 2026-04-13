# CLAUDE.md

## Project overview

conforme is a Rust CLI that synchronizes AI coding agent configurations across 13 tools. It reads config from a source tool (Claude Code, Cursor, etc.) or AGENTS.md, and propagates to all other tool-specific config files.

## Build & test

```bash
cargo build --release
cargo test                     # 190 tests (112 unit + 50 integration + 17 error + 9 roundtrip)
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
  hook.rs           — Git pre-commit hook install/uninstall (like Husky)
  project_config.rs  — .conformerc.toml parser (source, only, exclude, clean options)
  validate.rs        — Config validation (duplicate names, empty content, invalid globs)
  watch.rs           — File watcher for auto-sync (notify + debounce)
  help_ai.rs        — Detailed help about all supported tools and formats
  mcp.rs            — MCP config generation/parsing per tool:
                       - Standard mcpServers: Claude, Windsurf, Kiro, Roo, Amazon Q
                       - Copilot: "servers" key
                       - OpenCode: "mcp" key, type local/remote
                       - Zed: "context_servers" key
                       - Gemini: mcpServers, no type field, httpUrl for HTTP
                       - Amp: "amp.mcpServers" key
  skills.rs         — Skills (SKILL.md) and agents generation per tool
  adapters/
    mod.rs          — AiToolAdapter trait + registry + shared write_if_changed
    claude.rs       — Claude Code: CLAUDE.md + .claude/rules/*.md (paths: frontmatter)
    cursor.rs       — Cursor: .cursor/rules/*.mdc (alwaysApply/globs/description)
    windsurf.rs     — Windsurf: .windsurf/rules/*.md (trigger/description/globs)
    copilot.rs      — GitHub Copilot: .github/copilot-instructions.md (applyTo)
    codex.rs        — OpenAI Codex CLI: reads AGENTS.md natively
    opencode.rs     — OpenCode: reads AGENTS.md natively
    roocode.rs      — Roo Code / Cline: .roo/rules/*.md (plain Markdown)
    gemini.rs       — Gemini CLI: GEMINI.md
    continuedev.rs  — Continue.dev: .continue/rules/*.md (name/globs/alwaysApply)
    zed.rs          — Zed AI: .rules file
    amazonq.rs      — Amazon Q: .amazonq/rules/*.md
    kiro.rs         — Kiro (AWS): .kiro/steering/*.md (inclusion/fileMatchPattern)
    amp.rs          — Amp (Sourcegraph): reads AGENTS.md natively
tests/
  integration.rs    — CLI integration tests (assert_cmd + tempfile)
  roundtrip.rs      — Write→read round-trip tests for all per-rule adapters
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
<!-- tools: Bash -->
Run npm run deploy.

## Agent: reviewer
<!-- description: Code review -->
<!-- model: gpt-4o -->
<!-- tools: codebase -->
Review for bugs.

## MCP: filesystem
<!-- command: npx -->
<!-- args: -y, @mcp/server-filesystem -->
```

### Adapter categories

**Per-rule adapters** (have frontmatter or per-file rules):
- Claude, Cursor, Windsurf, Copilot, Continue.dev, Kiro, Roo Code, Amazon Q

**Single-file adapters** (merge all content into one file):
- Codex, OpenCode, Gemini, Zed, Amp

### Adapter mapping

| Activation | Claude | Cursor | Windsurf | Copilot | Continue | Kiro |
|---|---|---|---|---|---|---|
| Always | in CLAUDE.md | `alwaysApply: true` | `trigger: always_on` | in main file | `alwaysApply: true` | `inclusion: always` |
| GlobMatch | `paths: [globs]` | `globs:` | `trigger: glob` | `applyTo:` | `globs: [array]` | `inclusion: fileMatch` |
| AgentDecision | no frontmatter | `description:` | `trigger: model_decision` | in main file | `description:` | `inclusion: auto` |
| Manual | no frontmatter | `alwaysApply: false` | `trigger: manual` | in main file | `alwaysApply: false` | `inclusion: manual` |

### MCP key mapping per tool

| Tool | JSON key | Notes |
|---|---|---|
| Claude, Windsurf, Kiro, Roo, Amazon Q | `mcpServers` | Standard format |
| Copilot | `servers` | VS Code format |
| OpenCode | `mcp` | `type: local/remote` |
| Zed | `context_servers` | No type field |
| Gemini | `mcpServers` | No type field, uses `httpUrl` |
| Amp | `amp.mcpServers` | Dotted key |

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
```

## Skills

This project uses Claude Code skills in `.claude/skills/`:

- **verify-providers** — Audit all 13 provider adapters against latest official documentation, fix discrepancies, and verify links

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
