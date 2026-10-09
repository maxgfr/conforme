---
status: approved
---

# Codex custom agents and a distinct SSE transport

## Goal
`conforme sync` writes every source agent to Codex as `.codex/agents/<name>.toml` (Codex
loads them, `--from codex` reads them back), and an MCP server on the legacy HTTP+SSE
transport stays SSE in every tool that can express it, instead of being rewritten as
streamable HTTP; the tools that cannot express it get it as streamable HTTP with a warning.

## Locked constraints
- Q-001 — Two PRs from `main` after #13 (merged as 616685c) → this plan is PR A; Antigravity is
  `docs/plans/2026-10-09-antigravity-target.md` (PR B, after this one).
- Q-002 — A Codex agent file holds `name`, `description`, `developer_instructions` only; no `model`,
  no tools → S-007.
- Q-003 — Codex built-in names `default`, `worker`, `explorer` are never written nor read, such a file
  is never swept, and `sync` warns → S-007, S-008.
- Q-004 — Orphan cleanup in `.codex/agents/` deletes only a top-level `.toml` whose keys are a subset
  of `name`, `description`, `developer_instructions` → S-007, S-008.
- Q-005 — SSE is a new variant `McpTransport::Sse { url, headers }` → S-001.
- Q-006 — SSE is written as: Claude Code, Copilot (`.vscode/mcp.json` and `.github/mcp.json`), Zoo Code,
  Cursor `type: "sse"` + `url`; Gemini CLI `url` + `type: "sse"`; Devin `url` + `transport: "sse"`;
  Kiro `url` with no `type`; OpenCode/Kilo `type: "remote"` (they retry SSE themselves); Codex, Zed,
  Mistral Vibe as streamable HTTP plus a warning; AGENTS.md `<!-- transport: sse -->` → S-002 to S-006.

## Grounded facts
Repository (`main` at 616685c):
- `src/config.rs:50` — `pub enum McpTransport { Stdio { command, args }, Http { url, headers } }`.
- `src/mcp.rs:944-957` — `struct ServerShape { stdio_type, http_type, url_key, http_extra, env_on_http, env_refs }`;
  shapes at `:961` CLAUDE, `:972` CURSOR, `:982` KIRO, `:993` COPILOT, `:1005` ZOOCODE, `:1016` ZED,
  `:1026` GEMINI (`url_key: "httpUrl"`), `:1037` DEVIN (`http_extra: Some(("transport", "http"))`).
- `src/mcp.rs:1054-1100` — `build_servers_object` writes each entry from a `ServerShape`; remote branch at `:1077-1089`.
- `src/mcp.rs:1585-1596` — `parse_mcp_json` reads `type`, finds the URL in `url`/`httpUrl`/`serverUrl`, and maps
  `http|https|sse|streamable-http|ws` (or a bare URL) to `McpTransport::Http`.
- `src/mcp.rs:1266-1278` — `parse_opencode_mcp_object` maps `type: "remote"` to `McpTransport::Http`.
- `src/mcp.rs:10` `merge_codex_mcp_toml`, `:523` `parse_codex_mcp_toml`, `:282` `merge_vibe_mcp_toml`,
  `:418` `parse_vibe_mcp_toml`, `:161` `server_strings`, `:182` `codex_keeps_literal`.
- `src/markdown.rs:400` — `build_mcp` builds `McpTransport::Http { url, headers }` when a `url:` comment exists;
  `src/markdown.rs:540` — `export_as_agents_md` writes `<!-- url: ... -->` for a remote server.
- `src/validate.rs:66`, `:200` — `McpTransport::Http` matches (headers, empty URL).
- `src/adapters/kilo.rs:151`, `src/adapters/vibe.rs:120` — `warnings()` match `McpTransport::Http`.
- `src/adapters/codex.rs:29-36` — `capabilities()` has `agents: false`; `:48-52` `managed_directories`;
  `:91-117` `read()`; `:119-144` `generate()`.
- `src/skills.rs:603` `generate_vibe_agents` (toml_edit document per agent) and `:645` `read_vibe_agents`
  are the TOML agent pattern to follow; `description_or_name` and `sanitize_name` are used there.
- `src/adapters/opencode.rs` `stale_agent_entries` — precedent for "only keys conforme writes ⇒ conforme's".
- `src/sync.rs` `instructions_rules_and_unheld` keeps in a migrated AGENTS.md the agents the output cannot
  hold (`caps.agents`): with Codex holding agents, `migrate --output codex` stops writing `## Agent:` there.
- `tests/integration.rs` `test_migrated_agents_md_keeps_only_what_the_output_cannot_hold` asserts
  `## Agent: reviewer` in AGENTS.md after `migrate --output codex`.
- `src/gitignore.rs:59` — `"codex" => vec![".agents/skills/"]`; `:65` Vibe precedent `".vibe/agents/*.toml"`.
- `src/watch.rs:204` — Codex watches `".codex"` (agents included).
- `AGENTS.md` is ~32.5 KB; Codex reads 32 KiB of it: CLAUDE.md edits must not grow it past 32768 bytes.

Upstream (verified 2026-10-09):
- Codex rust-v0.162.0 `codex-rs/agent-roles/src/loader.rs:75-78` loads `<config folder>/agents`, i.e. the
  project `.codex/agents/`, only in trusted projects (`config/src/loader/mod.rs:133-134`); `discovery.rs:23-38`
  scans it recursively for `.toml`.
- `agent-roles/src/agent_role_config.rs:20-28` — `#[serde(deny_unknown_fields)]` with `name`, `description`,
  `nickname_candidates` and the flattened `ConfigToml`; required: non-empty `name` (`:80-84`),
  `description` (`:125-128`), `developer_instructions` (`:148-151`); a bad file is skipped with a warning
  (`loader.rs:119-123`). Built-ins `default`, `explorer`, `worker` (`core/src/agent/role.rs:348-385`);
  a custom one of that name overrides it (`role.rs:168-171`). No tools allowlist (`role.rs:36-48`).
  Minimal file (`core/src/config/config_tests.rs:9006-9010`):
  `name = "reviewer"` / `description = "Review role"` / `developer_instructions = "Review carefully"`.
- SSE: Claude Code `"type": "sse"` (https://code.claude.com/docs/en/mcp); VS Code `"type": "sse"`
  (https://code.visualstudio.com/docs/copilot/reference/mcp-configuration); Devin `"transport": "sse"`
  (https://docs.devin.ai/cli/extensibility/mcp/configuration); Zoo Code `type: z.enum(["sse"])`
  (Zoo-Code v3.86.0 `src/services/mcp/McpHub.ts:112-126`); Gemini CLI `url` + `type === 'sse'`
  (v0.63.0 `packages/core/src/tools/mcp-client.ts:2239-2250`); Cursor CLI parser accepts `"sse"`, the IDE
  ignores `type` (Cursor forum, staff); Kiro supports SSE with no documented key
  (https://kiro.dev/blog/introducing-remote-mcp/); OpenCode/Kilo try StreamableHTTP then SSE on a
  `remote` url (opencode v1.18.35 `packages/opencode/src/mcp/index.ts:269-289`); Codex has only
  stdio/streamable HTTP (`codex-rs/config/src/mcp_types.rs:614-645`); Zed `Http { url, headers }` is
  streamable only (v1.23.2 `crates/context_server/src/transport/http.rs:105-110`); Vibe `http` and
  `streamable-http` are the same transport (v2.26.1 `vibe/core/tools/mcp/registry.py:309-312`).

## Non-goals
- WebSocket (`ws`) stays mapped to the HTTP variant, as today.
- No `model`, `nickname_candidates` or `[features]` in Codex agent files; no Codex agent read from a
  user-level `~/.codex/agents`.
- OpenCode/Kilo read a `remote` server back as HTTP (they have no SSE marker); documented, not modelled.
- Gemini CLI keeps `httpUrl` for streamable HTTP.

## Steps

### S-001 — Add the `Sse` transport variant, handled like `Http` everywhere
- **Files:** Modify `src/config.rs:50-59` · `src/mcp.rs` · `src/markdown.rs` · `src/validate.rs` ·
  `src/adapters/kilo.rs` · `src/adapters/vibe.rs` · any other file `cargo build` reports
- **Depends on:** none
- **Change:** add `Sse { url: String, headers: BTreeMap<String, String> }` to `McpTransport` with doc
  comment `/// A remote server on the legacy HTTP+SSE transport (MCP 2024-11-05).`. Make every
  exhaustive `match` compile by extending each `McpTransport::Http { .. }` arm to
  `McpTransport::Http { .. } | McpTransport::Sse { .. }` with the same bindings, so SSE behaves exactly
  as HTTP for now. In `src/mcp.rs` `server_strings` (`:161`) and `codex_keeps_literal` (`:182`) treat
  `Sse` as remote. Do not change any output yet.
- **Preserve:** every existing output byte for stdio and HTTP servers.
- **Verify:** `test "$(cargo test 2>&1 | grep -E '^test result' | grep -vc ' 0 failed')" = 0` → exit 0

### S-002 — Read and write SSE in the AGENTS.md convention
- **Files:** Modify `src/markdown.rs` (`build_mcp` near `:347-410`, `export_as_agents_md` near `:536-545`) ·
  Test `src/markdown.rs` tests module
- **Depends on:** S-001
- **Change:** in `build_mcp`, read a `<!-- transport: sse -->` comment (same parsing as the other
  `<!-- key: value -->` lines); with a `url:` and `transport: sse` build `McpTransport::Sse`, else
  `McpTransport::Http` as today. In `export_as_agents_md`, for `McpTransport::Sse` write the same lines as
  HTTP plus `<!-- transport: sse -->` right after the `url:` line. Add test
  `test_sse_server_round_trips_through_agents_md`: parse
  `"## MCP: legacy\n<!-- url: https://x.dev/sse -->\n<!-- transport: sse -->\n"`, assert `Sse` with that
  URL, export, parse again, assert equal.
- **Preserve:** HTTP sections without `transport:` parse exactly as before.
- **Verify:** `cargo test --lib test_sse_server_round_trips_through_agents_md 2>&1 | tail -3` → `1 passed`

### S-003 — Write SSE in every JSON shape
- **Files:** Modify `src/mcp.rs:943-1100` · Test `src/mcp.rs` tests module
- **Depends on:** S-001
- **Change:** add to `ServerShape` a field `sse: Option<SseShape>` with
  `struct SseShape { url_key: &'static str, marker: Option<(&'static str, &'static str)> }` and doc
  comment "How a legacy SSE server is written; `None`: as the HTTP variant (the tool has no SSE)".
  Set: CLAUDE, CURSOR, COPILOT, ZOOCODE `Some(SseShape { url_key: "url", marker: Some(("type", "sse")) })`;
  GEMINI `Some(SseShape { url_key: "url", marker: Some(("type", "sse")) })`; DEVIN
  `Some(SseShape { url_key: "url", marker: Some(("transport", "sse")) })`; KIRO
  `Some(SseShape { url_key: "url", marker: None })`; ZED `None`. In `build_servers_object` split the
  remote arm: `McpTransport::Sse` with `Some(sse)` writes `sse.marker` (if any) and `sse.url_key` +
  `headers`, never `shape.http_type` nor `shape.http_extra`; with `None` it is written exactly as `Http`.
  `env` follows `env_on_http` for both. Add test `test_sse_entry_shape_per_tool` that builds one SSE server
  `legacy` (`https://x.dev/sse`, header `X-Org: acme`) through each shape and asserts the JSON:
  Claude/Cursor/Copilot/Zoo `{"type":"sse","url":…,"headers":…}`, Gemini `{"type":"sse","url":…}` (no
  `httpUrl`), Devin `{"transport":"sse","url":…}`, Kiro `{"url":…}` with no `type`, Zed same as HTTP.
- **Preserve:** HTTP entries of every shape are unchanged.
- **Verify:** `cargo test --lib test_sse_entry_shape_per_tool 2>&1 | tail -3` → `1 passed`

### S-004 — Read SSE back from JSON MCP files
- **Files:** Modify `src/mcp.rs:1585-1600` (`parse_mcp_json`) · Test `src/mcp.rs` tests module
- **Depends on:** S-001
- **Change:** an entry with a URL whose `type` is `"sse"` or whose `transport` is `"sse"` becomes
  `McpTransport::Sse { url, headers }`; every other remote entry stays `McpTransport::Http`. Add test
  `test_parse_mcp_json_reads_sse`: Claude `{"type":"sse","url":"u"}`, Devin `{"transport":"sse","url":"u"}`,
  Gemini `{"type":"sse","url":"u"}` → `Sse`; `{"type":"http","url":"u"}` and `{"url":"u"}` → `Http`.
- **Preserve:** `is_expressible_server` skipping, header and env parsing.
- **Verify:** `cargo test --lib test_parse_mcp_json_reads_sse 2>&1 | tail -3` → `1 passed`

### S-005 — Warn where SSE becomes streamable HTTP
- **Files:** Modify `src/adapters/codex.rs` (`warnings`), `src/adapters/zed.rs` (`warnings`),
  `src/adapters/vibe.rs` (`warnings`) · Test `tests/roundtrip.rs`
- **Depends on:** S-001
- **Change:** in each of the three `warnings()` add, for every server with `McpTransport::Sse`, the message
  `"MCP server {name}: <Tool> has no SSE transport, so it is written as streamable HTTP and connects only if the server speaks that too"`
  with `<Tool>` = `Codex`, `Zed`, `Mistral Vibe`. Add test `test_sse_is_warned_about_where_it_cannot_be_written`
  asserting one warning from each of `CodexAdapter`, `ZedAdapter`, `VibeAdapter` and none from
  `ClaudeAdapter` for a config holding one SSE server with a plain URL.
- **Preserve:** the existing `${VAR}` warnings.
- **Verify:** `cargo test --test roundtrip test_sse_is_warned_about 2>&1 | tail -3` → `1 passed`

### S-006 — Round-trip SSE through every adapter
- **Files:** Test `tests/roundtrip.rs`
- **Depends on:** S-002, S-003, S-004
- **Change:** add `test_sse_survives_where_the_tool_can_hold_it`: for each adapter in `all_adapters()` with
  `capabilities().mcp`, write a config holding one SSE server and read it back; assert the server is
  `McpTransport::Sse` for `claude`, `cursor`, `copilot`, `zoocode`, `gemini`, `devin`, and
  `McpTransport::Http` with the same URL for `kiro`, `opencode`, `kilo`, `codex`, `zed`, `vibe`.
  (Kiro writes no marker, so it reads back as HTTP: assert that, it documents the loss.)
- **Preserve:** nothing else.
- **Verify:** `cargo test --test roundtrip test_sse_survives 2>&1 | tail -3` → `1 passed`

### S-007 — Codex agent generation and reading helpers
- **Files:** Modify `src/skills.rs` (next to `generate_vibe_agents`, `:603`) · Test `src/skills.rs` tests module
- **Depends on:** none
- **Change:** add
  `const CODEX_BUILTIN_AGENTS: &[&str] = &["default", "worker", "explorer"];`
  `pub(crate) fn is_codex_builtin_agent(name: &str) -> bool` (compares `sanitize_name(name)`);
  `pub fn generate_codex_agents(project_root: &Path, agents: &[NormalizedAgent]) -> Result<Vec<(PathBuf, String)>>`
  writing `.codex/agents/<sanitize_name(name)>.toml` for every agent that is not built in, as a
  `toml_edit::DocumentMut` with keys in this order: `name` (sanitized), `description`
  (`description_or_name`), `developer_instructions` (`agent.content.trim()`);
  `pub(crate) fn is_conforme_codex_agent_file(path: &Path) -> bool` → true when the file parses as TOML and
  every top-level key is one of `name`, `description`, `developer_instructions`;
  `pub(crate) fn read_codex_agents(project_root: &Path) -> Result<Vec<NormalizedAgent>>` scanning
  `.codex/agents/` recursively for `*.toml` (sorted), skipping a file without a non-empty `name` or
  `developer_instructions` and built-in names, mapping `name`, `description` (default empty) and
  `developer_instructions` → `content`. Tests: `test_codex_agent_file_round_trips` (generate then read gives
  the same name, description, content), `test_codex_builtin_agent_names_are_not_written`,
  `test_codex_agent_with_other_keys_is_not_conforme_shaped` (a file with `model = "gpt-5"` → false).
- **Preserve:** the Vibe helpers.
- **Verify:** `cargo test --lib -- codex_agent codex_builtin_agent 2>&1 | tail -3` → `3 passed`

### S-008 — Codex holds agents
- **Files:** Modify `src/adapters/codex.rs:29-36,48-52,91-144` · `src/gitignore.rs:59` · Test `tests/roundtrip.rs` ·
  Modify `tests/integration.rs` (`test_migrated_agents_md_keeps_only_what_the_output_cannot_hold`)
- **Depends on:** S-007
- **Change:** `capabilities()` → `agents: true`. `generate()` appends `crate::skills::generate_codex_agents`.
  `read()` passes `agents: crate::skills::read_codex_agents(project_root)?` into the config given to
  `read_native_agents_md` (tool files win over AGENTS.md sections). `managed_directories()` adds
  `ManagedDir::files_except(project_root.join(".codex").join("agents"), ".toml", |path| !crate::skills::is_conforme_codex_agent_file(path) || path.file_stem().is_some_and(|s| crate::skills::is_codex_builtin_agent(&s.to_string_lossy())))`.
  `warnings()` adds `"agent {name} is not written: a file of that name would replace Codex's built-in agent"`
  for built-in names. `src/gitignore.rs:59` → `vec![".agents/skills/", ".codex/agents/*.toml"]`.
  Tests: `test_roundtrip_codex_agents` (write `rich_config()`, read back the agent; a hand-written
  `.codex/agents/mine.toml` with an extra `model` key is not in `find_orphans`). Change the integration
  test to migrate `--output deepseek` and assert `## Agent: reviewer` and `## MCP: fs` are in AGENTS.md and
  `## Skill:` is not; add `test_migrate_to_codex_writes_codex_agents` asserting
  `.codex/agents/reviewer.toml` exists and AGENTS.md has no `## Agent:`.
- **Preserve:** `.codex/config.toml` merge; skills in `.agents/skills`.
- **Verify:** `test "$(cargo test 2>&1 | grep -E '^test result' | grep -vc ' 0 failed')" = 0` → exit 0

### S-009 — Docs for Codex agents and SSE
- **Files:** Modify `docs/providers/codex.md`, `docs/providers/claude-code.md`, `docs/providers/cursor.md`,
  `docs/providers/copilot.md`, `docs/providers/devin.md`, `docs/providers/zoocode.md`, `docs/providers/gemini.md`,
  `docs/providers/kiro.md`, `docs/providers/opencode.md`, `docs/providers/kilo.md`, `docs/providers/zed.md`,
  `docs/providers/vibe.md`, `README.md`, `src/help_ai.rs`, `CLAUDE.md`
- **Depends on:** S-008, S-006
- **Change:** codex.md: replace "conforme does not generate Codex agents yet (known gap)" with the agent
  format, trust note, built-in names and the ownership rule, each with its proof from Grounded facts.
  Each provider doc: one bullet on how SSE is written/read for that tool with its proof URL; delete the
  "a `type: "sse"` server is written as `http` (known gap)" bullets (copilot.md, zoocode.md).
  README: Codex agents in the supported-tools table; one sentence on SSE. help_ai.rs: Codex agents line;
  SSE note per JSON tool. CLAUDE.md: MCP key mapping table rows mention SSE; `codex.rs` line mentions
  `.codex/agents/*.toml`; replace text rather than add, keeping `AGENTS.md` under 32768 bytes after S-010.
- **Preserve:** every proof URL already in the docs.
- **Verify:** `test "$(cat docs/providers/codex.md docs/providers/copilot.md docs/providers/zoocode.md | grep -ciE '(agent|sse)[^.]*known gap')" = 0` → exit 0

### S-010 — Regenerate dogfooded configs and run the gate
- **Files:** generated files only
- **Depends on:** S-009
- **Change:** `cargo run --release -- sync`; then the gate.
- **Preserve:** `.conformerc.toml`.
- **Verify:** `cargo run --release -q -- check && test $(wc -c < AGENTS.md) -le 32768 && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check && python3 .claude/skills/verify-providers/scripts/skills_conformity.py . && echo GREEN` → `GREEN`

### S-011 — Live check with the Codex CLI
- **Files:** none (scratch fixture outside the repo)
- **Depends on:** S-010
- **Change:** in a temporary directory with `.codex/`, a Claude source holding agent `reviewer` and an SSE
  server, run `conforme sync`, then
  `codex -c 'projects={"<abs path>"={trust_level="trusted"}}' debug prompt-input` and check that
  `reviewer` is listed among the available roles; report the result in the PR.
- **Preserve:** the user's own Codex config (pass the trust inline only).
- **Verify:** `codex -c 'projects={"<abs path>"={trust_level="trusted"}}' debug prompt-input | grep -c reviewer` → `≥ 1`
