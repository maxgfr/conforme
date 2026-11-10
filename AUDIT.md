# Code audit — 2026-11-10

Scope: the whole crate (`src/`, `tests/`, `Cargo.toml`, CI), from `main` at `6cf6974` (v5.3.2).

## Summary

1. conforme is a mature, well-tested Rust CLI (≈28 k lines, 755 tests at the start, 777 now). Its baseline was green: build, tests, `clippy -D warnings`, `fmt`, `conforme check`.
2. The core engine (`sync.rs`) is careful about user data, but `migrate` and `sync` still had paths that deleted or overwrote files the user wrote. Those are the main fixes below.
3. The MCP layer (`mcp.rs`, `json_settings.rs`) is robust: no panics on user data, no TOML or JSON injection, and JSONC files edited in place. Two Codex/Vibe round-trip gaps are fixed.
4. Performance is not a concern: every command takes ~15–60 ms on a 14-tool, 723-file fixture. The remaining cost is mostly process start and small file I/O.
5. What is left is mainly design questions (below), two unmaintained transitive dependencies, and some duplication (directory walkers, env-reference predicates).

## Bugs fixed

Each fix comes with a test that failed before the fix and passes after it, except the watch-timing fix (E6), whose failure depends on timing and was reasoned through instead.

| # | File | Issue | Fix | Test added |
|---|---|---|---|---|
| E1 | `src/sync.rs` (`migrated_files`), `src/adapters/mod.rs` (`ManagedDir::recursive`) | `migrate` deleted every file at any depth under a source directory. That included files the tool never reads: a nested `.kiro/steering/archive/old.md`, nested Kiro/Cursor/Antigravity/Gemini/Vibe agents, and a `.kiro/skills/notes/README.md` folder that is not a skill. They were lost. | `ManagedDir::recursive` marks the directories a tool reads recursively (Claude, Cursor, Devin, Zoo rules; Copilot instructions and agents; Claude, Codex, OpenCode and Kilo agents). Elsewhere only top-level files are migrated. In a skills directory, only files of a folder holding a `SKILL.md` are. | `tests/integration.rs::test_migrate_keeps_nested_files_the_source_does_not_read` |
| E2 | `src/sync.rs` (`target_config`, `target_managed_dirs`), `src/adapters/mod.rs` (`find_orphans`, `clean_superseded_skills`) | Paths were compared as spelled. With `.cursor/skills -> ../.claude/skills` and `source = "claude"`, Cursor rewrote the source's own `SKILL.md` files and dropped `allowed-tools`. Orphan cleanup also walked into a linked skill folder and could delete files outside the project. | `is_one_of` also matches paths through links. Skill-folder walks use `is_real_dir`. | `tests/integration.rs::test_sync_leaves_source_skills_behind_a_symlinked_target_dir_alone`, `adapters::tests::test_find_orphans_does_not_walk_a_linked_skill_folder` |
| E3 | `src/sync.rs` (`refuse_hand_written_agents_md`, AGENTS.md write) | Any read error on AGENTS.md (a Latin-1 file, a permission error) was treated as "no file", so the hand-written-file guard was skipped and the file silently replaced. | Only `NotFound` means absent. Any other error stops the command and nothing is changed. | `tests/error_cases.rs::test_sync_refuses_to_replace_an_agents_md_it_cannot_read` |
| M1 | `src/mcp.rs` (`merge_codex_mcp_toml`) | Every non-string `env_vars` item was kept, not only `source = "remote"` ones. A local `{ name = "TOKEN" }` table was then (a) duplicated by the plain name, (b) still forwarded after the variable left the source, and (c) on read, overrode a literal `env` value with `${TOKEN}`. | Keep only tables whose `source` is set and is not `local`. | `mcp::tests::test_codex_merge_replaces_local_env_var_tables` |
| P1 | `src/validate.rs` | A skill or agent with an empty name (`## Skill:` followed by spaces, `name: ""`) passed validation. It was written into every tool as `.claude/skills/SKILL.md`, `.claude/agents/.md` or `.codex/agents/.toml`. | "Skill/Agent with empty name found" is an error, as it already was for rules and MCP servers. | `validate::tests::test_skill_and_agent_without_a_name_are_errors`, `tests/error_cases.rs::test_sync_refuses_skills_and_agents_without_a_name` |
| P2 | `src/markdown.rs` (`export_as_agents_md`) | A body with an unclosed code fence (a Cursor rule ending mid-example) made every later line of the generated AGENTS.md parse as text, so all following sections were lost on read-back by Codex/OpenCode/Kilo or `migrate --source codex`. | `push_section_text` closes a fence a body leaves open. | `markdown::tests::test_export_closes_a_code_fence_left_open` |
| E5 | `src/watch.rs` | On macOS, FSEvents reports resolved absolute paths, so `watch -C .`, `-C proj` or `-C /tmp/proj` matched no watched path. It printed its list, then ignored every edit. Reproduced manually with the release binary. | The root is resolved (`watch_root`) before the watch paths are built. Checked again by hand after the fix. | `watch::tests::test_events_reported_under_the_resolved_root_are_relevant` |
| E6 | `src/watch.rs` | The fingerprint was taken *after* the sync, so an edit saved while a sync ran counted as already synced and was skipped. | The fingerprint is now the content the sync started from. | none: depends on timing |
| E7 | `src/sync.rs` (`run_remove`) | `conforme remove cursor` failed outright when another detected tool's settings did not parse (`.gemini/settings.json`). `migrate` already tolerated this. | That tool is generated without its MCP servers (its settings files are shared and never removed), and its managed directories stay protected. | `tests/error_cases.rs::test_remove_is_not_blocked_by_another_tools_unreadable_settings` |
| P3 | `src/frontmatter.rs` (`serialize`) | A file written without frontmatter (a Claude manual rule) whose body starts with `---` was read back as a frontmatter block. Its first lines were dropped, or the read failed. | Such a body gets an empty `---\n---` block in front. | `frontmatter::tests::test_serialize_without_fields_keeps_a_leading_dash_line_in_the_body`, `tests/roundtrip.rs::test_roundtrip_claude_rule_body_starting_with_a_dash_line` |
| P4 | `src/adapters/zoocode.rs` | Zoo Code loads rules by file name and the order prefix was always two digits, so `100-x.md` sorted between `10-` and `11-`. | The prefix is as wide as the last index needs (`000-general.md` from 100 files on). | `adapters::zoocode::tests::test_rule_order_survives_a_hundred_rules` |
| M3 | `src/mcp.rs` (`shell_split`) | `char::is_whitespace` split a Vibe command on U+00A0, U+3000 or `\v`, where Python's `shlex` (which Vibe uses) does not. Checked against `python3 shlex.split`. | Split only on space, tab, CR and LF. | extended `mcp::tests::test_shell_split_matches_python_shlex` |
| M2 | `src/mcp.rs` (`vibe_api_key`) | A Vibe `api_key_format = "Token {token}"` read back as `Authorization: Token ${T}`, but was written back as a literal header. Vibe then sent the text `${T}`. | A header holding exactly one `${VAR}`, with plain text around it, becomes `api_key_env` plus `api_key_format`. | `mcp::tests::test_vibe_custom_api_key_format_round_trips` |

### User-visible behavior changes (flagged)

- **New validation errors:** a skill or agent with an empty name now fails `sync`/`check`. Before, it produced stray files.
- **Unreadable AGENTS.md:** `sync` now fails when a hand-written AGENTS.md cannot be read (for example, it is not UTF-8). Before, it replaced the file.
- **`migrate` keeps more files:** nested files in directories the tool reads only at the top level, and non-skill folders in skills directories, now stay in place instead of being deleted.
- **Zoo Code file names:** projects with 100 or more Zoo rules get three-digit prefixes (`000-general.md`, `001-…`). The first sync renames their files.
- **Vibe:** a header such as `X-Key: Bearer ${K}` or `Authorization: Token ${T}` is now written as `auth.api_key_env` + `api_key_format` instead of a literal header. That is what Vibe can use, and `sync` no longer warns about it.
- **Error text:** a file that cannot be read is named once, not twice ("failed to read X: failed to read X").
- **`watch`** now works with a relative `-C` or a root reached through a link on macOS.

## Optimizations

I measured on a fixture with all 14 tools, 40 rules, 15 skills, 10 agents and 6 MCP servers (723 files), 30–80 runs per command:

| Command | Median | Min |
|---|---|---|
| `check` | 41 ms | 26 ms |
| `sync -n` | 33 ms | 26 ms |
| `status` | 37 ms | 27 ms |
| `check --from claude` | 54 ms | 33 ms |
| `diff --from claude` | 61 ms | 36 ms |
| `migrate --dry-run --source claude --output cursor` | 18 ms | 13 ms |

- **`perf(adapters)` (`e0af872`):** each Claude rule file is now read once instead of twice, and each Antigravity rule file once instead of three times. That is 30 + 80 fewer reads on the fixture. **Measured gain: below noise** (`check --from claude` / `--from antigravity` ≈ 22 ms minimum before and after). The files are small and already in the OS cache. I kept the change because it costs nothing and does not change behavior.
- I made no other optimization. At 15–60 ms per command, larger caching work would not be noticeable and would add risk. Candidates are listed below.

## Issues found but not fixed

### Critical

None.

### Important

1. **Unmaintained transitive dependencies.** `cargo audit` passes with two warnings:
   - `instant` (RUSTSEC-2024-0384), pulled in by `notify` 7.
   - `unsafe-libyaml` (RUSTSEC-2024-0449), pulled in by `serde_yaml_ng` 0.10.

   *Recommendation:* upgrade to `notify` 8 and `notify-debouncer-mini` 0.6, which drop `instant`. Evaluate a YAML crate with no `unsafe-libyaml` dependency, for example `serde_norway` or `saphyr`. Each is a library swap with a round-trip risk on frontmatter, so do it in its own PR and run the full roundtrip suite.
2. **The OpenCode V1 migration can delete a hand-written server.** See open question Q1: `adapters/opencode.rs::config_files` combined with `json_settings::merge_server_entries`.
3. **The hook can be installed into a global hooks directory.** With a global `core.hooksPath`, `hook install` writes `conforme check || exit 1` into a hook every repository uses. Commits then fail in repositories that have no conforme source. *Recommendation:* refuse, or warn, when the hooks directory is outside the repository's git dir, or guard the block with `[ -f .conformerc.toml ] || [ -f AGENTS.md ]`. See Q4.

### Minor

4. **Four separate recursive walkers have drifted apart:** `sync::collect_files_recursive`, `adapters::collect_rule_files_into`, `skills::skill_bundle_paths` and `watch::fingerprint`. Bug E2 came from this drift in how links are handled. *Recommendation:* one walker in `adapters/mod.rs` (real dirs only, optional depth, optional suffix).
5. **`main.rs` declares every module again** instead of using the `conforme` library crate, so the library modules compile twice and dead-code analysis differs between the two crates. *Recommendation:* move `sync`, `watch`, `hook`, `gitignore`, `help_ai`, `detect` and `cli` into `lib.rs` and make `main.rs` a thin `use conforme::…`. It is mechanical but touches every `crate::` path, so I left it out of this audit.
6. **Redundant generation in `init` and `migrate`:** `report_pending_orphans` (`sync.rs`) reads the source and regenerates every target a second time. `unheld_skills_and_agents(output)` runs twice in `migrate`. `target_config`/`target_managed_dirs` rebuild the 14 adapters and the source's `source_files()` for every target. *Recommendation:* compute these once in the command and pass them down, and give `Held` a per-item cache. The expected gain is a few ms, so this is low priority.
7. **Three different "is this a variable reference" predicates** (`mcp::env_ref_name`, `is_var_name`, Kilo's `value.trim() == "${key}"`). One server can be classified differently depending on the target. *Recommendation:* unify them into one helper.
8. **Codex parse:** when a hand-written `config.toml` sets the same name in both `env` and `env_vars`, the `env_vars` entry silently wins on read (`parse_codex_mcp_toml`). conforme itself no longer writes such a file (M1).
9. **Copy-paste readers:** `cursor.rs` re-implements `skills::read_agents_from_dir(.., false)`, and `claude.rs` re-implements `read_skills_from_dir` with an extra name filter. They differ in subtle filters, so I did not merge them blind. A refactor would need a parameterized predicate.
10. **More than 999 Zoo rules** would need a 4-digit prefix, which `strip_order_prefix` (2–3 digits) does not strip. This is theoretical.
11. **The Vibe `api_key_format` generalization (M2)** relies on the `{token}` substitution conforme already used on read. Confirm it against Vibe's source at the next `verify-providers` run.

## Open questions (need your decision)

1. **OpenCode V1 → V2 migration and the root `opencode.json`** (`adapters/opencode.rs::config_files`, `json_settings::merge_server_entries`). A hand-written, enabled `mcp.<name>` in the root `opencode.json` that is not in the source gets moved out of the root file into `.opencode/opencode.json`. The merge there then drops it, because it is expressible, not disabled, and absent from the source. Net effect: the server is deleted from both files, though conforme says it never writes the root file. Should the move apply only to entries an earlier conforme wrote, or keep unknown entries?
2. **E4: should `sync` refuse to replace a hand-written AGENTS.md whose `## Skill:` / `## Agent:` / `## MCP:` sections the source lacks?** Today only the instructions and rules are checked (`text_is_covered`). I implemented the stricter check, but it broke `test_sync_from_every_tool_is_idempotent`. That test expects `sync --from devin` (Devin has no agents) to replace an AGENTS.md holding an agent, since choosing such a source drops agents from every tool anyway. I reverted it. Which behavior do you want?
3. **Gemini's bare `$word`:** it is read as `${word}` (`EnvRefStyle::DollarOrBare`), but Gemini leaves an unset `$word` as written. A literal `$` in a value then becomes a variable reference in every other tool. Is it a reference or text?
4. **`hook install` with a global `core.hooksPath`:** refuse, warn, or guard the block (Important #3)?
5. **`init --force`** replaces AGENTS.md with the template and syncs it over every tool's instruction files, with no backup. Is that the intent of "Overwrite existing tool configs"?
6. **Cursor / VS Code `${foo}`:** a name that is neither predefined nor `env:` is treated as an environment variable and rewritten to `${env:foo}`. Is that right, or should unknown names stay literal?
7. **Metadata-looking comments:** an `<!-- description: … -->` or `<!-- tools: … -->` at the start of a section *body* is read back as metadata. Stopping the metadata block at the first blank line would fix it, but the documented convention allows blank lines between metadata comments. Keep it tolerant, or make it strict?
8. **A rule named `General` with no instructions** is read back from Cursor, Devin, Kiro or Zoo as the instructions and loses its `## Rule:` heading. `validate.rs` checks the collision only when there are instructions. Should it warn always?
9. **Copilot agents:** `.github/agents/**/*.md` is read recursively with any `.md` suffix, so a `README.md` there becomes an agent and is propagated. The Claude adapter filters such files (`is_claude_agent_file`). Should Copilot require `.agent.md`, or frontmatter?
10. **The same skill in `AGENTS.md` and `.claude/AGENTS.md`** (Claude fallback) fails with "Duplicate skill name". Rules override, but skills and agents append (`markdown::read_agents_md_files`). Should the later file win, as for rules?
11. **Long non-ASCII rule names:** `rule_file_name` does not cap length, so a long CJK rule name can exceed 255 bytes and the write fails. Should it be truncated (with a hash suffix)?
12. **`split_globs` splits inside a character class** (`[a,b]`). Should it be bracket-aware like the brace handling?
