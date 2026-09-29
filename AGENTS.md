# Project Instructions

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
    windsurf.rs     — Windsurf: .devin/rules/*.md when .devin/ exists, else .windsurf/rules/*.md (trigger/description/globs)
    copilot.rs      — GitHub Copilot: .github/copilot-instructions.md (applyTo); skills at .github/skills/<name>/SKILL.md
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
| Zoo Code | `mcpServers` | Standard format; HTTP uses `type: streamable-http` (not `http`) |
| Copilot | `servers` | VS Code format; supports `env` + `headers` |
| Windsurf | _(none)_ | Cascade only reads the user-global `~/.codeium/windsurf/mcp_config.json`; nothing project-scoped is generated |
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

## Rule: adapter-consistency
<!-- activation: glob src/adapters/**,src/mcp.rs,src/skills.rs,docs/providers/** -->

- Every adapter change MUST be reflected in its `docs/providers/<tool>.md`
- Every MCP format change MUST update the corresponding `generate_*_mcp_json` function AND its unit test
- When adding a new adapter, update ALL of: README.md tables, src/help_ai.rs, src/cli.rs tool count, CLAUDE.md architecture section
- Provider docs must list all official documentation URLs for the tool
- Test round-trips: `read()` output fed into `generate()` should produce identical files
- MCP JSON keys per tool: Claude/Kiro/Zoo Code/Gemini/Cursor = `mcpServers`, Copilot = `servers`, OpenCode = `mcp` (inside `opencode.json`), Zed = `context_servers`, Amp = `amp.mcpServers`
- OpenCode MCP specifics: `command` is a single array `[cmd, ...args]`, env key is `environment` (not `env`), servers live inside `opencode.json` at project root (conforme merges — never clobber user-authored keys)
- Windsurf has NO project-level MCP file (Cascade only reads `~/.codeium/windsurf/mcp_config.json`); never generate `.windsurf/mcp.json`
- Never add a user-authored directory (e.g. `.github/prompts/`) to `managed_directories()`: orphan cleanup deletes every file there that conforme did not generate
- An adapter must generate no files for an empty config and never a blank file (guarded by `test_no_adapter_writes_blank_files`)
- When an upstream tool is retired (as Amazon Q was), delete its adapter outright rather than keeping it behind a deprecation flag; the removal checklist is the mirror of the "adding a new adapter" one above
- Any adapter that merges into a user-owned settings file (`opencode.json`, `.zed/settings.json`, `.gemini/settings.json`, `.amp/settings.json`, `.codex/config.toml`) MUST implement `is_shared_file()` so `remove` and `migrate` never delete it wholesale
- Amp MCP specifics: dotted `amp.mcpServers` key, no `type` field, merged into `.amp/settings.json` (never clobber user settings)
- Cursor subagents: `.md` extension (not `.mdc`); no `tools` frontmatter field — tool access is inherited from the parent agent
- Copilot skills: `.github/skills/<name>/SKILL.md` (NOT `.github/prompts/*.prompt.md` — prompt files are a separate VS Code feature)
- Any adapter whose `generate()` writes skills, agents, or MCP MUST read them back in `read()`, or `--from <tool>` silently drops them

## Rule: rust-conventions
<!-- activation: glob **/*.rs -->

- Run `cargo clippy -- -D warnings` before considering any change complete
- Run `cargo test` after modifying adapter logic, MCP generation, or skills generation
- Use `anyhow::Result` for fallible functions, `anyhow::Context` for error messages
- Use `BTreeMap` (not `HashMap`) for deterministic output ordering in generated files
- Parse frontmatter fields defensively: use `.and_then(|v| v.as_str())` chains, never `.unwrap()` on user input
- When parsing tool-separated lists (e.g. `allowed-tools`, `tools`), handle both space-separated AND comma-separated formats: `split_whitespace().flat_map(|t| t.split(','))`
- Keep adapter `read()` and `generate()` symmetric: if `generate()` writes a field, `read()` must parse it back correctly (round-trip guarantee)

## Rule: testing
<!-- activation: glob tests/** -->

- Integration tests use `assert_cmd` + `tempfile` crates
- Each adapter must have round-trip tests in `tests/roundtrip.rs`
- MCP format tests belong in `src/mcp.rs` unit tests
- Error case tests go in `tests/error_cases.rs`
- Always assert on file paths AND content (not just existence)
- When changing an output path (e.g. MCP location), update ALL tests that reference it

## Skill: verify-providers
<!-- description: Use when auditing conforme's provider adapters against upstream documentation, after a vendor announcement (rename, acquisition, retirement, new config format), before a release, or when a user reports that a tool ignores or rejects a generated file -->
<!-- invocation: manual -->
<!-- tools: Read, Grep, Glob, Bash, WebFetch, WebSearch, Edit, Write -->

Audit every provider adapter against the upstream tool as it exists today,
then fix what the audit finds. The audit is exhaustive when every registered
adapter has a verdict on each check below, with evidence, and every fix is
merged with a regression test.

Work in a worktree. Run the full gate before touching anything so a failure
later is attributable to the audit:

```bash
cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check && cargo run --release -- check
```

## 1. Inventory the adapters

The registry is the source of truth, never a list written in this file:

```bash
grep -n 'Box::new' src/adapters/mod.rs
```

Done when: one audit row exists per registered adapter, and the count matches
`README.md` ("Supported tools (N)"), `src/cli.rs` (`long_about`),
`CLAUDE.md` and this skill's description.

## 2. Check the upstream product is alive

Format drift is the visible failure; a retired product is the silent one. For
each adapter, from the URLs in `docs/providers/<tool>.md`, establish:

- the product still ships under this name (a docs page that is only a redirect
  or a one-line "X is now Y" stub means it does not);
- its source repository, when there is one, is not archived or marked
  unmaintained;
- the config paths conforme writes are the ones the *current* product reads,
  and which paths win when the successor reads both (for example `.kiro/`
  over `.amazonq/`).

Verdict per adapter: **alive**, **absorbed by `<successor adapter>`**, or
**retired**. Absorbed and retired adapters are deleted outright in step 7;
conforme carries no deprecation flags. Done when: every adapter has a verdict
with the URL that proves it.

## 3. Fetch the current documentation

Open every URL listed in `docs/providers/<tool>.md` with the host's web reader
(`WebFetch` on Claude Code). When a page fails or answers a different
question, search the vendor's own domain for the replacement and record the
new URL for step 7. Also open the tool's changelog or release notes and scan
for config changes since the last audit (the date of the previous
`verify-providers` commit in `git log`).

Extract, per tool, a fact sheet:

- rules: directory, extension, recursive discovery or not, frontmatter fields
  and their types, activation values;
- skills: directory, `SKILL.md` frontmatter (required and optional keys),
  manual-invocation mechanism;
- agents: directory, extension, frontmatter keys and accepted values;
- MCP: file path, top-level key, transport `type` values, remote URL key
  (`url`, `serverUrl`, `httpUrl`), env and headers keys, whether the file is
  project-scoped or user-global;
- instruction file: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.rules`, and the
  fallback order.

Done when: every bullet above has a value or an explicit "not supported" for
every alive adapter.

## 4. Compare the adapter to the fact sheet

Read `src/adapters/<tool>.rs`, plus the tool's generators in `src/skills.rs`
and `src/mcp.rs`. For each fact-sheet line, one of three findings:

- **match**: nothing to do;
- **drift**: paths, keys, values or types differ, fix in step 7;
- **deliberate deviation**: conforme emits something the tool tolerates but
  does not document (for example a `type: "http"` the tool ignores). Keep it
  only if `docs/providers/<tool>.md` already records it as deliberate and the
  tool provably accepts it; otherwise it is drift.

Add these checks, which the fact sheet alone does not cover:

- **Consumed output**: every file `generate()` writes is read by the tool at
  that path. A best-effort file nobody consumes is drift, and the fix is to
  stop writing it, drop the capability, and let `sync` warn that the feature
  is skipped.
- **User-owned locations**: every entry in `managed_directories()` is a
  directory only conforme writes to. Orphan cleanup deletes every file there
  that the current config did not generate, so a directory the user also
  authors in (`.github/prompts/`, the top-level `.opencode/`, `.agents/skills`
  shared with other tools) must not be listed.
- **Merged settings**: every file `generate()` merges into rather than owns
  (`opencode.json`, `.zed/settings.json`, `.gemini/settings.json`,
  `.amp/settings.json`, `.codex/config.toml`) is covered by
  `is_shared_file()`, so `remove` and `migrate` preserve it.
- **Round-trip**: every feature `generate()` writes, `read()` parses back,
  otherwise `--from <tool>` silently drops it.
- **Blank output**: an empty config yields no file, and a full config yields
  no blank file (`test_no_adapter_writes_blank_files` guards it).

Done when: the audit table has a finding for each fact-sheet line and each
check above, for every adapter.

## 5. Verify every documentation link

```bash
grep -oh 'https://[^ )>,]*' docs/providers/*.md | sed 's/[.,]$//' | sort -u > urls.txt
while read -r u; do printf '%s %s\n' "$(curl -sS -o /dev/null -w '%{http_code}' -A 'Mozilla/5.0' --max-time 25 -L "$u")" "$u"; done < urls.txt | sort | grep -v '^200'
```

GitHub answers `429` under a burst; retry those alone with a pause. Any other
non-200 is a broken link: find the replacement on the vendor domain and fix it
in step 7. Done when: the loop prints nothing.

## 6. Classify every finding

Before fixing, sort the audit table into three lists so the user can see the
decisions, and confirm the third one:

- **fix now**: drift, broken links, failed safety checks;
- **leave**: deliberate deviations already documented as such;
- **delete the adapter**: absorbed or retired upstream (step 2). Deleting a
  tool id is a breaking change and ships as a major release; say so before
  merging.

## 7. Fix, with a regression test per finding

For each "fix now" and "delete" item, update every affected location. The
checklist in `.claude/rules/adapter-consistency.md` is authoritative; the
usual set is:

1. `src/adapters/<tool>.rs`, `src/skills.rs`, `src/mcp.rs`, `src/gitignore.rs`;
2. `docs/providers/<tool>.md` (facts, deviations, new URLs);
3. `README.md` tables and tool count, `CLAUDE.md` architecture and MCP
   tables, `src/help_ai.rs`, `src/cli.rs` tool count and tool list, this
   skill's description;
4. tests: `tests/roundtrip.rs` for round-trips, `tests/error_cases.rs` and
   `tests/integration.rs` for behaviour, `src/mcp.rs` unit tests for MCP
   shapes. A safety finding (user files deleted, blank file, settings file
   removed) gets an integration test that reproduces the loss and asserts
   the file survives;
5. a deleted adapter also leaves the registry, `all_adapters()` order-based
   tests, `.gitignore` templates, and the dogfooded output directory.

Then regenerate the dogfooded configs and run the full gate:

```bash
cargo run --release -- sync && cargo run --release -- check
cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check
```

Done when: the gate is green, `git status` shows only intended files, and a
`grep -rni '<old tool id>'` outside `CHANGELOG.md` returns only sentences
that describe the removal.

## 8. Report

One table, one row per adapter:

| Tool | Upstream status | Findings | Fixed | Left as deliberate |

followed by the three lists from step 6 and the PR link. State plainly what
was not verified (a page that could not be read, a behaviour only a manual
run in the IDE would confirm).
