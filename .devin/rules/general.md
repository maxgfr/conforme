---
trigger: always_on
---

# CLAUDE.md

## Project overview

conforme is a Rust CLI that synchronizes AI coding agent configurations across 14 tools, and switches a project from one tool to another (`migrate`). It reads config from a source tool (Claude Code, Cursor, etc.) or AGENTS.md, and propagates to all other tool-specific config files.

## Build & test

```bash
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings    # lint — MUST pass before pushing
cargo fmt -- --check           # format check
conforme check                 # verify AI configs are in sync (dogfooding)
```

## Architecture

```
src/
  main.rs           — Entry point, dispatches subcommands
  cli.rs            — CLI arg parsing (clap derive)
  config.rs         — NormalizedConfig, NormalizedRule, NormalizedSkill, NormalizedAgent, NormalizedMcpServer, ActivationMode;
                       yaml_globs reads a glob field as YAML list or comma string (Claude `paths`, Cursor `globs`);
                       read_text / normalize_text: every file read from a tool loses its BOM and CRLF
  markdown.rs       — AGENTS.md parser (sections → rules via ## Rule: headings); metadata only in a section's
                       leading comment block (any spacing, any case of the mode); a value that would not read
                       back (comma, line break, `-->`, edge spaces) is written in JSON form, both forms read
  frontmatter.rs    — `---` block split by hand (BOM, CRLF, files of 3 bytes or fewer), parsed as YAML, else
                       entry by entry (`globs: **/*.ts`); `read` names the file in errors; is_strict_yaml for
                       tools that skip a broken block (Antigravity); serialization
  lib.rs            — Library crate re-exports (adapters, config, etc.)
  sync.rs           — Core sync engine: init, sync, check, status, remove, diff, migrate commands
                       (target_config, renamed-id check, AGENTS.md output unless the source reads it, holding only
                       instructions and rules (migrate adds what the output cannot hold); a Codex
                       warning past its 32 KiB AGENTS.md budget; generated AGENTS.md names its source on
                       line 1 and sync/check refuse a source changed by hand since (migrate moves it); remove prunes emptied directories
                       and says how to exclude a tool still detected;
                       selected_targets (detection + only/exclude) is the one target set of sync, check,
                       diff and status; target_files/write_target leave out the source's source_files();
                       generated_agents_md is compared by check and diff too; check validates the config;
                       status shows excluded tools as "Excluded" and AGENTS.md as "Source's own file" /
                       "Generated output" / "Not managed";
                       a source that reads back empty warns "nothing to sync" and writes/cleans nothing;
                       migrate keeps what staying tools generate or manage — see Orphan cleanup below;
                       each target is generated once per command (TargetOutput: files written, expected list for
                       orphan cleanup); one target that fails does not stop the others, the command fails at the end;
                       init never cleans and lists what the next sync would remove; sync refuses to replace a
                       hand-written AGENTS.md whose text the source lacks; sync -n lists orphans and AGENTS.md;
                       check and diff refuse what sync refuses (checked_source); status shows read errors;
                       migrate prunes emptied directories and points `.conformerc.toml` `source` at its output;
                       add validates the resulting AGENTS.md before writing; Held memoizes what a tool holds)
  detect.rs         — Tool detection (which tools present in project)
  json_settings.rs  — JSONC-safe merge of conforme's keys into user-owned JSON settings files
                       (in-place edit keeps comments; unparsable file is refused, never overwritten;
                       merge_server_entries keeps per-server keys conforme does not own;
                       is_expressible_server: an entry with neither `command` nor `url`/`httpUrl`/`serverUrl`,
                       or a Claude `type: "sdk"` one, is skipped on read and kept untouched on write
                       (Zed extension servers, Claude SDK servers, OpenCode `{ "enabled": false }` toggles);
                       server_settings_file leaves the file untouched when the source has no server;
                       server_settings_file_or_bare moves a bare-format file's servers under the key (Copilot CLI);
                       a tool's own toggle (ServerShape.toggle, an owned `disabled` / `enabled`) skips a disabled
                       entry on read and keeps one absent from the source on write; Zed's legacy
                       `command: {path, args, env}` is expressible; an existing file is edited in place (key order, indent, comments; an
                       unchanged key keeps its text), a BOM is stripped, a comments-only file is `{}`)
  hook.rs           — Git pre-commit hook install/uninstall (like Husky): hooks directory from
                       `git rev-parse --git-path hooks` (worktrees, core.hooksPath), a `# >>> conforme >>>` block
                       right after the shebang, `conforme check -C <dir>` from a sub-directory; a non-shell hook
                       is refused; uninstall also removes the block earlier versions appended
  gitignore.rs      — `conforme gitignore install/uninstall` (a block without its end marker is refused,
                       an unchanged file is "already up to date"): ignores the files each non-source
                       adapter selected by only/exclude owns (root files anchored, rules/agents dirs
                       by suffix; Gemini's pattern is the context file it writes); merged settings
                       files, the source's own locations and its source_files() stay tracked
  project_config.rs  — .conformerc.toml parser (source, only, exclude, clean options); strict: an unknown key,
                       a parse error or an unknown or renamed tool id (RENAMED_IDS) is an error in every command
  validate.rs        — Config validation (duplicate names, empty content, invalid globs,
                       names that sanitize to nothing or collide, skill descriptions over the 1024
                       characters Zoo Code and Vibe accept,
                       a rule whose file name becomes `general` while there are instructions — Cursor,
                       Devin and Kiro write the instructions there; warns on skills Claude Code reserves:
                       synced, anthropic-skills)
  watch.rs           — File watcher for auto-sync (notify + debounce): watches every location the
                       source reads (per-tool source_locations + source_files()), .conformerc.toml and
                       AGENTS.md; an existing directory recursively, a file or missing path through its
                       nearest existing parent (never a whole .claude/ or .github/ tree); re-plans after
                       each change so directories created later are watched, and forgets paths that disappeared;
                       each sync re-reads .conformerc.toml; syncs only when a watched file's content changed
  help_ai.rs        — Detailed help about all supported tools and formats
  mcp.rs            — MCP config generation/parsing per tool:
                       - each JSON format is a `ServerShape` (type values, URL key, env on remote, env-var
                         syntax, layout, on/off toggle): `build`, `owned_keys` (every URL spelling and `type`
                         included), `read` / `read_file`; OpenCode and Kilo are shapes too; every JSON target
                         is merged via json_settings
                       - env_ref_losses: warnings for a `${VAR:-default}` a syntax cannot hold and editor
                         variables (`${workspaceFolder}`) a tool lacks
                       - EnvRefStyle: `${VAR}` normalized, written as `${env:VAR}` (Cursor, VS Code, Zoo,
                         Devin) or `{env:VAR}` (OpenCode), read back by canonicalize_env_refs
                       - Codex: project `.codex/config.toml`, atomically merged with comment/settings preservation; strict safe parser — merge_codex_mcp_toml / parse_codex_mcp_toml.
                         Codex expands no `${VAR}`: stdio `NAME=${NAME}` → `env_vars`, `Authorization: Bearer ${VAR}` →
                         `bearer_token_env_var`, an exact `${VAR}` header → `env_http_headers`, all read back to `${VAR}`;
                         `env` on HTTP is dropped; tuning keys (timeouts, `enabled_tools`, `required`, …) are read past and kept;
                         a server that left the source is dropped, an `enabled = false` one kept
                       - per-tool keys and shapes: see "MCP key mapping per tool" below
                       - Claude .mcp.json parsing: `sse` maps to Sse, http/https/streamable-http/ws to Http;
                         `type: "sdk"` entries are skipped
                       - OpenCode: merge_opencode_agents keeps the user's own `agent` entries (the adapter
                         drops one an earlier sync wrote for an agent that left the source)
                       - Kilo: KILO_SHAPE (no variable reference)
                       - Vibe: merge_vibe_mcp_toml / parse_vibe_mcp_toml; a string `command` is shell-split
                         as Vibe does, one that would not survive it is written as a list; vibe_api_key maps
                         `Bearer ${VAR}`, else an exact `${VAR}` header, to `api_key_env`; an inline
                         `mcp_servers = []` is accepted; a server that left the source is dropped
                       - parse_mcp_json reads back mcpServers / servers / context_servers
                       - OpenCode agents: parse_opencode_agent_object
  skills.rs         — Skills: one generate_skills driven by a SkillShape per tool (directory, `allowed-tools`,
                       manual-invocation encoding, Codex policy sidecar); SKILL.md plus the text files bundled beside it: read_bundled_files,
                       written next to SKILL.md in every tool, stale ones found by stale_bundled_files)
                       and agents generation per tool; shared read helpers
                       preserve manual_invocation (and Codex policy sidecars, which Vibe honours too,
                       created only in .agents/skills) across sync (read_skills_from_dir, read_skills_recursive,
                       read_agents_from_dir, parse_frontmatter_tool_list) used by adapters to
                       round-trip skills/agents on read(); parse_frontmatter_tool_list splits outside parentheses
                       (`Bash(git add:*)`); a skill folder holding a SKILL.md without the marker is never adopted
                       nor pruned; TOOL_EQUIVALENTS translates agent tools
                       for Claude/Gemini/Kiro/Vibe (Gemini `mcp_*` ⇄ Kiro `@mcp`, Kiro todo tool `todo`; a Kiro agent's `task`, its task-list tool, reads as `todo`);
                       claude_model/gemini_model/opencode_model keep only values the tool accepts
                       (claude_model drops dotted ids such as Kiro's `claude-sonnet-4.5`;
                       gemini_model also keeps `gemma-*`); kiro_model/copilot_model/cursor_model drop
                       another host's alias (sonnet, opus, haiku, fable, pro, flash, flash-lite) and any
                       provider/model id, Kiro and Copilot also `inherit`
  adapters/
    mod.rs          — AiToolAdapter trait + registry + shared write_files (atomic for is_shared_file paths),
                       write_if_changed, contents_match (line ends aside), is_real_dir (walks never follow a
                       linked directory), push_inline_rule, is_instructions_file (only the top-level `general`) +
                       collect_rule_files (recursive rules-dir scan, sorted by base name) +
                       ManagedDir / find_orphans (what sync would remove, including skill folders that hold the
                       `.conforme` marker but left the source; check, diff and status report it) /
                       clean_orphans + source_files() (what read() loads outside the
                       managed dirs; see Orphan cleanup below)
    claude.rs       — Claude Code: CLAUDE.md (or .claude/CLAUDE.md when only that exists)
                       + .claude/rules/**/*.md, read recursively (paths: frontmatter); with no CLAUDE.md
                       nor .claude/CLAUDE.md, reads AGENTS.md and .claude/AGENTS.md (root first) as Claude
                       Code does (CLAUDE.local.md deliberately ignored); a skill and a command of the same
                       name read as the skill; skips what Claude Code refuses: a skill or command name with
                       control, zero-width, bidi, private-use, odd blank or </> characters (claude_loads_name),
                       an agent whose frontmatter is not strict YAML or whose name starts with -, holds : or
                       passes 256 characters
    cursor.rs       — Cursor: .cursor/rules/**/*.mdc, read recursively (alwaysApply/globs/description), plus .cursorrules; subagents at .cursor/agents/*.md
    devin.rs        — Devin Desktop (formerly Windsurf): writes .devin/{rules,skills,mcp_config.json}; reads
                       .devin/ and the legacy .windsurf/ (both loaded upstream) and cleans conforme's legacy copies;
                       global_rules.md (.devin/, else .windsurf/) and .windsurfrules are read into the instructions
    copilot.rs      — GitHub Copilot: .github/copilot-instructions.md (applyTo); skills at .github/skills/<name>/SKILL.md; MCP merged into .vscode/mcp.json
                       and .github/mcp.json (a bare-format file is read, and moved under mcpServers on write:
                       server_settings_file_or_bare); agents read from .github/agents/ recursively;
                       detected from copilot-instructions.md or .github/{instructions,agents,skills}/
    codex.rs        — OpenAI Codex CLI: reads AGENTS.md natively; agents at .codex/agents/*.toml; warns when the
                       source has MCP servers or agents and $CODEX_HOME/config.toml (default ~/.codex) trusts
                       neither the project nor its main repository root (codex_trusts: untrusted .codex/ layers
                       are ignored)
    opencode.rs     — OpenCode: reads AGENTS.md natively; agents read from .opencode/agents/, .opencode/agent/
                       and the opencode.json `agent` key together (markdown wins; built-in overrides skipped);
                       an agent named like a built-in one (build, plan, …) is never written; config merged into
                       the existing root or .opencode/ opencode.json(c), else a new .opencode/opencode.json
                       (Kilo, which refuses `{env:VAR}`, never reads .opencode/)
    zoocode.rs      — Zoo Code (community fork of Roo Code): .roo/rules/**/*.md, read recursively (plain Markdown);
                       2–3 digit `NN-` prefix stripped, `Intended scope` comment read back as globs and
                       `Apply when` as an agent-decision description; .roorules read
                       when .roo/rules/ is missing or empty (.clinerules does not trigger detection)
    gemini.rs       — Gemini CLI: writes the first `context.fileName` entry other than AGENTS.md (GEMINI.md
                       by default; none when it names only AGENTS.md); on read, the files `context.fileName`
                       names (AGENTS.md among them is read with the AGENTS.md convention); remote agents
                       (`kind: remote`, or an agent card without `kind`) and `_` drafts left alone
    zed.rs          — Zed AI: .rules file
    kiro.rs         — Kiro (AWS): .kiro/steering/*.md (inclusion/fileMatchPattern)
    deepseek.rs     — DeepSeek Harness (dsh): reads AGENTS.md natively; skills at .dsh/skills/<name>/SKILL.md
    vibe.rs         — Mistral Vibe: reads AGENTS.md natively; .vibe/skills/, .vibe/agents/<name>.toml subagents
                       (other agent files are the user's modes; built-in agent names never written), MCP merged
                       into .vibe/config.toml; warns about reserved names and `${VAR}` Vibe does not expand
    kilo.rs         — Kilo Code (OpenCode fork): reads AGENTS.md (else CLAUDE.md, CONTEXT.md) and .kilo/rules/*.md
                       natively; .kilo/skills/, .kilo/agents/*.md (built-in names skipped), MCP merged into
                       .kilo/kilo.jsonc or the existing kilo.json(c) (read: every kilo/opencode.json(c) Kilo
                       loads); legacy .kilocode/ read; warns about a root
                       opencode.json holding `{env:VAR}`
    antigravity.rs  — Antigravity CLI: reads AGENTS.md (else GEMINI.md); rules in .agents/rules/*.md (top level,
                       Devin's trigger vocabulary); skills in the shared .agents/skills/; subagents in
                       .agents/agents/*.md (built-in names never written); MCP merged into .agents/mcp_config.json
                       (`serverUrl`, no `type`); warns about `${VAR}`, SSE servers and manual skills
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
- Claude, Cursor, Devin, Copilot, Kiro, Zoo Code, Antigravity

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
| Claude, Kiro | `mcpServers` (`.mcp.json`, `.kiro/settings/mcp.json`) | `type: stdio/http/sse` (Kiro: SSE has no `type`); merged (keeps `oauth`, Kiro `autoApprove`/`disabledTools`, Claude `type: "sdk"` entries) |
| Cursor | `mcpServers` (`.cursor/mcp.json`) | `type: stdio` locally, no `type` on remote entries (SSE: `type: "sse"`); `${env:VAR}`; merged |
| Zoo Code | `mcpServers` (inside `.roo/mcp.json`) | HTTP uses `type: streamable-http` (not `http`), SSE `type: "sse"`, no `env` on remote servers; merged (keeps Zoo's `alwaysAllow`/`disabledTools`) |
| Copilot | `servers` (inside `.vscode/mcp.json`) + `mcpServers` (inside `.github/mcp.json`) | VS Code reads the first (`env` on stdio, `headers` on HTTP; keeps `inputs`/`sandbox`), Copilot CLI only the second (Claude Code shape); both merged, SSE as `type: "sse"` in both. The cloud agent reads no file: its servers are in the repository settings |
| Devin | `mcpServers` (inside `.devin/mcp_config.json`) | No type field; remote `url` + `transport: http` (SSE: `transport: "sse"`); `${env:VAR}`; merged |
| OpenCode | `mcp` (inside `opencode.json`: an existing root one, else `.opencode/opencode.json`) | `type: local/remote` (SSE is `remote`, read as HTTP); `command` is a single array; env key is `environment` (local only); `{env:VAR}`, editor variables such as `${workspaceFolder}` kept verbatim; merged (preserves user keys and `{ "enabled": false }` toggles) |
| Zed | `context_servers` (inside `.zed/settings.json`) | No type field; SSE becomes streamable HTTP (warned); merged into existing settings (preserves theme/keybindings/etc. and extension servers configured only via `settings`) |
| Gemini | `mcpServers` (inside `.gemini/settings.json`) | `httpUrl` for HTTP, `url` + `type: "sse"` for SSE; merged into existing settings |
| Kilo Code | `mcp` (inside `.kilo/kilo.jsonc` or the existing `kilo.json(c)`) | OpenCode shape (SSE is `remote`); no variable reference (Kilo refuses `{env:VAR}` in a project config): `NAME=${NAME}` dropped, other `${VAR}` written as is with a warning; merged |
| Mistral Vibe | `[[mcp_servers]]` (inside `.vibe/config.toml`) | TOML; `transport` stdio / streamable-http (SSE becomes streamable-http, warned); `Bearer ${VAR}` → static `auth.api_key_env`; other `${VAR}` written as is with a warning; atomic merge keeps settings and OAuth auth; a server that left the source is dropped, a `disabled` one kept |
| DeepSeek Harness | _(none)_ | MCP is a user-level `cordis.patch.yml` plugin entry under `$DSH_HOME`; nothing project-scoped is generated |
| Codex | `[mcp_servers.<name>]` (inside `.codex/config.toml`) | TOML; SSE becomes streamable HTTP (warned); no `${VAR}` expansion (mapping under mcp.rs above); atomic merge preserves unrelated settings, comments and Codex-specific options; a server that left the source is dropped, an `enabled = false` one kept |
| Antigravity CLI | `mcpServers` (`.agents/mcp_config.json`) | No `type`; remote `serverUrl` + `headers` (SSE uses the same key, read back as HTTP); `${VAR}` written as is (expansion unconfirmed, warned); merged |

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

Directories two tools share (`.agents/skills/` for Codex, Zed and Antigravity CLI):
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
`AGENTS.md`, OpenCode the first of `AGENTS.md` / `CLAUDE.md` / `CONTEXT.md`, Kilo the first of
`AGENTS.md` / `CLAUDE.md` / `CONTEXT.md` plus its rule directories, Vibe
`AGENTS.md` plus `.agents/skills` when `.vibe/skills` has none, DeepSeek the first of `AGENTS.md` /
`CLAUDE.md` plus `.agents/skills` when `.dsh/skills` has none, Gemini CLI its
existing context files, Devin `global_rules.md` and `.windsurfrules`, Cursor `.cursorrules`, Antigravity CLI the first of `AGENTS.md` / `GEMINI.md`, Zoo Code
`.roorules`. With that tool as the source no target writes them
(`sync::target_files` / `write_target`), `remove`/`migrate` never delete them,
`gitignore install` never ignores them, and `target_config` treats a fallback
skills root like a shared one.

`reads_agents_md(project_root)` is true for Codex, OpenCode, DeepSeek, Vibe, Kilo and Antigravity CLI;
for Claude Code when the project has no `CLAUDE.md` nor `.claude/CLAUDE.md` but
has `AGENTS.md` / `.claude/AGENTS.md` (a personal `CLAUDE.local.md` deliberately
does not change that); for Gemini CLI when `.gemini/settings.json`
`context.fileName` names `AGENTS.md`. Such a tool reads it with the AGENTS.md
convention, and when it is the source `AGENTS.md` is never regenerated nor
gitignored, and `conforme add` refuses to append to an `AGENTS.md` a tool
source regenerates. Renamed tool ids (`windsurf` → `devin`) are an error
wherever an id is accepted.

In a tool that has its own off switch (`disabled: true` for Kiro, Zoo, Devin, Antigravity and OpenCode 2
`mcp.servers`; `enabled: false` for Zed and OpenCode; Codex `enabled = false`; Vibe `disabled = true`), a
disabled server is not read from a source and the tool's own disabled server is kept; Claude Code, Cursor,
Copilot and Gemini keep their toggles outside the file. Every other server the source does not define is
dropped from every target, Codex and Vibe included. OpenCode 2's native `mcp.servers` and `agents` are read
too (OPENCODE_V2_SHAPE), and a native entry shadowing a V1 one conforme writes is warned about.

Every JSON file conforme merges into (`.mcp.json`, `.cursor/mcp.json`,
`.kiro/settings/mcp.json`, `.devin/mcp_config.json`, `opencode.json`,
`.zed/settings.json`, `.gemini/settings.json`, `kilo.json(c)`,
`.vscode/mcp.json`, `.github/mcp.json`, `.roo/mcp.json`, `.agents/mcp_config.json`) goes through `json_settings`, and together
with `.codex/config.toml` and `.vibe/config.toml` is declared by `is_shared_file()` so `remove`/`migrate`
never delete it wholesale and `gitignore install` never ignores it.

`AiToolAdapter::warnings(project_root, config)` names what the tool will not
load as written (Kilo: `${VAR}` it cannot resolve, a root `opencode.json` it
refuses; Vibe: reserved names, `${VAR}`; Codex, Zed, Zoo: references kept literal); `sync` and
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

`conforme --help` lists them (`src/cli.rs`: subcommands and `after_help` examples), as does the README's
Quick start: init, sync, check, status, diff, remove, add, watch, migrate, hook, gitignore, help-ai.

## Skills

This project uses Claude Code skills in `.claude/skills/`:

- **verify-providers** — Verify all 14 provider adapters online, with a proof URL for every fact, the tools' own CLIs and every command; fix drift with regression tests (see its `SKILL.md`)

## Upstream documentation

The project configures no MCP server. Upstream formats are checked online
against the vendors' own pages, changelogs and source with the
`verify-providers` skill; each `docs/providers/<tool>.md` says when and
against which version (`Last verified online`). An MCP server added to `.mcp.json` is synced into
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
