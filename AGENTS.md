# Project Instructions

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

## Rule: adapter-consistency
<!-- activation: glob src/adapters/**,src/mcp.rs,src/skills.rs,docs/providers/** -->

- Every adapter change MUST be reflected in its `docs/providers/<tool>.md`
- Every MCP format change MUST update the corresponding `generate_*_mcp_json` function AND its unit test
- When adding a new adapter, update ALL of: README.md tables, src/help_ai.rs, src/cli.rs tool count, CLAUDE.md architecture section
- Provider docs must list all official documentation URLs for the tool
- Test round-trips: `read()` output fed into `generate()` should produce identical files
- MCP JSON keys per tool: Claude/Windsurf/Kiro/RooCode/AmazonQ/Gemini = `mcpServers`, Copilot = `servers`, OpenCode = `mcp`, Zed = `context_servers`, Amp = `amp.mcpServers`

## Rule: testing
<!-- activation: glob tests/** -->

- Integration tests use `assert_cmd` + `tempfile` crates
- Each adapter must have round-trip tests in `tests/roundtrip.rs`
- MCP format tests belong in `src/mcp.rs` unit tests
- Error case tests go in `tests/error_cases.rs`
- Always assert on file paths AND content (not just existence)
- When changing an output path (e.g. MCP location), update ALL tests that reference it

## Rule: rust-conventions
<!-- activation: glob **/*.rs -->

- Run `cargo clippy -- -D warnings` before considering any change complete
- Run `cargo test` after modifying adapter logic, MCP generation, or skills generation
- Use `anyhow::Result` for fallible functions, `anyhow::Context` for error messages
- Use `BTreeMap` (not `HashMap`) for deterministic output ordering in generated files
- Parse frontmatter fields defensively: use `.and_then(|v| v.as_str())` chains, never `.unwrap()` on user input
- When parsing tool-separated lists (e.g. `allowed-tools`, `tools`), handle both space-separated AND comma-separated formats: `split_whitespace().flat_map(|t| t.split(','))`
- Keep adapter `read()` and `generate()` symmetric: if `generate()` writes a field, `read()` must parse it back correctly (round-trip guarantee)

## Skill: verify-providers
<!-- description: Verify all 13 provider adapters against their official documentation and fix any discrepancies -->
<!-- tools: Read, Grep, Glob, Bash, WebFetch, WebSearch, Edit, Write -->

Audit every conforme provider adapter against the latest upstream documentation.
For each of the 13 tools, follow these steps:

## 1. Read the adapter code and provider docs

For each tool (claude, cursor, copilot, windsurf, continuedev, kiro, amazonq, codex, opencode, gemini, zed, amp, roocode):

- Read `src/adapters/<tool>.rs`
- Read `docs/providers/<tool>.md`
- Note the official doc URLs listed in the provider doc

## 2. Fetch the latest official documentation

Use WebFetch on each official doc URL from `docs/providers/<tool>.md`.
Extract the current:
- Config file paths and names
- Frontmatter fields and their types
- Activation mode mappings
- Skills/agents/MCP format and file locations
- Any new features or breaking changes

## 3. Compare adapter code vs official docs

Check for discrepancies in:
- **File paths**: Are we writing to the correct locations?
- **Frontmatter fields**: Are we reading/writing all required fields? Using correct separators?
- **MCP format**: Correct JSON key (`mcpServers`, `servers`, `context_servers`, `amp.mcpServers`, `mcp`)? Correct transport types?
- **Skills format**: Correct frontmatter fields per tool?
- **Agents format**: Correct file extension and frontmatter?
- **Activation modes**: Correct mapping values?

## 4. Fix discrepancies

For each issue found:
1. Fix the adapter code in `src/adapters/` or `src/mcp.rs` or `src/skills.rs`
2. Update the provider doc in `docs/providers/`
3. Update `src/help_ai.rs` if the help text is affected
4. Update tests in `tests/` to match new behavior
5. Run `cargo test` to verify

## 5. Verify links

WebFetch each documentation URL to confirm it still resolves (not 404).
If a link is broken, search for the new URL and update `docs/providers/<tool>.md`.

## 6. Final checks

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

Report a summary table:
| Tool | Status | Issues found | Fixed |

## MCP: context7
<!-- url: https://mcp.context7.com/mcp -->

