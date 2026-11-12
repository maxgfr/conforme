---
inclusion: always
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

`src/main.rs` dispatches the subcommands of `src/cli.rs` to `src/sync.rs`, the core engine; each tool is an adapter in `src/adapters/`, with skills and agents generated in `src/skills.rs` and MCP formats in `src/mcp.rs`. **Read `docs/architecture.md`, the module-by-module map, before changing an adapter or one of those files.** It lives outside this file because this file is copied into `AGENTS.md`, of which Codex reads only the first 32 KiB.

## Keeping docs in sync

**IMPORTANT**: When adding/removing adapters, commands, or changing behavior, update ALL of:
1. `README.md` — supported tools table, CLI commands, activation mapping table
2. `src/help_ai.rs` — the `conforme help-ai` output must list all tools with correct formats
3. `src/cli.rs` — the `after_help` examples and `long_about` tool count
4. `src/project_config.rs` — if adding new config options
5. `src/validate.rs` — if adding new validation rules
6. `src/watch.rs` — if changing watched file patterns
7. `docs/providers/<tool>.md` — provider-specific documentation
8. `docs/architecture.md` — module map; this `CLAUDE.md` — key concepts and MCP table (keep `AGENTS.md` under 32 KiB)

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

`## Plugin: <name>` declares a plugin (manifest metadata comments, its body
the plugin's instructions); `<!-- plugin: <name> -->` places a Rule, Skill,
Agent or MCP section in it. Claude Code, Vibe, Antigravity and Codex (through
its repository marketplace `.agents/plugins/marketplace.json`, merged and
shared) hold plugins as
bundles (`AiToolAdapter::plugin_shape`, a `plugins::PluginShape`; bundle dirs
are `ManagedKind::Bundles`, owned through the `.conforme-plugin` marker); the
trait's `generate` / `warnings` flatten what a tool cannot hold
(`plugins::flatten_for`) before its `generate_flat` / `tool_warnings`. A
`## Plugin:` with `<!-- marketplace: m -->` is a plugin enabled from a
marketplace (`## Marketplace: m` declares it), synced between Claude Code and
Copilot `enabledPlugins` / `extraKnownMarketplaces` and Codex `[plugins]`
(`src/marketplaces.rs`). A bundle's hooks are translated between Claude Code,
Vibe and Antigravity (`src/hooks.rs`); see `docs/plugins.md`.

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
| OpenCode | `mcp.servers` (inside `.opencode/opencode.jsonc` if it exists, else `.opencode/opencode.json`; never the root one) | OpenCode 2's native shape: `type: local/remote` (SSE is `remote` with a warning, read as HTTP); `disabled` toggle; `command` is a single array; env key is `environment` (local only); `{env:VAR}`, editor variables such as `${workspaceFolder}` kept verbatim; merged (preserves user keys and every other member of `mcp`: `mcp.timeout`, OpenCode 1 `mcp.<name>` entries); OpenCode 1 entries (`mcp.<name>`, `agent`) are neither read nor touched, in any config file |
| Zed | `context_servers` (inside `.zed/settings.json`) | No type field; SSE becomes streamable HTTP (warned); merged into existing settings (preserves theme/keybindings/etc. and extension servers configured only via `settings`) |
| Gemini | `mcpServers` (inside `.gemini/settings.json`) | `httpUrl` for HTTP, `url` + `type: "sse"` for SSE; merged into existing settings |
| Kilo Code | `mcp` (inside `.kilo/kilo.jsonc` or the existing `kilo.json(c)`) | OpenCode 1 shape (SSE is `remote`); no variable reference (Kilo refuses `{env:VAR}` in a project config): `NAME=${NAME}` dropped, other `${VAR}` written as is with a warning; merged |
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
`sync::target_config` leaves the source's skills root (or a link to it) to the source, and
`remove` never deletes a file the source or another kept tool also generates.
`migrate` generates the output through `target_config` too, never deletes a
file another detected tool generates nor anything in a directory a staying tool
(the output or another detected tool) manages, and in the source's own
directories deletes only files with conforme's suffix not protected by `keep`
(Kiro `.json` agents, Zoo `.txt` rules survive), nested files only in a
directory the tool reads recursively (`ManagedDir::recursive`: Claude, Cursor,
Devin and Zoo rules, Copilot instructions and agents, Claude/Codex/OpenCode/Kilo
agents, Codex/Cursor/Kilo/OpenCode skills) and, in a skills directory, only files
of a skill folder (a direct child holding a `SKILL.md`, the first one met at
any depth where the tool scans recursively; not a skill nested in it), never
top-level files. It also keeps a skill folder that
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
`AGENTS.md`, OpenCode `AGENTS.md`, Kilo the first of
`AGENTS.md` / `CLAUDE.md` / `CONTEXT.md` plus its rule directories, Vibe
`AGENTS.md` plus `.agents/skills` when `.vibe/skills` has none, DeepSeek the first of `AGENTS.md` /
`CLAUDE.md` plus `.agents/skills` when `.dsh/skills` has none, Gemini CLI its
existing context files, Devin `global_rules.md` and `.windsurfrules`, Cursor `.cursorrules`, Antigravity CLI the first of `AGENTS.md` / `GEMINI.md`, Zoo Code
`.roorules`. With that tool as the source no target writes them
(`sync::target_files` / `write_target`), `remove`/`migrate` never delete them,
`gitignore install` never ignores them, and `target_config` treats a fallback
skills root like a shared one.

Symbolic links: a file may be reached under two names (`CLAUDE.md ->
AGENTS.md`, `.cursor/skills -> ../.claude/skills`, a single skill folder or
settings file linked likewise), so paths are compared where they really are
(`sync::is_source_file`, `is_one_of`, `LinkWalk`, `is_beneath` against
`source_read_paths`; `source_files_for` names the source's linked files
where they are; `sync::TargetGuard` holds the checks for one target). sync
never writes, nor cleans as an orphan, a target file that really is one the
source reads, in either direction of the link, nor generates an `AGENTS.md`
over it; a target leaves a file it reaches through a link to the other target
that writes that very file (`GEMINI.md -> CLAUDE.md`, `.kiro/steering ->
../.claude/rules`), and never cleans as an orphan a file another target reaches
through its own link; without any link in the project (`project_has_links`,
looked up once per command) none of this costs anything; nothing reached through a link out of the project is
ever deleted. `remove` keeps such a file, and one a staying tool reaches
through its own link; `gitignore install` keeps such links tracked (`!` lines,
a directory pattern turned into one on its entries); `migrate` replaces the
output link that leads into the source with a file or directory of its own
(`--dry-run` says so), keeps a source file that really is a staying tool's
(a source file that is itself a link goes, its target staying), and removes a
link at the project root it left pointing at nothing.

`reads_agents_md(project_root)` is true for Codex, OpenCode, DeepSeek, Vibe, Kilo and Antigravity CLI;
for Claude Code when the project has no `CLAUDE.md` nor `.claude/CLAUDE.md` but
has `AGENTS.md` / `.claude/AGENTS.md` (a personal `CLAUDE.local.md` deliberately
does not change that); for Gemini CLI when `.gemini/settings.json`
`context.fileName` names `AGENTS.md`. Such a tool reads it with the AGENTS.md
convention, and when it is the source `AGENTS.md` is never regenerated nor
gitignored, and `conforme add` refuses to append to an `AGENTS.md` a tool
source regenerates. Renamed tool ids (`windsurf` → `devin`) are an error
wherever an id is accepted.

In a tool that has its own off switch (`disabled: true` for Kiro, Zoo, Devin, Antigravity and OpenCode
`mcp.servers`; `enabled: false` for Zed and Kilo; Codex `enabled = false`; Vibe `disabled = true`), a
disabled server is not read from a source and the tool's own disabled server is kept; Claude Code, Cursor,
Copilot and Gemini keep their toggles outside the file. Every other server the source does not define is
dropped from every target, Codex and Vibe included. conforme targets OpenCode 2 only: it reads and writes the native
`mcp.servers` and `agents`; OpenCode 1's `mcp.<name>` and `agent` entries are the user's, never read nor touched.

Every JSON file conforme merges into (`.mcp.json`, `.cursor/mcp.json`,
`.kiro/settings/mcp.json`, `.devin/mcp_config.json`, `opencode.json`,
`.zed/settings.json`, `.gemini/settings.json`, `kilo.json(c)`,
`.vscode/mcp.json`, `.github/mcp.json`, `.roo/mcp.json`, `.agents/mcp_config.json`) goes through `json_settings`, and together
with `.codex/config.toml` and `.vibe/config.toml` is declared by `is_shared_file()` so `remove`/`migrate`
never delete it wholesale and `gitignore install` never ignores it.

`AiToolAdapter::warnings(project_root, config)` names what the tool will not
load as written (Kilo: `${VAR}` it cannot resolve, a root `opencode.json` it
refuses; OpenCode: SSE servers; Vibe: built-in and reserved agent names, `${VAR}`; Codex, Zed, Zoo: references kept literal); `sync` and
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
