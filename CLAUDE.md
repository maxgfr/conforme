# CLAUDE.md

## Project overview

conforme is a Rust CLI that synchronizes AI coding agent configurations across tools. It treats AGENTS.md as the source of truth and generates/updates tool-specific config files for 11 AI coding tools.

## Build & test

```bash
cargo build --release
cargo test                     # 40 tests (13 unit + 27 integration)
cargo clippy -- -D warnings    # lint — MUST pass before pushing
cargo fmt -- --check           # format check
```

## Architecture

```
src/
  main.rs           — Entry point, dispatches subcommands
  cli.rs            — CLI arg parsing (clap derive)
  config.rs         — NormalizedConfig, NormalizedRule, ActivationMode types
  markdown.rs       — AGENTS.md parser (sections → rules via ## Rule: headings)
  frontmatter.rs    — gray_matter wrapper for YAML frontmatter parsing/serialization
  sync.rs           — Core sync engine: init, sync, check, status commands
  detect.rs         — Tool detection (which tools present in project)
  hash.rs           — SHA-256 content hashing for change detection
  hook.rs           — Git pre-commit hook install/uninstall (like Husky)
  adapters/
    mod.rs          — AiToolAdapter trait + registry + shared write_if_changed
    claude.rs       — Claude Code: CLAUDE.md + .claude/rules/*.md
    cursor.rs       — Cursor: .cursor/rules/*.mdc
    windsurf.rs     — Windsurf: .windsurf/rules/*.md
    copilot.rs      — GitHub Copilot: .github/copilot-instructions.md + .github/instructions/
    codex.rs        — OpenAI Codex CLI: reads AGENTS.md natively
    opencode.rs     — OpenCode: reads AGENTS.md natively
    roocode.rs      — Roo Code / Cline: .roo/rules/*.md (plain Markdown, no frontmatter)
    gemini.rs       — Gemini CLI: GEMINI.md
    continuedev.rs  — Continue.dev: .continue/rules/*.md
    zed.rs          — Zed AI: .rules file
    amazonq.rs      — Amazon Q: .amazonq/rules/*.md
tests/
  integration.rs    — CLI integration tests (assert_cmd + tempfile)
```

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

### Adapter mapping

| Activation | Claude | Cursor | Windsurf | Copilot | Continue | Roo Code |
|---|---|---|---|---|---|---|
| Always | in CLAUDE.md | `alwaysApply: true` | `trigger: always_on` | in copilot-instructions.md | `alwaysApply: true` | plain .md |
| GlobMatch | `paths: [globs]` | `globs:`, `alwaysApply: false` | `trigger: glob` | `applyTo:` | `globs: [array]` | comment hint |
| AgentDecision | no frontmatter | `description:` | `trigger: model_decision` | in main file | `description:` | comment hint |
| Manual | no frontmatter | `alwaysApply: false` | `trigger: manual` | in main file | `alwaysApply: false` | plain .md |

**Single-file adapters** (Codex, OpenCode, Gemini, Zed): merge all content into one file (AGENTS.md / GEMINI.md / .rules).
**No-frontmatter adapters** (Roo Code, Amazon Q): use plain Markdown with numeric prefixes for ordering.

### Sync algorithm

1. Parse AGENTS.md → NormalizedConfig
2. For each detected adapter: generate expected files, write if changed
3. Change detection uses SHA-256 content hashing

### Pre-commit hook

`conforme hook install` installs a git pre-commit hook that runs `conforme check`.
`conforme hook uninstall` removes it. Works alongside existing hooks (appends/removes block).

## CLI commands

```
conforme init [--force]                    # Create AGENTS.md + sync to tools
conforme sync [--dry-run] [--only tools]   # AGENTS.md → all tool configs
conforme check                             # Exit 0 if in sync, 1 if not
conforme status                            # Show detected tools + sync state
conforme hook install                      # Install git pre-commit hook
conforme hook uninstall                    # Remove git pre-commit hook
```

## Versioning

Managed by semantic-release. The `.version-hook.sh` script updates `Cargo.toml` during release.

## CI/CD

- **ci.yml**: build, unit tests, integration tests, clippy, fmt, audit, macOS smoke test
- **release.yml**: matrix build (linux-x64, macos-x64, macos-arm64), semantic-release, upload binaries
- Homebrew formula in `maxgfr/homebrew-tap`
