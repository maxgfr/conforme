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
  config.rs         — NormalizedConfig, NormalizedRule, NormalizedSkill, NormalizedAgent, NormalizedMcpServer, ActivationMode;
                       yaml_globs reads a glob field as YAML list or comma string (Claude `paths`, Cursor `globs`)
  markdown.rs       — AGENTS.md parser (sections → rules via ## Rule: headings)
  frontmatter.rs    — gray_matter wrapper for YAML frontmatter parsing/serialization
  lib.rs            — Library crate re-exports (adapters, config, etc.)
  sync.rs           — Core sync engine: init, sync, check, status, remove, diff, migrate commands
                       (target_config, renamed-id check, AGENTS.md output unless the source reads it;
                       a source that reads back empty warns "nothing to sync" and writes/cleans nothing;
                       migrate keeps what staying tools generate or manage — see Orphan cleanup below)
  detect.rs         — Tool detection (which tools present in project)
  hash.rs           — SHA-256 content hashing for change detection
  json_settings.rs  — JSONC-safe merge of conforme's keys into user-owned JSON settings files
                       (in-place edit keeps comments; unparsable file is refused, never overwritten;
                       merge_server_entries keeps per-server keys conforme does not own;
                       is_expressible_server: an entry with neither `command` nor `url`/`httpUrl`/`serverUrl`,
                       or a Claude `type: "sdk"` one, is skipped on read and kept untouched on write
                       (Zed extension servers, Claude SDK servers, OpenCode `{ "enabled": false }` toggles);
                       server_settings_file leaves the file untouched when the source has no server)
  hook.rs           — Git pre-commit hook install/uninstall (like Husky)
  gitignore.rs      — `conforme gitignore install/uninstall`: ignores the files each non-source
                       adapter owns (root files anchored, rules/agents dirs by suffix);
                       merged settings files and the source's own locations stay tracked
  project_config.rs  — .conformerc.toml parser (source, only, exclude, clean options)
  validate.rs        — Config validation (duplicate names, empty content, invalid globs,
                       names that sanitize to nothing or collide, over-long skill descriptions)
  watch.rs           — File watcher for auto-sync (notify + debounce)
  help_ai.rs        — Detailed help about all supported tools and formats
  mcp.rs            — MCP config generation/parsing per tool:
                       - JSON entry shapes are data (`ServerShape`: type values, URL key, env on remote,
                         env-var syntax) fed to one builder; every JSON target is merged via json_settings
                       - EnvRefStyle: `${VAR}` normalized, written as `${env:VAR}` (Cursor, VS Code, Zoo,
                         Devin) or `{env:VAR}` (OpenCode), read back by canonicalize_env_refs
                       - Codex: project `.codex/config.toml`, atomically merged with comment/settings preservation; strict safe parser — merge_codex_mcp_toml / parse_codex_mcp_toml.
                         Codex expands no `${VAR}`: stdio `NAME=${NAME}` → `env_vars`, `Authorization: Bearer ${VAR}` →
                         `bearer_token_env_var`, an exact `${VAR}` header → `env_http_headers`, all read back to `${VAR}`;
                         `env` on HTTP is dropped; tuning keys (timeouts, `enabled_tools`, `required`, …) are read past and kept
                       - Claude, Kiro: mcpServers with type stdio/http; Cursor: no type on remote entries
                       - Zoo Code: mcpServers, HTTP uses type "streamable-http" (not "http"), no env on remote
                       - Claude .mcp.json parsing accepts http/https, sse, streamable-http, and ws transports (all mapped to the HTTP variant);
                         `type: "sdk"` entries are skipped
                       - Copilot: "servers" key (env on stdio, headers on HTTP)
                       - Devin: mcpServers in .devin/mcp_config.json, no type, remote `transport: http`
                       - OpenCode: "mcp" key merged into opencode.json, type local/remote, command as array, `environment` key (local only);
                         merge_opencode_agents keeps the user's own `agent` entries
                       - Zed: "context_servers" key
                       - Gemini: mcpServers, no type field, httpUrl for HTTP
                       - Amp: "amp.mcpServers" key merged into .amp/settings.json(c) — build_amp_mcp_object
                       - parse_mcp_json reads back mcpServers / servers / context_servers / amp.mcpServers
                       - OpenCode needs its own inverse (parse_opencode_mcp_object / parse_opencode_agent_object)
  skills.rs         — Skills (SKILL.md) and agents generation per tool; shared read helpers
                       preserve manual_invocation (and Codex policy sidecars, created only in
                       .agents/skills) across sync (read_skills_from_dir, read_skills_recursive,
                       read_agents_from_dir, parse_frontmatter_tool_list) used by adapters to
                       round-trip skills/agents on read(); TOOL_EQUIVALENTS translates agent tools
                       for Claude/Gemini/Kiro (Gemini `mcp_*` ⇄ Kiro `@mcp`, Kiro todo tool `todo`);
                       claude_model/gemini_model/opencode_model keep only values the tool accepts
                       (gemini_model also keeps `gemma-*`); kiro_model/copilot_model/cursor_model drop
                       another host's alias (sonnet, opus, haiku, fable, pro, flash, flash-lite) and any
                       provider/model id, Kiro and Copilot also `inherit`
  adapters/
    mod.rs          — AiToolAdapter trait + registry + shared write_if_changed +
                       collect_rule_files (recursive rules-dir scan, sorted by base name) +
                       ManagedDir / clean_orphans
    claude.rs       — Claude Code: CLAUDE.md (or .claude/CLAUDE.md when only that exists)
                       + .claude/rules/**/*.md, read recursively (paths: frontmatter); with no CLAUDE.md,
                       .claude/CLAUDE.md nor CLAUDE.local.md, reads AGENTS.md / .claude/AGENTS.md as Claude Code does
    cursor.rs       — Cursor: .cursor/rules/**/*.mdc, read recursively (alwaysApply/globs/description); subagents at .cursor/agents/*.md
    devin.rs        — Devin Desktop (formerly Windsurf): writes .devin/{rules,skills,mcp_config.json}; reads
                       .devin/ and the legacy .windsurf/ (both loaded upstream) and cleans conforme's legacy copies;
                       global_rules.md (.devin/, else .windsurf/) is read into the instructions
    copilot.rs      — GitHub Copilot: .github/copilot-instructions.md (applyTo); skills at .github/skills/<name>/SKILL.md; MCP merged into .vscode/mcp.json
    codex.rs        — OpenAI Codex CLI: reads AGENTS.md natively
    opencode.rs     — OpenCode: reads AGENTS.md natively; agents read from .opencode/agents/, .opencode/agent/
                       and the opencode.json `agent` key together (markdown wins; built-in overrides skipped)
    zoocode.rs      — Zoo Code (community fork of Roo Code): .roo/rules/**/*.md, read recursively (plain Markdown);
                       `NN-` prefix stripped and `Intended scope` comment read back as globs; .roorules read
                       when .roo/rules/ is missing or empty (.clinerules does not trigger detection)
    gemini.rs       — Gemini CLI: GEMINI.md; on read, the files `context.fileName` names (AGENTS.md among
                       them is read with the AGENTS.md convention); remote agents and `_` drafts left alone
    zed.rs          — Zed AI: .rules file
    kiro.rs         — Kiro (AWS): .kiro/steering/*.md (inclusion/fileMatchPattern)
    amp.rs          — Amp: reads AGENTS.md natively; MCP in .amp/settings.json (or .jsonc)
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
<!-- env: ROOT=${HOME} -->

## MCP: github
<!-- url: https://api.githubcopilot.com/mcp/ -->
<!-- headers: Authorization=Bearer ${GITHUB_TOKEN} -->
```

### Adapter categories

**Per-rule adapters** (have frontmatter or per-file rules):
- Claude, Cursor, Devin, Copilot, Kiro, Zoo Code

**Single-file adapters** (merge all content into one file):
- Codex, OpenCode, Gemini, Zed, Amp, DeepSeek Harness

### Adapter mapping

| Activation | Claude | Cursor | Devin | Copilot | Kiro |
|---|---|---|---|---|---|
| Always | in CLAUDE.md | `alwaysApply: true` | `trigger: always_on` | in main file | `inclusion: always` |
| GlobMatch | `paths: [globs]` | `globs:` | `trigger: glob` | `applyTo:` | `inclusion: fileMatch` |
| AgentDecision | no frontmatter | `description:` | `trigger: model_decision` | in main file | `inclusion: auto` |
| Manual | no frontmatter | `alwaysApply: false` | `trigger: manual` | in main file | `inclusion: manual` |

### MCP key mapping per tool

| Tool | JSON key | Notes |
|---|---|---|
| Claude, Kiro | `mcpServers` (`.mcp.json`, `.kiro/settings/mcp.json`) | `type: stdio/http`; merged (keeps `oauth`, Kiro `autoApprove`/`disabledTools`, Claude `type: "sdk"` entries) |
| Cursor | `mcpServers` (`.cursor/mcp.json`) | `type: stdio` locally, no `type` on remote entries; `${env:VAR}`; merged |
| Zoo Code | `mcpServers` (inside `.roo/mcp.json`) | HTTP uses `type: streamable-http` (not `http`), no `env` on remote servers; merged (keeps Zoo's `alwaysAllow`/`disabledTools`) |
| Copilot | `servers` (inside `.vscode/mcp.json`) | VS Code format; `env` on stdio, `headers` on HTTP; merged (keeps `inputs`/`sandbox`) |
| Devin | `mcpServers` (inside `.devin/mcp_config.json`) | No type field; remote `url` + `transport: http`; `${env:VAR}`; merged |
| OpenCode | `mcp` (inside `opencode.json`) | `type: local/remote`; `command` is a single array; env key is `environment` (local only); `{env:VAR}`; merged (preserves user keys and `{ "enabled": false }` toggles) |
| Zed | `context_servers` (inside `.zed/settings.json`) | No type field; merged into existing settings (preserves theme/keybindings/etc. and extension servers configured only via `settings`) |
| Gemini | `mcpServers` (inside `.gemini/settings.json`) | No type field, uses `httpUrl` for HTTP; merged into existing settings |
| Amp | `amp.mcpServers` (inside `.amp/settings.json` or `.jsonc`) | Dotted key; no type field; merged into existing settings |
| DeepSeek Harness | _(none)_ | MCP is a user-level `cordis.patch.yml` plugin entry under `$DSH_HOME`; nothing project-scoped is generated |
| Codex | `[mcp_servers.<name>]` (inside `.codex/config.toml`) | TOML; no `${VAR}` expansion, so references go through `env_vars` (stdio), `bearer_token_env_var` and `env_http_headers`; atomic merge preserves unrelated settings, comments, target-only servers, and Codex-specific options; shared file is never deleted wholesale |

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

`ManagedDir::files_except` keeps files a tool does not load as agents (a
Claude README or an agent without `description`, a Gemini `_draft.md` or remote agent).
`ManagedDir::legacy_files` / `legacy_skills` (Devin's `.windsurf/`) remove only
conforme's old copies: a legacy file or skill whose name is generated in the
current location (a skill folder that bundles other files is kept).

Directories two tools share (`.agents/skills/` for Codex, Zed and Amp):
`sync::target_config` leaves the source's skills root to the source, and
`remove` never deletes a file the source or another kept tool also generates.
`migrate` generates the output through `target_config` too, never deletes a
file another detected tool generates nor anything in a directory a staying tool
(the output or another detected tool) manages, and in the source's own
directories deletes only files with conforme's suffix not protected by `keep`
(Kiro `.json` agents, Zoo `.txt` rules survive) and, in a skills directory, only
skill sub-folders, never top-level files.
`reads_agents_md(project_root)` is true for Codex, OpenCode, Amp and DeepSeek;
for Claude Code when the project has no `CLAUDE.md`, `.claude/CLAUDE.md` nor
`CLAUDE.local.md` but has `AGENTS.md` / `.claude/AGENTS.md`; for Gemini CLI when
`.gemini/settings.json` `context.fileName` names `AGENTS.md`. Such a tool reads
it with the AGENTS.md convention, and when it is the source `AGENTS.md` is
never regenerated nor gitignored. Renamed tool ids
(`windsurf` → `devin`) are an error wherever an id is accepted.

Every JSON file conforme merges into (`.mcp.json`, `.cursor/mcp.json`,
`.kiro/settings/mcp.json`, `.devin/mcp_config.json`, `opencode.json`,
`.zed/settings.json`, `.gemini/settings.json`, `.amp/settings.json(c)`,
`.vscode/mcp.json`, `.roo/mcp.json`) goes through `json_settings`, and together
with `.codex/config.toml` is declared by `is_shared_file()` so `remove`/`migrate`
never delete it wholesale and `gitignore install` never ignores it.

### Sync algorithm

1. Parse AGENTS.md (or the source tool) → NormalizedConfig; an empty config stops with "nothing to sync"
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

- **verify-providers** — Audit all 12 provider adapters: upstream product alive, renamed, forked or retired; official docs vs adapter code; required fields and value vocabularies (tool names, models, names, env-var syntax); safety invariants (orphan suffixes, JSONC-safe shared settings, gitignore, round-trip, blank output); link check; then fix with regression tests

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
- MCP JSON keys per tool: Claude/Kiro/Zoo Code/Gemini/Cursor/Devin = `mcpServers`, Copilot = `servers`, OpenCode = `mcp` (inside `opencode.json`), Zed = `context_servers`, Amp = `amp.mcpServers`
- A JSON MCP entry shape is a `ServerShape` in `src/mcp.rs` (type values, URL key, `env` on remote, env-var syntax); add or change a tool there rather than writing another builder
- OpenCode MCP specifics: `command` is a single array `[cmd, ...args]`, env key is `environment` (not `env`), servers live inside `opencode.json` at project root (conforme merges — never clobber user-authored keys)
- Devin (formerly Windsurf): write `.devin/` only; `.windsurf/` is read as a legacy location (Devin loads both) and conforme's old copies there are cleaned. Project MCP is `.devin/mcp_config.json`; never generate `.windsurf/mcp.json`
- Never add a user-authored directory (e.g. `.github/prompts/`) to `managed_directories()`: orphan cleanup deletes every file there that carries the directory's suffix and that conforme did not generate
- Give each `ManagedDir` the exact suffix conforme writes there (`ManagedDir::files(dir, ".agent.md")`), and use `ManagedDir::subdirs` for skills directories, so files the tool accepts but conforme never writes (Kiro `.json` agents, dsh flat skills) survive a sync
- An adapter must generate no files for an empty config and never a blank file (guarded by `test_no_adapter_writes_blank_files`)
- When an upstream tool is retired (as Amazon Q was), delete its adapter outright rather than keeping it behind a deprecation flag; the removal checklist is the mirror of the "adding a new adapter" one above
- Every MCP file is merged, never owned: any adapter that merges into a user-owned settings file (`.mcp.json`, `.cursor/mcp.json`, `.kiro/settings/mcp.json`, `.devin/mcp_config.json`, `opencode.json`, `.zed/settings.json`, `.gemini/settings.json`, `.amp/settings.json`, `.vscode/mcp.json`, `.roo/mcp.json`, `.codex/config.toml`) MUST implement `is_shared_file()` so `remove` and `migrate` never delete it wholesale, and its gitignore patterns must not match it
- Merge JSON settings through `json_settings::server_settings_file` (or `load`/`merge_server_entries`/`render_with_removals`), never `serde_json::from_str(..).unwrap_or_default()`: those files are JSONC, a parse failure must refuse the write instead of replacing the user's file, and a source with no server leaves the file untouched
- Amp MCP specifics: dotted `amp.mcpServers` key, no `type` field, merged into `.amp/settings.json` (never clobber user settings)
- Cursor subagents: `.md` extension (not `.mdc`); no `tools` frontmatter field — tool access is inherited from the parent agent
- Copilot skills: `.github/skills/<name>/SKILL.md` (NOT `.github/prompts/*.prompt.md` — prompt files are a separate VS Code feature)
- Any adapter whose `generate()` writes skills, agents, or MCP MUST read them back in `read()`, or `--from <tool>` silently drops them
- Skills and agents always carry a `description` (`description_or_name`): Codex, Copilot, Gemini, OpenCode and Zoo Code skip one without it
- Tools with their own tool vocabulary (Claude Code, Gemini CLI, Kiro) get translated `tools` lists (`TOOL_EQUIVALENTS`, MCP names respelled), never names copied verbatim from another host; a `model` another tool cannot use is left out (`claude_model`, `gemini_model`, `opencode_model`, `kiro_model`, `copilot_model`, `cursor_model`)
- Skill and agent names go through `sanitize_name` (kebab-case ASCII, at most 64 characters), rule file names through `rule_file_name`, and MCP strings through `EnvRefStyle` so `${VAR}` becomes each tool's own reference syntax
- A tool that reads `AGENTS.md` itself in a project returns `true` from `reads_agents_md(project_root)` (always for Codex, OpenCode, Amp, DeepSeek; Claude Code without `CLAUDE.md`; Gemini CLI when `context.fileName` names it) and reads it with `markdown::read_native_agents_md`; renaming a tool id adds it to `sync::RENAMED_IDS`
- A server entry conforme cannot express (no `command`, no URL, or Claude `type: "sdk"`) is skipped on read and kept on write (`json_settings::is_expressible_server`)

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
`README.md` ("Supported tools (N)" and "to N-1 others"), `src/cli.rs`
(`long_about`), and `CLAUDE.md` (project overview and Skills section).

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

Verdict per adapter, with the URL that proves it:

- **alive**;
- **renamed**: the product ships under a new name and still reads the paths
  conforme writes (Windsurf became Devin Desktop, and the `windsurf` id
  became `devin`). Update the docs and any paths the new name prefers;
  keeping or renaming the tool id is the user's call in step 6;
- **forked**: the original is retired but a maintained fork reads the same
  files (Roo Code, continued by Zoo Code). Rename the adapter to the fork in
  step 7 rather than deleting working output;
- **absorbed by `<successor adapter>`** or **retired**: deleted outright in
  step 7; conforme carries no deprecation flags.

Done when: every adapter has a verdict with its proof URL.

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
- **User-owned locations**: orphan cleanup deletes every top-level file of a
  `managed_directories()` entry that carries the entry's suffix and that the
  current config did not generate. A directory the user authors in with the
  same file kind (`.github/prompts/`, the top-level `.opencode/`) must not be
  listed; a shared skills root (`.agents/skills`) is only safe as
  `ManagedDir::subdirs`.
- **Merged settings**: every MCP or settings file `generate()` writes is
  merged, not owned (`.mcp.json`, `.cursor/mcp.json`,
  `.kiro/settings/mcp.json`, `.devin/mcp_config.json`, `opencode.json`,
  `.zed/settings.json`, `.gemini/settings.json`, `.amp/settings.json`,
  `.vscode/mcp.json`, `.roo/mcp.json`, `.codex/config.toml`). Each is covered
  by `is_shared_file()`, so `remove` and `migrate` preserve it, is matched by
  no `gitignore` pattern, and, when JSON, goes through `json_settings` so a
  JSONC comment, an unparsable file or a per-server key the tool writes
  itself (Kiro `autoApprove`, OpenCode `agent.build`) never wipes the user's
  settings. Check which keys the tool writes into the file on its own, and
  that a source with no server leaves the file untouched.
- **Shared locations**: a directory two adapters write (`.agents/skills/` for
  Codex, Zed and Amp) must survive `migrate` between them and stay tracked by
  `gitignore install` when one of them is the source.
- **Orphan suffixes**: every `ManagedDir` names the suffix conforme writes
  there; list the file kinds the tool accepts in that directory and confirm
  none that conforme does not write shares it.
- **Required fields and vocabularies**: for each skill and agent format,
  which keys the tool requires (a missing `description` makes several tools
  skip the entry) and which values it accepts (Claude Code, Gemini and Kiro
  tool names and MCP tool spellings, model ids, OpenCode `provider/model`,
  name character sets and length). In MCP configs, check the
  environment-variable syntax (`${VAR}`, `${env:VAR}`, `{env:VAR}`) and
  which keys remote servers accept (`env` is often stdio-only). Anything
  copied verbatim from another host must be valid in this one.
- **Round-trip**: every feature `generate()` writes, `read()` parses back,
  otherwise `--from <tool>` silently drops it. Also check the layouts the tool
  reads that conforme does not write (nested directories, flat files,
  alternate spellings such as a comma-separated `paths` string).
- **Blank output**: an empty config yields no file, and a full config yields
  no blank file (`test_no_adapter_writes_blank_files` guards it).

Done when: the audit table has a finding for each fact-sheet line and each
check above, for every adapter.

## 5. Verify every documentation link

```bash
grep -oh 'https://[^ )>,`"]*' docs/providers/*.md README.md CLAUDE.md | sed 's/[.,]$//' | sort -u > urls.txt
while read -r u; do printf '%s %s\n' "$(curl -sS -o /dev/null -w '%{http_code}' -A 'Mozilla/5.0' --max-time 25 -L "$u")" "$u"; done < urls.txt | sort | grep -v '^200'
```

GitHub answers `429` under a burst; retry those alone with a pause. Any other
non-200 is a broken link: find the replacement on the vendor domain and fix it
in step 7. A `200` can still be a stub ("Page moved" with a meta refresh, as
`deepseek.com/harness/en/` became): the step 3 fetch catches those. Done when:
the loop prints nothing and no fetched page is a stub.

## 6. Classify every finding

Before fixing, sort the audit table into three lists so the user can see the
decisions, and confirm the third one:

- **fix now**: drift, broken links, failed safety checks;
- **leave**: deliberate deviations already documented as such;
- **delete or rename the adapter**: absorbed, retired, forked or renamed
  upstream (step 2). Deleting or renaming a tool id is a breaking change and
  ships as a major release: the commit needs `!` and a `BREAKING CHANGE:`
  footer with the migration, and say so before merging.

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
   tests, `.gitignore` templates, and the dogfooded output directory; a
   renamed one changes its module, struct, id, `docs/providers/` file and
   every test fixture that names it.

Then regenerate the dogfooded configs and run the full gate:

```bash
cargo run --release -- sync && cargo run --release -- check
cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check
```

Done when: the gate is green, `git status` shows only intended files, and a
`grep -rni '<old tool id>'` returns only sentences that describe the removal
or rename (release notes live on GitHub Releases, not in the tree).

## 8. Report

One table, one row per adapter:

| Tool | Upstream status | Findings | Fixed | Left as deliberate |

followed by the three lists from step 6 and the PR link. State plainly what
was not verified (a page that could not be read, a behaviour only a manual
run in the IDE would confirm) and the known gaps left for a later change.
After the merge, check that semantic-release published the expected version
and that the release binaries are attached.
