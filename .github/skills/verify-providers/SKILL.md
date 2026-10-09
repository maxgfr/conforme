---
allowed-tools: Read Grep Glob Bash WebFetch WebSearch Edit Write
description: Audit conforme's provider adapters against the tools as they ship today, prove every tool's copy of every skill matches the source, and fix what the audit finds
disable-model-invocation: true
metadata:
  opencode/autoinvoke: 'false'
name: verify-providers
---

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

Done when: each adapter is **verified** (with the command), **not installed**
or **needs login**, and every rejection or warning the tools print is a
finding.

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
