# Antigravity CLI

> Google's Antigravity command-line agent (`agy`). Source: `--from antigravity`

**Last verified online:** 2026-10-09, against Antigravity CLI 1.3.2 (https://github.com/google-antigravity/antigravity-cli/releases)

## Official docs

- Rules: https://antigravity.google/docs/rules
- Skills: https://antigravity.google/docs/skills
- Subagents: https://antigravity.google/docs/subagents
- MCP: https://antigravity.google/docs/mcp
- Gemini CLI migration (what moves where): https://antigravity.google/docs/cli/gcli-migration
- Releases and changelog: https://github.com/google-antigravity/antigravity-cli/releases

The CLI is closed source, so these pages and the release changelog are the upstream references. Changelog entries are cited by version.

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md`, else `GEMINI.md` | Markdown; "do not use frontmatter … (always_on)" (https://antigravity.google/docs/rules) |
| Rules | `.agents/rules/<name>.md` | Frontmatter `trigger` (`always_on`, `model_decision`, `glob` or `manual`), `description`, `globs` (comma-separated). "Antigravity scans only immediate .md children"; a file without frontmatter or with a wrong trigger is silently discarded; 24,000 bytes per file (https://antigravity.google/docs/rules) |
| Skills | `.agents/skills/<folder>/SKILL.md` | Frontmatter `name` (optional, defaults to the folder) and `description` (required); no manual-invocation flag. `.gemini/skills/` is not read (https://antigravity.google/docs/skills, https://antigravity.google/docs/cli/gcli-migration) |
| Subagents | `.agents/agents/<name>.md` (or `<name>/agent.md`) | Frontmatter `name` and `description` (both required), `tools`, `model` ("inherit, flash, or pro"). Built-in subagents: `research`, `browser`, `self`, `image-generator` (https://antigravity.google/docs/subagents; changelog 1.2.16) |
| MCP | `.agents/mcp_config.json` | One `mcpServers` object. Local: `command`, `args`, `env`, `cwd`. Remote: `serverUrl` ("URL for remote Streamable HTTP or SSE servers") and `headers`, no `type`. "Legacy fields like url or httpUrl aren't supported". JSONC accepted (changelog 1.1.24). `${VAR}` expansion undocumented (https://antigravity.google/docs/mcp) |

Detection signals (changelog 1.1.16): `.agents/mcp_config.json`, `.agents/rules/`, `.agents/agents/`, `.antigravityignore`.

## Activation modes

| Mode | Frontmatter on the rule file |
|------|------------------------------|
| Always | `trigger: always_on` |
| Glob | `trigger: glob` + `globs:` (comma-separated) |
| Agent Decision | `trigger: model_decision` + `description:` |
| Manual | `trigger: manual` |

## conforme adapter

- File: `src/adapters/antigravity.rs`
- ID: `antigravity`, name `Antigravity CLI`
- Capabilities: activation modes, skills, subagents, MCP
- Detected by `.agents/mcp_config.json`, a `.agents/rules/` or `.agents/agents/` directory, or `.antigravityignore`. `.agents/skills/` alone does not detect it, because Codex and Zed write that folder too.
- Instructions: read from `AGENTS.md`, else `GEMINI.md`. As a target, the instructions reach Antigravity through the generated root `AGENTS.md`; this target writes no instruction file. With Antigravity as the source, that file stays the source's own: no target writes it, and `sync` does not regenerate it.
- Rules: one file per rule in `.agents/rules/`. `read()` reads only the top-level `.md` files and skips any file without a `trigger`.
- Skills: written to the shared `.agents/skills/` and read back from it. A manual skill is written with a warning, since Antigravity has no manual-only skill.
- Subagents: `.agents/agents/<name>.md` with `name`, `description` and `model`; `model` is kept only as `inherit`, `flash` or `pro`. No `tools` field is written. A built-in name is never written, and a user file with one of those names is neither read nor cleaned.
- MCP: merged under `mcpServers` through `json_settings`, so comments and keys conforme does not own survive, and an unparsable file is refused rather than overwritten. Local servers keep `env`; remote servers get `serverUrl` and `headers`, with no `env` and no `type`. HTTP and SSE share that key, so an SSE server is written the same way and reads back as streamable HTTP.
- Warnings: a manual skill, a built-in subagent name, and any `${` in a server ("Antigravity documents no ${VAR} expansion"); the value may be sent as written.
- Migrating to Antigravity writes `AGENTS.md`, since that is where its instructions live.
- Limitation with Antigravity as the source: the rules in `.agents/rules/` do not reach the tools that read only `AGENTS.md` (Codex, OpenCode, Kilo, Vibe, DeepSeek), because `AGENTS.md` is the source's own file and `sync` never rewrites it. `sync` and `check` name each such tool and the rules it misses; add those rules to `AGENTS.md` as `## Rule:` sections to share them.
- Migrating from Antigravity to one of those tools adds the `.agents/rules/` rules to `AGENTS.md` as `## Rule:` sections before removing the rule files, so none is lost.

## Unconfirmed points

- **`${VAR}` in `.agents/mcp_config.json`.** The docs do not document expansion. conforme writes `${VAR}` as written and warns. The live check below could not prove expansion, so the warning stays.
- **Trust.** Not confirmed whether Antigravity loads a project's `.agents/` rules, skills, subagents and MCP servers only in a trusted folder, as Gemini CLI and Mistral Vibe do. The live check below did not settle it.

## Live check

Run 2026-10-09 with `agy --version` → `1.3.2`, in a temporary project synced from a Claude source (a skill, a subagent, a stdio server with `TOKEN=${TOKEN}`, a remote server with `Authorization: Bearer ${TOKEN}`) by `conforme sync` with `only = ["antigravity"]`. `agy` was already logged in on this machine and none of these commands needed it; nothing was logged in or out. The commands ran with a temporary `HOME` as well as the real one: `agy` honours `HOME` (it creates its `~/.gemini/` state under it), and both gave the same output.

| Command | Result |
|---|---|
| `agy agents` | exit 0, empty list. The generated `.agents/agents/reviewer.md` is not listed. |
| `agy mcp list` | exit 0, `No MCP servers configured.` The generated `.agents/mcp_config.json` (`local`, `remote`) is not listed. |
| skills | `agy` has no command that lists skills; not observable without a model prompt, which needs a login and a model call. |

Reading: `agy mcp list` and `agy agents` show nothing from the project's `.agents/`, so either they only list user-level (`~/.gemini/`) configuration or the project is not trusted. The CLI documents no flag to tell which. This is **not proven** either way, and neither is `${TOKEN}` expansion: the only way to see it is to run a prompt that starts the server, which this check does not do. The `${VAR}` warning in `src/adapters/antigravity.rs` is kept.

## Notes

- Antigravity always loads a root `AGENTS.md` (and `GEMINI.md`) whole, without activation modes, next to `.agents/rules/`, so in a project that keeps an `AGENTS.md` with rule sections its rules reach Antigravity twice and glob/manual rules are always on there. This is the same trade-off as Kiro, Cursor, Devin and Zoo Code: the generated `AGENTS.md` keeps its sections for the tools that read only `AGENTS.md` (Codex, OpenCode, …), and when `AGENTS.md` is the source's own file (`migrate --source codex --output antigravity`) it is left as it is.
- Antigravity also reads `AGENTS.md` or `GEMINI.md` under `.agents/` (https://antigravity.google/docs/rules); conforme reads only the root files.
- `.agents/skills/` is shared with Codex and Zed. `remove` and `migrate` never delete a file another kept tool generates there.
- Not handled: workflows, hooks, plugins, `.agents/*.json` manifests, nested rule folders listed in `.agents/rules.json`, and the `<name>/agent.md` subagent layout on write.
- The Gemini CLI target is unchanged; see [gemini.md](gemini.md).
