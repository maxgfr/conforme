# Kilo Code

> Open-source coding agent, a fork of OpenCode: the `kilo` CLI and the VS Code extension built on it read the same project files. Source: `--from kilo`

**Last verified online:** 2026-10-09, against Kilo Code 7.8.8 (https://github.com/Kilo-Org/kilocode/releases)

## Official docs

- CLI: https://kilo.ai/docs/code-with-ai/platforms/cli
- AGENTS.md: https://kilo.ai/docs/customize/agents-md
- Custom rules: https://kilo.ai/docs/customize/custom-rules
- Custom instructions: https://kilo.ai/docs/customize/custom-instructions
- Skills: https://kilo.ai/docs/customize/skills
- Custom subagents: https://kilo.ai/docs/customize/custom-subagents
- Custom modes: https://kilo.ai/docs/customize/custom-modes
- MCP in the CLI: https://kilo.ai/docs/automate/mcp/using-in-cli
- MCP in the extension: https://kilo.ai/docs/automate/mcp/using-in-kilo-code
- Settings: https://kilo.ai/docs/getting-started/settings
- Repository: https://github.com/Kilo-Org/kilocode
- CLI reference (`debug config`, `debug skill`, `config check`, `agent list`): https://kilo.ai/docs/code-with-ai/platforms/cli-reference
- CLI runtime (config merge order, `kilo`/`opencode` files, `.kilo` and `.kilocode`): https://kilo.ai/docs/contributing/architecture/cli-runtime
- Built-in agents (code, ask, plan, debug; orchestrator deprecated): https://kilo.ai/docs/code-with-ai/agents/using-agents
- Auto-approving actions (project `.kilo/kilo.jsonc`, MCP permission keys): https://kilo.ai/docs/getting-started/settings/auto-approving-actions
- Agent permissions (`permission` frontmatter): https://kilo.ai/docs/customize/agent-permissions

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `AGENTS.md` (native), else `CLAUDE.md`, else `CONTEXT.md` | Markdown |
| Rules | `.kilo/rules/*.md`, legacy `.kilocode/rules/*.md` | Plain Markdown, every file loaded |
| Skills | `.kilo/skills/<name>/SKILL.md` (also `.kilo/skill/`, `.kilocode/`, `.claude/skills/`, `.agents/skills/`) | YAML frontmatter: `name` (required), `description` |
| Agents | `.kilo/agents/<name>.md` (also `.kilo/agent/`, `.kilocode/`) and the `agent` key of the config | OpenCode format: `description`, `mode`, `model` (`provider/model`) |
| MCP | `mcp` key of `kilo.json(c)` then `opencode.json(c)` at the root, then in `.kilocode/` and `.kilo/` (`ALL_CONFIG_FILES`, `packages/opencode/src/kilocode/config/config.ts`); later files win | OpenCode shape: `type: local/remote`, `command` array, `environment`, `url`, `headers` |

Kilo's docs name the project file `.kilo/kilo.jsonc` (auto-approving actions) or `.kilo/kilo.json` (CLI runtime); it reads both, and `.kilo/` wins over the root files. Its Orchestrator agent is deprecated but still built in. It no longer reads `.opencode/` directories.

## Activation modes

None: every rule file and the instruction file are always loaded.

## conforme adapter

- File: `src/adapters/kilo.rs`
- ID: `kilo`
- Capabilities: skills, agents, MCP
- Detected by `.kilo/`, `.kilocode/`, `kilo.json` or `kilo.jsonc`
- Reads `AGENTS.md` natively (else `CLAUDE.md`, else `CONTEXT.md`): with Kilo as the source that file is the source, never regenerated, and the rule directories it reads are its `source_files()` too
- Writes skills to `.kilo/skills/<name>/SKILL.md` (with bundled files and the `.conforme` marker) and subagents to `.kilo/agents/<name>.md` (`mode: subagent`, `model` only as `provider/model`)
- An agent named like a Kilo built-in (`code`, `build`, which Kilo maps to Code, `plan`, `ask`, `debug`, `orchestrator`, `general`, `explore`, `compaction`, `title`, `summary`, and `scout` behind Kilo's `experimentalScout` flag) is neither written nor read: `.kilo/agents/code.md` tunes Kilo's own Code agent and is the user's (kept by orphan cleanup)
- MCP is merged into the highest-precedence existing file of `.kilo/kilo.jsonc`, `.kilo/kilo.json`, `.kilocode/kilo.jsonc`, `.kilocode/kilo.json`, `kilo.jsonc`, `kilo.json`, else a new `.kilo/kilo.jsonc`, through `json_settings` (JSONC comments and the user's settings kept); `read()` merges the servers of every file Kilo loads, `opencode.json(c)` at the root and in `.kilo/`/`.kilocode/` included, later ones winning
- `read()` round-trips skills, agents (markdown and `agent` key), MCP, the instruction file and the rule files (as always-on rules named after the file)

## Notes

- **No variable reference in a project config.** Kilo refuses a project config holding `{env:VAR}` (`kilo config check`: "environment references are not allowed in project config"), and drops a server with one in its headers. conforme therefore writes no reference: an `env` entry that only passes a variable through (`TOKEN=${TOKEN}`) is left out, since a local server inherits Kilo's environment, and any other `${VAR}` (a bearer header, for example) is written as is, which Kilo sends literally. `sync` and `migrate` warn for each such server. Kilo deep-merges the project config over the global one, so a server of the same name in `~/.config/kilo/kilo.jsonc` gets the project's literal header; declare it there under another name, where `{env:VAR}` is allowed
- **Root `opencode.json`.** Kilo also loads the root `opencode.json(c)`, so an OpenCode config there that holds `{env:VAR}` makes Kilo refuse the whole project config. conforme's OpenCode target writes a new config to `.opencode/opencode.json`, which Kilo never reads, so both tools work side by side; for a root `opencode.json` that already holds references, `sync` and `migrate` warn
- Skill frontmatter has no manual-invocation key in Kilo; `disable-model-invocation` is written for the other readers and is harmless
- Kilo also discovers skills in `.claude/skills/` and `.agents/skills/`: when Claude Code or Codex is synced too, the same skill exists in several roots, and Kilo keeps one copy per name (`kilo debug skill` lists each once, checked with 7.8.8). Kilo ships a built-in `kilo-config` skill that a synced skill of that name would override
- Kilo loads the legacy `.kilocoderules`, `.kilocoderules-<mode>` and the mode rule directories `.kilo/rules-<mode>/` and `.kilocode/rules-<mode>/` (`packages/opencode/src/kilocode/rules-migrator.ts`); conforme reads none of them (known gap)
- A local MCP server accepts `env` as an alias of `environment` but has no `cwd` (`packages/core/src/v1/config/mcp.ts`); conforme writes `environment` and no `cwd`
- A legacy SSE server is written as `type: "remote"`: OpenCode and Kilo Code try streamable HTTP and then SSE on a `remote` URL (opencode v1.18.35 `packages/opencode/src/mcp/index.ts:269-289`), and the server reads back as streamable HTTP, since there is no SSE marker (documented, not modelled)

## Live check

Without a model call, in a project and with an isolated `HOME`/`XDG_*`:

```bash
kilo config check    # exit 1 and the file name when a project config is refused
kilo debug config    # resolved config: the `mcp` and `agent` entries
kilo debug skill     # every skill with its location
kilo agent list      # agents, `reviewer (subagent)` for a synced one
```

`kilo mcp list` connects to every server; avoid it in checks.
