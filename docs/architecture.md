# Architecture

What each module of conforme owns. Update it with any change to a module's responsibilities, an adapter or a test file.

```
src/
  main.rs           — Entry point, dispatches subcommands; a thin binary over the library crate
  cli.rs            — CLI arg parsing (clap derive)
  config.rs         — NormalizedConfig, NormalizedRule, NormalizedSkill, NormalizedAgent, NormalizedMcpServer,
                       NormalizedPlugin (manifest metadata, disabled, instructions and its own components), ActivationMode;
                       yaml_globs reads a glob field as YAML list or comma string (Claude `paths`, Cursor `globs`);
                       read_text / normalize_text: every file read from a tool loses its BOM and CRLF
  markdown.rs       — AGENTS.md parser (sections → rules via ## Rule: headings); metadata only in a section's
                       leading comment block (any spacing, any case of the mode); a value that would not read
                       back (comma, line break, `-->`, edge spaces) is written in JSON form, both forms read;
                       export closes a code fence a section body leaves open; `## Plugin:` sections hold manifest
                       metadata and instructions, a `<!-- plugin: x -->` comment places a Rule/Skill/Agent/MCP
                       section in that plugin (declared or not, before or after it), a second `## Plugin:` of
                       the same name stays apart so flattening reports it
  marketplaces.rs   — Plugins enabled from a marketplace (NormalizedPluginRef, `<plugin>@<marketplace>`) and declared
                       marketplaces (NormalizedMarketplace, Claude Code's source shape): read_settings / settings_file
                       for Claude Code and Copilot settings (`enabledPlugins`, `extraKnownMarketplaces`; `@skills-dir`
                       switches stay apart; merged in place; no ref: untouched), read_codex_refs /
                       merge_codex_refs_toml for Codex `[plugins]` (its repository marketplace's tables left alone),
                       codex_add_argument for the `codex plugin marketplace add` warning; holds_plugin_refs on the
                       trait marks the three tools, sync warns for the others
  hooks.rs          — Hooks (NormalizedHook: Claude Code's events and tool names) in each bundle format, and the
                       project's own (merge_vibe_project_toml: `conforme-` names; antigravity_group: the `conforme`
                       group; Claude, Copilot and Codex take claude_json; sync gates them on `hooks = true`): read_claude /
                       claude_json, vibe_toml / read_vibe (pre_tool, post_tool, post_agent; tool names through
                       skills::vibe_tool), antigravity_json / read_antigravity (a `conforme` group; ANTIGRAVITY_TOOLS,
                       the names `agy plugin validate` gives); `${CLAUDE_PLUGIN_ROOT}` ⇄ `${PLUGIN_ROOT}`; what a format
                       lacks is left out and warned, and payload_warning names the protocol a script must handle
  plugins.rs        — Plugin bundles: flatten moves a plugin's content into the top-level config for a tool
                       that cannot hold a bundle (instructions → always-on rule, `<plugin>-<name>` on a name
                       clash, items using ${PLUGIN_ROOT}-style variables and disabled plugins left out, each
                       warned; empty or duplicate plugin names an error); flatten_for keeps the plugins a
                       PluginShape holds (CLAUDE_PLUGINS, VIBE_PLUGINS, ANTIGRAVITY_PLUGINS, CODEX_PLUGINS: dir,
                       BundleFormat; holds_rules, holds_agents — Codex bundles hold neither)
                       and flattens only the rest (rules of a Claude or Vibe bundle; a Claude plugin named like
                       a skill is left out, warned); generate_plugins / read_plugins write and read bundles
                       (manifest per format, components, loose text files, the PLUGIN_MARKER `.conforme-plugin`;
                       a hand-written bundle is never written over, bundle_warnings says so); stale_bundle_files
                       is orphan cleanup inside a marked bundle. sync prints flattening's warnings once for the
                       source (checked_plugins), compares AGENTS.md flattened (flat_view, parse_agents_md_flat),
                       and migrate carries plugins flattened (flattened_source) while keeping source bundles
  frontmatter.rs    — `---` block split by hand (BOM, CRLF, files of 3 bytes or fewer), parsed as YAML, else
                       entry by entry (`globs: **/*.ts`); `read` names the file in errors; is_strict_yaml for
                       tools that ignore a broken block (Antigravity rules, which accept an unquoted
                       `description:` holding a colon; Claude agents; Claude rules, read as always-on); serialization (a body opening with `---` gets an empty block in front)
  lib.rs            — Library crate: declares every module (compiled and unit-tested once)
  sync.rs           — Core sync engine: init, sync, check, status, remove, diff, migrate commands
                       (target_config, renamed-id check, AGENTS.md output unless the source reads it, holding only
                       instructions and rules (migrate adds what the output cannot hold); a Codex
                       warning past its 32 KiB AGENTS.md budget; generated AGENTS.md names its source on
                       line 1 and sync/check refuse a source changed by hand since (migrate moves it); remove prunes emptied directories, is not blocked by another tool's unreadable settings file,
                       and says how to exclude a tool still detected;
                       selected_targets (detection + only/exclude) is the one target set of sync, check,
                       diff and status; target_files/write_target leave out the source's source_files() and any
                       file reached through a link into what the source reads (`CLAUDE.md -> AGENTS.md`,
                       `.cursor/skills -> ../.claude/skills`: is_source_file, is_one_of, LinkWalk and is_beneath
                       against source_read_paths, all held by TargetGuard, which orphan cleanup obeys too) or that
                       the source reads through its own link; a link into another tool's directory yields; remove and migrate
                       never delete a file that really is the source's or a staying tool's, migrate replaces an
                       output link into the source with a file or directory of its own;
                       generated_agents_md is compared by check and diff too; check validates the config;
                       status shows excluded tools as "Excluded" and AGENTS.md as "Source's own file" /
                       "Generated output" / "Not managed";
                       a source that reads back empty warns "nothing to sync" and writes/cleans nothing;
                       migrate keeps what staying tools generate or manage — see Orphan cleanup below;
                       each target is generated once per command (TargetOutput: files written, expected list for
                       orphan cleanup); one target that fails does not stop the others, the command fails at the end;
                       init never cleans and lists what the next sync would remove (so does migrate, naming what it
                       kept that the output cannot hold); sync refuses to replace a
                       hand-written AGENTS.md whose text, or a skill, agent or MCP section it could hold, the source lacks,
                       or that it cannot read (not UTF-8); the generated AGENTS.md keeps the sections the source
                       cannot hold at all (carried_agents_md_sections); sync -n lists orphans and AGENTS.md;
                       check and diff refuse what sync refuses (checked_source); status shows read errors;
                       migrate prunes emptied directories and points `.conformerc.toml` `source` at its output;
                       add validates the resulting AGENTS.md before writing; Held memoizes what a tool holds)
  detect.rs         — Tool detection (which tools present in project)
  json_settings.rs  — JSONC-safe merge of conforme's keys into user-owned JSON settings files
                       (in-place edit keeps comments; unparsable file is refused, never overwritten;
                       merge_server_entries keeps per-server keys conforme does not own;
                       is_expressible_server: an entry with neither `command` nor `url`/`httpUrl`/`serverUrl`,
                       or a Claude `type: "sdk"` one, is skipped on read and kept untouched on write
                       (Zed extension servers, Claude SDK servers, Kilo `{ "enabled": false }` toggles);
                       server_settings_file leaves the file untouched when the source has no server;
                       server_settings_file_or_bare moves a bare-format file's servers under the key (Copilot CLI);
                       a tool's own toggle (ServerShape.toggle, an owned `disabled` / `enabled`) skips a disabled
                       entry on read and keeps one absent from the source on write; Zed's legacy
                       `command: {path, args, env}` is expressible; an existing file is edited in place (key order, indent, comments; an
                       unchanged key keeps its text), a BOM is stripped, a comments-only file is `{}`)
  hook.rs           — Git pre-commit hook install/uninstall (like Husky): hooks directory from
                       `git rev-parse --git-path hooks` (worktrees, core.hooksPath), a `# >>> conforme >>>` block
                       right after the shebang, `conforme check -C <dir>` from a sub-directory; a non-shell hook
                       and a hooks directory outside the repository (a global core.hooksPath, shared by every
                       repository) are refused; uninstall also removes the block earlier versions appended
  gitignore.rs      — `conforme gitignore install/uninstall` (a block without its end marker is refused,
                       an unchanged file is "already up to date"): ignores the files each non-source
                       adapter selected by only/exclude owns (root files anchored, rules/agents dirs
                       by suffix; Gemini's pattern is the context file it writes); merged settings
                       files, the source's own locations and its source_files() stay tracked
  project_config.rs  — .conformerc.toml parser (source, only, exclude, clean, hooks options; `hooks` gates the project's own hooks); strict: an unknown key,
                       a parse error or an unknown or renamed tool id (RENAMED_IDS) is an error in every command
  validate.rs        — Config validation (duplicate or empty names, empty content, invalid globs,
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
                       - OpenCode: OPENCODE_SHAPE is OpenCode 2's `mcp.servers` (`disabled`), the only shape
                         read or written for OpenCode
                       - Kilo: KILO_SHAPE (OpenCode 1's shape: `mcp.<name>`, `enabled`; no variable reference)
                       - Vibe: merge_vibe_mcp_toml / parse_vibe_mcp_toml; a string `command` is shell-split
                         as Vibe does, one that would not survive it is written as a list; vibe_api_key maps
                         `Bearer ${VAR}`, else a header holding one `${VAR}`, to `api_key_env` + `api_key_format`; an inline
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
                       linked directory, a linked skill folder is never an orphan), push_inline_rule, is_instructions_file (only the top-level `general`) +
                       collect_rule_files (recursive rules-dir scan, sorted by base name) +
                       plugin_shape + the provided generate / warnings, which flatten what the tool cannot hold
                       (plugins::flatten_for) before the adapter's generate_flat / tool_warnings +
                       ManagedDir (`kind`: a ManagedKind, `Files(suffix)`, `Skills` or `Bundles`, matched exhaustively wherever
                       a directory's contents are cleaned or migrated; `recursive`: the tool reads nested files there,
                       so migrate moves them) / find_orphans (what sync would remove, including skill folders that hold the
                       `.conforme` marker but left the source; check, diff and status report it) /
                       clean_orphans + source_files() (what read() loads outside the
                       managed dirs; see Orphan cleanup below)
    claude.rs       — Claude Code: CLAUDE.md (or .claude/CLAUDE.md when only that exists)
                       + .claude/rules/**/*.md, read recursively (paths: frontmatter, ignored when not YAML); with no CLAUDE.md
                       nor .claude/CLAUDE.md, reads AGENTS.md and .claude/AGENTS.md (root first) as Claude
                       Code does (CLAUDE.local.md deliberately ignored); a skill and a command of the same
                       name read as the skill; skips what Claude Code refuses: a skill or command name with
                       control, zero-width, bidi, private-use, odd blank or </> characters (claude_loads_name),
                       an agent whose frontmatter is not strict YAML or whose name starts with -, holds : or
                       passes 256 characters; warns about a reserved MCP server name and a credential variable
                       Claude Code reads as empty in a remote server's url and headers
    cursor.rs       — Cursor: .cursor/rules/**/*.mdc, read recursively (alwaysApply/globs/description), plus .cursorrules; subagents at .cursor/agents/*.md
    devin.rs        — Devin Desktop (formerly Windsurf): writes .devin/{rules,skills,mcp_config.json}; reads
                       .devin/ and the legacy .windsurf/ (both loaded upstream) and cleans conforme's legacy copies;
                       global_rules.md (.devin/, else .windsurf/) and .windsurfrules are read into the instructions
    copilot.rs      — GitHub Copilot: .github/copilot-instructions.md (applyTo); skills at .github/skills/<name>/SKILL.md; MCP merged into .vscode/mcp.json
                       and .github/mcp.json (a bare-format file is read, and moved under mcpServers on write:
                       server_settings_file_or_bare); agents read from .github/agents/ recursively;
                       warns about an agent named like a Copilot CLI built-in agent;
                       detected from copilot-instructions.md or .github/{instructions,agents,skills}/
    codex.rs        — OpenAI Codex CLI: reads AGENTS.md natively; agents at .codex/agents/*.toml; warns when the
                       source has MCP servers or agents and $CODEX_HOME/config.toml (default ~/.codex) trusts
                       neither the project nor its main repository root (codex_trusts: untrusted .codex/ layers
                       are ignored); plugin bundles in .codex/plugins/, listed in the merged repository
                       marketplace .agents/plugins/marketplace.json (merged_marketplace: only entries pointing
                       at conforme's bundles are its own) and switched by [plugins."<p>@<marketplace>"] in
                       .codex/config.toml (merge_codex_plugins_toml, composed with the MCP merge)
    opencode.rs     — OpenCode 2 only: reads AGENTS.md natively (no CLAUDE.md / CONTEXT.md fallback); agents
                       written to .opencode/agents/<name>.md only, read from .opencode/agents/, .opencode/agent/
                       and the opencode.json `agents` key together (markdown wins; built-in
                       overrides skipped); an agent named like a built-in one (build, plan, …) is never written;
                       MCP merged into `mcp.servers` of .opencode/opencode.jsonc if it exists, else
                       .opencode/opencode.json — never the root one (Kilo, which refuses `{env:VAR}`, loads it
                       and never reads .opencode/); OpenCode 1 entries (`mcp.<name>`, `agent`) are neither read
                       nor touched, in any project opencode.json(c)
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
                       into .vibe/config.toml; warns about built-in and reserved (`main`) agent names and `${VAR}` Vibe does not expand
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
