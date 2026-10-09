# Architecture notes

Reference details moved out of `CLAUDE.md` to keep the generated `AGENTS.md` within the
32 KiB Codex reads (`project_doc_max_bytes`). `CLAUDE.md` points here.

## Rules-directory discovery

Claude Code, Cursor and Zoo Code all scan their rules directory **recursively**
(`.claude/rules/frontend/react.md`, `.cursor/rules/backend/rpc.mdc`, …).
`adapters::collect_rule_files` implements that scan for all three, sorting by
base name (case-insensitive, so Zoo's `00-`/`01-` prefixes keep their meaning)
then by full path. Nested rules are written back flat, one file per rule name.
`clean_orphans` stays non-recursive, so hand-authored files in subdirectories are
never deleted.

## Orphan cleanup and shared files

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
`AGENTS.md`, OpenCode the first of `AGENTS.md` / `CLAUDE.md` / `CONTEXT.md`, Kilo the first of
`AGENTS.md` / `CLAUDE.md` / `CONTEXT.md` plus its rule directories, Vibe
`AGENTS.md` plus `.agents/skills` when `.vibe/skills` has none, DeepSeek the first of `AGENTS.md` /
`CLAUDE.md` plus `.agents/skills` when `.dsh/skills` has none, Gemini CLI its
existing context files, Devin `global_rules.md` and `.windsurfrules`, Cursor `.cursorrules`, Zoo Code
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
refuses; Vibe: reserved names, `${VAR}`; Codex, Zed, Zoo: references kept literal); `sync` and
`migrate` print them for every target.
