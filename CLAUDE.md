# CLAUDE.md

## Project overview

conforme is a Rust CLI that synchronizes AI coding agent configurations across tools. It treats AGENTS.md as the source of truth and generates/updates tool-specific config files for Claude Code, Cursor, Windsurf, GitHub Copilot, and more.

## Build & test

```bash
cargo build --release
cargo test                     # 32 tests (13 unit + 19 integration)
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
  adapters/
    mod.rs          — AiToolAdapter trait + registry
    claude.rs       — CLAUDE.md + .claude/rules/*.md
    cursor.rs       — .cursor/rules/*.mdc
    windsurf.rs     — .windsurf/rules/*.md
    copilot.rs      — .github/copilot-instructions.md + .github/instructions/
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

| Activation | Claude | Cursor | Windsurf | Copilot |
|---|---|---|---|---|
| Always | in CLAUDE.md | `alwaysApply: true` | `trigger: always_on` | in copilot-instructions.md |
| GlobMatch | `paths: [globs]` | `globs:`, `alwaysApply: false` | `trigger: glob` | `applyTo:` |
| AgentDecision | no frontmatter | `description:`, `alwaysApply: false` | `trigger: model_decision` | no applyTo |
| Manual | no frontmatter | `alwaysApply: false` | `trigger: manual` | no applyTo |

### Sync algorithm

1. Parse AGENTS.md → NormalizedConfig
2. For each detected adapter: generate expected files, write if changed
3. Change detection uses SHA-256 content hashing

## CLI commands

```
conforme init [--force]                    # Create AGENTS.md + sync to tools
conforme sync [--dry-run] [--only tools]   # AGENTS.md → all tool configs
conforme check                             # Exit 0 if in sync, 1 if not
conforme status                            # Show detected tools + sync state
```

## Versioning

Managed by semantic-release. The `.version-hook.sh` script updates `Cargo.toml` during release.

## CI/CD

- **ci.yml**: build, unit tests, integration tests, clippy, fmt, audit, macOS smoke test
- **release.yml**: matrix build (linux-x64, macos-x64, macos-arm64), semantic-release, upload binaries
- Homebrew formula in `maxgfr/homebrew-tap`
