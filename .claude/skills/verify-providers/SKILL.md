---
name: verify-providers
description: Use when auditing conforme's provider adapters against upstream documentation, after a vendor announcement (rename, acquisition, retirement, new config format), before a release, or when a user reports that a tool ignores or rejects a generated file
allowed-tools: Read Grep Glob Bash WebFetch WebSearch Edit Write
disable-model-invocation: true
metadata:
  opencode/autoinvoke: 'false'
---

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
