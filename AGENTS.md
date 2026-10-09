# CLAUDE.md

## Project overview

conforme is a Rust CLI that synchronizes AI coding agent configurations across 13 tools, and switches a project from one tool to another (`migrate`). It reads config from a source tool (Claude Code, Cursor, etc.) or AGENTS.md, and propagates to all other tool-specific config files.

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
                       selected_targets (detection + only/exclude) is the one target set of sync, check,
                       diff and status; target_files/write_target leave out the source's source_files();
                       generated_agents_md is compared by check and diff too; check validates the config;
                       status shows excluded tools as "Excluded" and AGENTS.md as "Source's own file" /
                       "Generated output" / "Not managed";
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
                       adapter selected by only/exclude owns (root files anchored, rules/agents dirs
                       by suffix; Gemini's pattern is the context file it writes); merged settings
                       files, the source's own locations and its source_files() stay tracked
  project_config.rs  — .conformerc.toml parser (source, only, exclude, clean options)
  validate.rs        — Config validation (duplicate names, empty content, invalid globs,
                       names that sanitize to nothing or collide, over-long skill descriptions,
                       a rule whose file name becomes `general` while there are instructions — Cursor,
                       Devin and Kiro write the instructions there; warns on skills Claude Code reserves:
                       synced, anthropic-skills, claude-ai)
  watch.rs           — File watcher for auto-sync (notify + debounce): watches every location the
                       source reads (per-tool source_locations + source_files()), .conformerc.toml and
                       AGENTS.md; an existing directory recursively, a file or missing path through its
                       nearest existing parent (never a whole .claude/ or .github/ tree); re-plans after
                       each change so directories created later are watched; syncs only on events there
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
                       - Kilo: OpenCode shape with no variable reference (build_kilo_mcp_object: Kilo refuses
                         `{env:VAR}` in a project config; `NAME=${NAME}` dropped, a local server inherits the env)
                       - Vibe: `[[mcp_servers]]` in .vibe/config.toml — merge_vibe_mcp_toml / parse_vibe_mcp_toml
                         (`Authorization: Bearer ${VAR}` ⇄ static auth `api_key_env`)
                       - parse_mcp_json reads back mcpServers / servers / context_servers
                       - OpenCode needs its own inverse (parse_opencode_mcp_object / parse_opencode_agent_object)
  skills.rs         — Skills (SKILL.md plus the text files bundled beside it: read_bundled_files,
                       written next to SKILL.md in every tool, stale ones found by stale_bundled_files)
                       and agents generation per tool; shared read helpers
                       preserve manual_invocation (and Codex policy sidecars, created only in
                       .agents/skills) across sync (read_skills_from_dir, read_skills_recursive,
                       read_agents_from_dir, parse_frontmatter_tool_list) used by adapters to
                       round-trip skills/agents on read(); TOOL_EQUIVALENTS translates agent tools
                       for Claude/Gemini/Kiro/Vibe (Gemini `mcp_*` ⇄ Kiro `@mcp`, Kiro todo tool `todo`);
                       claude_model/gemini_model/opencode_model keep only values the tool accepts
                       (claude_model drops dotted ids such as Kiro's `claude-sonnet-4.5`;
                       gemini_model also keeps `gemma-*`); kiro_model/copilot_model/cursor_model drop
                       another host's alias (sonnet, opus, haiku, fable, pro, flash, flash-lite) and any
                       provider/model id, Kiro and Copilot also `inherit`
  adapters/
    mod.rs          — AiToolAdapter trait + registry + shared write_if_changed +
                       collect_rule_files (recursive rules-dir scan, sorted by base name) +
                       ManagedDir / find_orphans (what sync would remove, including skill folders that hold the
                       `.conforme` marker but left the source; check, diff and status report it) /
                       clean_orphans + source_files() (what read() loads outside the
                       managed dirs; see Orphan cleanup below)
    claude.rs       — Claude Code: CLAUDE.md (or .claude/CLAUDE.md when only that exists)
                       + .claude/rules/**/*.md, read recursively (paths: frontmatter); with no CLAUDE.md
                       nor .claude/CLAUDE.md, reads AGENTS.md and .claude/AGENTS.md (root first) as Claude
                       Code does (CLAUDE.local.md deliberately ignored); a skill and a command of the same
                       name read as the skill
    cursor.rs       — Cursor: .cursor/rules/**/*.mdc, read recursively (alwaysApply/globs/description); subagents at .cursor/agents/*.md
    devin.rs        — Devin Desktop (formerly Windsurf): writes .devin/{rules,skills,mcp_config.json}; reads
                       .devin/ and the legacy .windsurf/ (both loaded upstream) and cleans conforme's legacy copies;
                       global_rules.md (.devin/, else .windsurf/) is read into the instructions
    copilot.rs      — GitHub Copilot: .github/copilot-instructions.md (applyTo); skills at .github/skills/<name>/SKILL.md; MCP merged into .vscode/mcp.json;
                       detected from copilot-instructions.md or .github/{instructions,agents,skills}/
    codex.rs        — OpenAI Codex CLI: reads AGENTS.md natively
    opencode.rs     — OpenCode: reads AGENTS.md natively; agents read from .opencode/agents/, .opencode/agent/
                       and the opencode.json `agent` key together (markdown wins; built-in overrides skipped);
                       an agent named like a built-in one (build, plan, …) is never written; config merged into
                       the existing root or .opencode/ opencode.json(c), else a new .opencode/opencode.json
                       (Kilo, which refuses `{env:VAR}`, never reads .opencode/)
    zoocode.rs      — Zoo Code (community fork of Roo Code): .roo/rules/**/*.md, read recursively (plain Markdown);
                       2–3 digit `NN-` prefix stripped and `Intended scope` comment read back as globs; .roorules read
                       when .roo/rules/ is missing or empty (.clinerules does not trigger detection)
    gemini.rs       — Gemini CLI: writes the first `context.fileName` entry other than AGENTS.md (GEMINI.md
                       by default; none when it names only AGENTS.md); on read, the files `context.fileName`
                       names (AGENTS.md among them is read with the AGENTS.md convention); remote agents
                       (`kind: remote`, or an agent card without `kind`) and `_` drafts left alone
    zed.rs          — Zed AI: .rules file
    kiro.rs         — Kiro (AWS): .kiro/steering/*.md (inclusion/fileMatchPattern)
    deepseek.rs     — DeepSeek Harness (dsh): reads AGENTS.md natively; skills at .dsh/skills/<name>/SKILL.md
    vibe.rs         — Mistral Vibe: reads AGENTS.md natively; .vibe/skills/, .vibe/agents/<name>.toml subagents
                       (other agent files are the user's modes), MCP merged into .vibe/config.toml; warns about
                       reserved skill names and `${VAR}` Vibe does not expand
    kilo.rs         — Kilo Code (OpenCode fork): reads AGENTS.md (else CLAUDE.md, CONTEXT.md) and .kilo/rules/*.md
                       natively; .kilo/skills/, .kilo/agents/*.md (built-in names skipped), MCP merged into
                       .kilo/kilo.jsonc or the existing kilo.json(c); legacy .kilocode/ read; warns about a root
                       opencode.json holding `{env:VAR}`
tests/
  integration.rs    — CLI integration tests (assert_cmd + tempfile)
  roundtrip.rs      — Write→read round-trip tests for every adapter (rules, skills, agents, MCP)
  error_cases.rs    — Edge cases, MCP sync, agents sync, activation modes
  migrate_matrix.rs — `migrate` between every pair of tools, and around all of them and back
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
- Codex, OpenCode, Gemini, Zed, DeepSeek Harness, Mistral Vibe, Kilo Code

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
| Copilot | `servers` (inside `.vscode/mcp.json`) + `mcpServers` (inside `.github/mcp.json`) | VS Code reads the first (`env` on stdio, `headers` on HTTP; keeps `inputs`/`sandbox`), Copilot CLI and the cloud agent only the second (Claude Code shape); both merged |
| Devin | `mcpServers` (inside `.devin/mcp_config.json`) | No type field; remote `url` + `transport: http`; `${env:VAR}`; merged |
| OpenCode | `mcp` (inside `opencode.json`: an existing root one, else `.opencode/opencode.json`) | `type: local/remote`; `command` is a single array; env key is `environment` (local only); `{env:VAR}`; merged (preserves user keys and `{ "enabled": false }` toggles) |
| Zed | `context_servers` (inside `.zed/settings.json`) | No type field; merged into existing settings (preserves theme/keybindings/etc. and extension servers configured only via `settings`) |
| Gemini | `mcpServers` (inside `.gemini/settings.json`) | No type field, uses `httpUrl` for HTTP; merged into existing settings |
| Kilo Code | `mcp` (inside `.kilo/kilo.jsonc` or the existing `kilo.json(c)`) | OpenCode shape; no variable reference (Kilo refuses `{env:VAR}` in a project config): `NAME=${NAME}` dropped, other `${VAR}` written as is with a warning; merged |
| Mistral Vibe | `[[mcp_servers]]` (inside `.vibe/config.toml`) | TOML; `transport` stdio / streamable-http; `Bearer ${VAR}` → static `auth.api_key_env`; other `${VAR}` written as is with a warning; atomic merge keeps settings, OAuth auth and target-only servers |
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

Directories two tools share (`.agents/skills/` for Codex and Zed):
`sync::target_config` leaves the source's skills root to the source, and
`remove` never deletes a file the source or another kept tool also generates.
`migrate` generates the output through `target_config` too, never deletes a
file another detected tool generates nor anything in a directory a staying tool
(the output or another detected tool) manages, and in the source's own
directories deletes only files with conforme's suffix not protected by `keep`
(Kiro `.json` agents, Zoo `.txt` rules survive) and, in a skills directory, only
skill sub-folders, never top-level files. It also keeps a skill folder that
bundles other files (only `SKILL.md` reaches the output) and the skills or
agents the output cannot hold. It validates the config and refuses an empty
source, like `sync`; when the output keeps its instructions in `AGENTS.md`
(Codex, OpenCode, DeepSeek, Vibe, Kilo, Gemini CLI loading only `AGENTS.md`) it writes
`AGENTS.md`, and refuses before changing anything when a different `AGENTS.md`
exists that the source does not read. `--dry-run` reports identical files as
"unchanged".

`AiToolAdapter::source_files(project_root)` lists what `read()` loads outside
the tool's managed directories, only paths that exist and are used: Claude Code
`CLAUDE.md` (or `AGENTS.md` + `.claude/AGENTS.md` in the fallback), Codex
`AGENTS.md`, OpenCode the first of `AGENTS.md` / `CLAUDE.md`, Kilo the first of
`AGENTS.md` / `CLAUDE.md` / `CONTEXT.md` plus its rule directories, Vibe
`AGENTS.md` plus `.agents/skills` when `.vibe/skills` has none, DeepSeek the first of `AGENTS.md` /
`CLAUDE.md` plus `.agents/skills` when `.dsh/skills` has none, Gemini CLI its
existing context files, Devin `global_rules.md` and `.windsurfrules`, Zoo Code
`.roorules`. With that tool as the source no target writes them
(`sync::target_files` / `write_target`), `remove`/`migrate` never delete them,
`gitignore install` never ignores them, and `target_config` treats a fallback
skills root like a shared one.

`reads_agents_md(project_root)` is true for Codex, OpenCode, DeepSeek, Vibe and Kilo;
for Claude Code when the project has no `CLAUDE.md` nor `.claude/CLAUDE.md` but
has `AGENTS.md` / `.claude/AGENTS.md` (a personal `CLAUDE.local.md` deliberately
does not change that); for Gemini CLI when `.gemini/settings.json`
`context.fileName` names `AGENTS.md`. Such a tool reads it with the AGENTS.md
convention, and when it is the source `AGENTS.md` is never regenerated nor
gitignored, and `conforme add` refuses to append to an `AGENTS.md` a tool
source regenerates. Renamed tool ids (`windsurf` → `devin`) are an error
wherever an id is accepted.

Every JSON file conforme merges into (`.mcp.json`, `.cursor/mcp.json`,
`.kiro/settings/mcp.json`, `.devin/mcp_config.json`, `opencode.json`,
`.zed/settings.json`, `.gemini/settings.json`, `kilo.json(c)`,
`.vscode/mcp.json`, `.github/mcp.json`, `.roo/mcp.json`) goes through `json_settings`, and together
with `.codex/config.toml` and `.vibe/config.toml` is declared by `is_shared_file()` so `remove`/`migrate`
never delete it wholesale and `gitignore install` never ignores it.

`AiToolAdapter::warnings(project_root, config)` names what the tool will not
load as written (Kilo: `${VAR}` it cannot resolve, a root `opencode.json` it
refuses; Vibe: reserved skill names, `${VAR}` it does not expand); `sync` and
`migrate` print them for every target.

### Sync algorithm

1. Parse AGENTS.md (or the source tool) → NormalizedConfig; an empty config stops with "nothing to sync"
2. For each selected target (`selected_targets`: detected, filtered by `only`/`exclude`): generate expected files minus the source's `source_files()`, write if changed
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

- **verify-providers** — Audit all 13 provider adapters against the tools as they ship today (upstream alive, docs and source read on the web, fact sheet vs adapter code, safety invariants), prove every tool's copy of every skill matches the source (`scripts/skills_conformity.py`), check with the tools' own CLIs (`references/live-cli.md`), run every command with every tool as source plus a second pass by concern, keep secrets out of the copies, check links, then fix with regression tests

## Upstream documentation

The project configures no MCP server. Upstream formats are checked against the
vendors' own pages (the URLs in `docs/providers/<tool>.md`) with the
`verify-providers` skill. An MCP server added to `.mcp.json` is synced into
tracked files of every tool: reference secrets as `${VAR}`, never as values.

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
- MCP JSON keys per tool: Claude/Kiro/Zoo Code/Gemini/Cursor/Devin = `mcpServers`, Copilot = `servers`, OpenCode and Kilo Code = `mcp` (inside `opencode.json` / `kilo.jsonc`), Zed = `context_servers`; Codex `[mcp_servers.<name>]` and Mistral Vibe `[[mcp_servers]]` are TOML
- A JSON MCP entry shape is a `ServerShape` in `src/mcp.rs` (type values, URL key, `env` on remote, env-var syntax); add or change a tool there rather than writing another builder
- OpenCode MCP specifics: `command` is a single array `[cmd, ...args]`, env key is `environment` (not `env`), servers live inside the project's `opencode.json` (an existing root one, else `.opencode/opencode.json`; conforme merges — never clobber user-authored keys)
- Devin (formerly Windsurf): write `.devin/` only; `.windsurf/` is read as a legacy location (Devin loads both) and conforme's old copies there are cleaned. Project MCP is `.devin/mcp_config.json`; never generate `.windsurf/mcp.json`
- Never add a user-authored directory (e.g. `.github/prompts/`) to `managed_directories()`: orphan cleanup deletes every file there that carries the directory's suffix and that conforme did not generate
- Give each `ManagedDir` the exact suffix conforme writes there (`ManagedDir::files(dir, ".agent.md")`), and use `ManagedDir::subdirs` for skills directories, so files the tool accepts but conforme never writes (Kiro `.json` agents, dsh flat skills) survive a sync
- An adapter must generate no files for an empty config and never a blank file (guarded by `test_no_adapter_writes_blank_files`)
- When an upstream tool is retired (as Amazon Q was), delete its adapter outright rather than keeping it behind a deprecation flag; the removal checklist is the mirror of the "adding a new adapter" one above
- Every MCP file is merged, never owned: any adapter that merges into a user-owned settings file (`.mcp.json`, `.cursor/mcp.json`, `.kiro/settings/mcp.json`, `.devin/mcp_config.json`, `opencode.json`, `.zed/settings.json`, `.gemini/settings.json`, `kilo.jsonc`, `.vscode/mcp.json`, `.github/mcp.json`, `.roo/mcp.json`, `.codex/config.toml`, `.vibe/config.toml`) MUST implement `is_shared_file()` so `remove` and `migrate` never delete it wholesale, and its gitignore patterns must not match it
- Merge JSON settings through `json_settings::server_settings_file` (or `load`/`merge_server_entries`/`render_with_removals`), never `serde_json::from_str(..).unwrap_or_default()`: those files are JSONC, a parse failure must refuse the write instead of replacing the user's file, and a source with no server leaves the file untouched
- Kilo Code refuses `{env:VAR}` in a project config, and also loads the root `opencode.json`: Kilo's MCP carries no variable reference, and OpenCode's new config goes to `.opencode/opencode.json`
- A setting a tool will not load as written (a `${VAR}` it cannot resolve, a reserved name, a file it refuses) is reported by `AiToolAdapter::warnings`, which `sync` and `migrate` print
- Cursor subagents: `.md` extension (not `.mdc`); no `tools` frontmatter field — tool access is inherited from the parent agent
- Copilot skills: `.github/skills/<name>/SKILL.md` (NOT `.github/prompts/*.prompt.md` — prompt files are a separate VS Code feature)
- Any adapter whose `generate()` writes skills, agents, or MCP MUST read them back in `read()`, or `--from <tool>` silently drops them
- Skills and agents always carry a `description` (`description_or_name`): Codex, Copilot, Gemini, OpenCode and Zoo Code skip one without it
- A skill is its whole folder: every reader fills `NormalizedSkill::files` (`read_bundled_files`) and every `generate_*_skills` writes them beside `SKILL.md` (`bundled_outputs`); each generated skill folder holds the `.conforme` marker (`SKILL_MARKER`); its skills directory is a `ManagedDir::subdirs`, so the copies are kept in step (`stale_bundled_files`, and a marked folder whose skill left the source is deleted by `stale_skill_folder_files`, never in a directory the source reads) and `verify-providers`' `skills_conformity.py` passes
- Tools with their own tool vocabulary (Claude Code, Gemini CLI, Kiro, Mistral Vibe) get translated `tools` lists (`TOOL_EQUIVALENTS`, MCP names respelled), never names copied verbatim from another host; a `model` another tool cannot use is left out (`claude_model`, `gemini_model`, `opencode_model`, `kiro_model`, `copilot_model`, `cursor_model`)
- Skill and agent names go through `sanitize_name` (kebab-case ASCII, at most 64 characters), rule file names through `rule_file_name`, and MCP strings through `EnvRefStyle` so `${VAR}` becomes each tool's own reference syntax
- A tool that reads `AGENTS.md` itself in a project returns `true` from `reads_agents_md(project_root)` (always for Codex, OpenCode, DeepSeek, Mistral Vibe, Kilo Code; Claude Code without `CLAUDE.md`; Gemini CLI when `context.fileName` names it) and reads it with `markdown::read_native_agents_md`; renaming a tool id adds it to `sync::RENAMED_IDS`
- A file or directory `read()` loads outside the tool's managed directories (a native or fallback `AGENTS.md` / `CLAUDE.md`, Gemini's context files, Devin's `global_rules.md` / `.windsurfrules`, Zoo's `.roorules`, DeepSeek's and Vibe's `.agents/skills` fallback, Kilo's rule directories) MUST be returned by `source_files(project_root)` when it exists and is used, so that with the tool as source no target writes it, `remove`/`migrate` never delete it and `gitignore install` never ignores it; a new read location also goes into `watch::source_locations`
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
<!-- description: Audit conforme's provider adapters against the tools as they ship today, prove every tool's copy of every skill matches the source, and fix what the audit finds -->
<!-- invocation: manual -->
<!-- tools: Read, Grep, Glob, Bash, WebFetch, WebSearch, Edit, Write -->

conforme duplicates one source config into every tool. The audit proves that
each tool, as it ships today, loads the same current skills, rules, agents and
MCP servers that conforme writes for it. It is exhaustive when every
registered adapter has a verdict on each step below, with evidence, every
copy of every skill matches its source, and every fix is merged with a
regression test.

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
  keeping or renaming the tool id is the user's call in step 10;
- **forked**: the original is retired but a maintained fork reads the same
  files (Roo Code, continued by Zoo Code). Rename the adapter to the fork in
  step 11 rather than deleting working output;
- **absorbed by `<successor adapter>`** or **retired**: deleted outright in
  step 11; conforme carries no deprecation flags.

Done when: every adapter has a verdict with its proof URL.

## 3. Fetch the current documentation

Read the vendors' own pages: every URL in `docs/providers/<tool>.md`, opened
with the host's web reader (`WebFetch` on Claude Code), plus the tool's source
when it is open (`gh api`, raw files) for what the docs leave out. When a page
fails or answers a different question, search the vendor's own domain for the
replacement and record the new URL for step 11. Open the changelog or release
notes and scan for config changes since the last audit (the date of the
previous `verify-providers` commit in `git log`).

The research fans out well: one subagent per two or three tools. Their
reports are leads, not evidence. Re-read the source line or page behind every
claim a fix will rely on before writing the fix; earlier audits caught
subagents misreading defaults and incomplete key lists this way.

Extract, per tool, a fact sheet:

- rules: directory, extension, recursive discovery or not, frontmatter fields
  and their types, activation values;
- skills: every directory the tool loads skills from (its own and shared ones
  such as `.agents/skills`), `SKILL.md` frontmatter (required and optional
  keys, name character set and length, description length), whether bundled
  files beside `SKILL.md` are used, manual-invocation mechanism;
- agents: directory, extension, frontmatter keys and accepted values;
- MCP: file path, top-level key, transport `type` values, remote URL key
  (`url`, `serverUrl`, `httpUrl`), env and headers keys, env-var syntax,
  whether the file is project-scoped or user-global;
- instruction file: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.rules`, and the
  fallback order.

Done when: every bullet above has a value or an explicit "not supported" for
every alive adapter.

## 4. Compare the adapter to the fact sheet

Read `src/adapters/<tool>.rs`, plus the tool's generators in `src/skills.rs`
and `src/mcp.rs`. For each fact-sheet line, one of three findings:

- **match**: nothing to do;
- **drift**: paths, keys, values or types differ, fix in step 11;
- **deliberate deviation**: conforme emits something the tool tolerates but
  does not document (for example a `type: "http"` the tool ignores). Keep it
  only if `docs/providers/<tool>.md` already records it as deliberate and the
  tool provably accepts it; otherwise it is drift.

Add these checks, which the fact sheet alone does not cover:

- **Consumed output**: every file `generate()` writes is read by the tool at
  that path. A best-effort file nobody consumes is drift, and the fix is to
  stop writing it, drop the capability, and let `sync` warn that the feature
  is skipped.
- **Source files**: every file or directory `read()` loads outside the tool's
  managed directories (a native or fallback `AGENTS.md` / `CLAUDE.md`, a
  context file named in settings, a fallback skills root) is returned by
  `source_files()` exactly when it is read, and listed in
  `watch::source_locations`.
- **User-owned locations**: orphan cleanup deletes every top-level file of a
  `managed_directories()` entry that carries the entry's suffix and that the
  current config did not generate. A directory the user authors in with the
  same file kind (`.github/prompts/`, the top-level `.opencode/`) must not be
  listed; a skills root is only safe as `ManagedDir::subdirs`.
- **Merged settings**: every MCP or settings file `generate()` writes is
  merged, not owned (`is_shared_file()`, no `gitignore` pattern, JSON through
  `json_settings`), keeps the keys the tool writes into it on its own, and is
  left untouched by a source with no server.
- **Orphan suffixes**: every `ManagedDir` names the suffix conforme writes
  there; list the file kinds the tool accepts in that directory and confirm
  none that conforme does not write shares it.
- **Required fields and vocabularies**: which keys the tool requires for a
  skill or agent and which values it accepts (tool names, MCP tool spellings,
  model ids, name character sets and lengths, env-var syntax, keys remote
  servers accept). Anything copied verbatim from another host must be valid
  in this one.
- **Round-trip**: every feature `generate()` writes, `read()` parses back,
  including bundled skill files and the layouts the tool reads that conforme
  does not write (nested directories, flat files, alternate spellings).
- **Blank output**: an empty config yields no file, and a full config yields
  no blank file (`test_no_adapter_writes_blank_files` guards it).
- **Warnings**: a value conforme writes that the tool will not use as written
  (a `${VAR}` it does not expand, a reserved skill name, a project file it
  refuses) is named by the adapter's `warnings()`, which `sync` and `migrate`
  print; a silent loss is drift.

Done when: the audit table has a finding for each fact-sheet line and each
check above, for every adapter.

## 5. Prove the skills conform

Duplicated skills are the product: every tool's copy of every skill must be
the source's, current, and loadable there. Check it on the repository itself
and on a fixture:

```bash
cargo run --release -- sync && python3 .claude/skills/verify-providers/scripts/skills_conformity.py .
```

The script compares each copy root (`.agents/skills`, `.cursor/skills`,
`.gemini/skills`, …) with `.claude/skills`, the dogfooded source: missing
copies, a `name` that is not the folder, a missing or over-long
`description`, different instructions, bundled files missing, different or
extra, a copy without conforme's `.conforme` marker, and a marked copy whose
skill left the source. Then build a fixture whose source skills cover a bundled script, a
manual skill, a nested skill and a long description, sync it to every tool,
run the script with `--source <its skills dir>`, and repeat after editing a
skill body, deleting a bundled file and deleting a whole skill.

Done when: the script exits 0 on the repository and on the fixture after
each edit and sync, and every UNMANAGED line (a skill written by hand in one
tool) is explained.

## 6. Check with the tools' own CLIs

Every tool installed here reads the fixture and says what it loads, following
[`references/live-cli.md`](references/live-cli.md): MCP servers resolved with
their env and headers, skills and agents accepted, a manual skill kept out of
automatic use. A witness file proves each validator actually checked.

Then check switching, conforme's other job: set up the full fixture in each
tool's own format (`migrate --source claude --output <tool>`), migrate it
into every installed tool, and ask that tool's CLI the same questions.
`tests/migrate_matrix.rs` covers every pair offline; this run proves each
output is what the real tool loads.

Done when: each adapter is **verified** (with the command), **not installed**
or **needs login**, every source migrated into each installed tool loads its
MCP servers, skills and agents there, and every rejection or warning the
tools print is a finding.

## 7. Run every command, then a second pass

Unit tests cover what their author thought of. Run the CLI against the
fixture with each of the 13 tools as source in turn: `sync` twice (the second
changes nothing), `check`, `status`, `diff`, `migrate` to tools that share a
directory (Codex, Zed) and to one that keeps its instructions in
`AGENTS.md`, `remove`, `gitignore install`. Look for lost files, blank files,
a source file rewritten, and commands that disagree with `sync`.

Then audit again with a different slicing: by concern across all adapters
(`watch`, `detect`, `gitignore`, `validate`, `status`/`check`/`diff`, `add`,
`migrate`) rather than adapter by adapter. A second pass sliced this way has
found what the first missed every time.

Done when: every command has run with every tool as source and the
second-pass concerns each have a verdict.

## 8. Keep secrets out of the copies

Every MCP header and env value is copied into each tool's file, and those
files are committed. A value in clear in `.mcp.json` (gitignored) once leaked
an API key through every target. Check the tracked files:

```bash
git ls-files | grep -E '^\.[^/]+/.*(mcp|settings|config\.toml)|^opencode\.jsonc?$|^\.mcp\.json$' \
  | xargs grep -nE '"[^"]*([Kk]ey|[Tt]oken|[Ss]ecret|[Aa]uth)[^"]*"\s*[:=]\s*"[^"$\{][^"]*"' || echo "no literal secret"
```

Done when: no tracked file holds a credential in clear (every one is a
`${VAR}` reference in that tool's syntax), and `validate` warns about such a
value in the source.

## 9. Verify every documentation link

```bash
grep -oh 'https://[^ )>,`"]*' docs/providers/*.md README.md CLAUDE.md | sed 's/[.,]$//' | sort -u > urls.txt
while read -r u; do printf '%s %s\n' "$(curl -sS -o /dev/null -w '%{http_code}' -A 'Mozilla/5.0' --max-time 25 -L "$u")" "$u"; done < urls.txt | sort | grep -v '^200'
```

GitHub answers `429` under a burst; retry those alone with a pause. An
example endpoint in a config snippet (an MCP server URL) may answer `401`.
Any other non-200 is a broken link: find the replacement on the vendor domain
and fix it in step 11. A `200` can still be a stub ("Page moved" with a meta
refresh): the step 3 fetch catches those. Done when: the loop prints nothing
unexplained and no fetched page is a stub.

## 10. Classify every finding

Before fixing, sort the audit table into three lists so the user can see the
decisions, and confirm the third one:

- **fix now**: drift, broken links, failed safety, conformity or CLI checks;
- **leave**: deliberate deviations already documented as such;
- **delete or rename the adapter**: absorbed, retired, forked or renamed
  upstream (step 2). Deleting or renaming a tool id is a breaking change and
  ships as a major release: the commit needs `!` and a `BREAKING CHANGE:`
  footer with the migration, and say so before merging.

## 11. Fix, with a regression test per finding

For each "fix now" and "delete" item, update every affected location. The
checklist in `.claude/rules/adapter-consistency.md` is authoritative; the
usual set is:

1. `src/adapters/<tool>.rs`, `src/skills.rs`, `src/mcp.rs`, `src/sync.rs`,
   `src/gitignore.rs`, `src/watch.rs`;
2. `docs/providers/<tool>.md` (facts, deviations, new URLs);
3. `README.md` tables and tool count, `CLAUDE.md` architecture and MCP
   tables, `src/help_ai.rs`, `src/cli.rs` tool count and tool list, this
   skill's description;
4. tests: `tests/roundtrip.rs` for round-trips, `tests/error_cases.rs` and
   `tests/integration.rs` for behaviour, `src/mcp.rs` unit tests for MCP
   shapes. A safety finding (user files deleted, blank file, settings file
   removed) gets an integration test that reproduces the loss and asserts
   the file survives; each new test fails on the code before the fix;
5. a deleted adapter also leaves the registry, `all_adapters()` order-based
   tests, `.gitignore` templates, and the dogfooded output directory; a
   renamed one changes its module, struct, id, `docs/providers/` file and
   every test fixture that names it.

Then regenerate the dogfooded configs, prove the skills again, and run the
full gate:

```bash
cargo run --release -- sync && cargo run --release -- check
python3 .claude/skills/verify-providers/scripts/skills_conformity.py .
cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check
```

Done when: the gate is green, the conformity script exits 0, `git status`
shows only intended files, and a `grep -rni '<old tool id>'` returns only
sentences that describe the removal or rename (release notes live on GitHub
Releases, not in the tree).

## 12. Report

One table, one row per adapter:

| Tool | Upstream status | Findings | Fixed | Left as deliberate | CLI check |

followed by the three lists from step 10 and the PR link. State plainly what
was not verified (a page that could not be read, a tool not installed here, a
behaviour only a manual run in the IDE would confirm) and the known gaps left
for a later change. After the merge, check that semantic-release published
the expected version and that the release binaries are attached.
