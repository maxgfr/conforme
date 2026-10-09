# Claude Code

> Anthropic's CLI agent. Source: `--from claude`

**Last verified online:** 2026-10-09, against Claude Code 2.1.295 (https://code.claude.com/docs/en/changelog)

## Official docs

- Overview: https://code.claude.com/docs/en/overview
- Rules (CLAUDE.md + .claude/rules/): https://code.claude.com/docs/en/memory
- Skills: https://code.claude.com/docs/en/skills
- Subagents: https://code.claude.com/docs/en/sub-agents
- MCP servers: https://code.claude.com/docs/en/mcp
- Hooks: https://code.claude.com/docs/en/hooks
- Hooks guide: https://code.claude.com/docs/en/hooks-guide
- Settings: https://code.claude.com/docs/en/settings
- Changelog: https://code.claude.com/docs/en/changelog
- Settings reference (every key: `enableAllProjectMcpServers`, `claudeMdExcludes`, `pluginConfigs`): https://code.claude.com/docs/en/settings-reference
- `.claude/` directory map (every file and what reads it): https://code.claude.com/docs/en/claude-directory
- Debug your config (`/context`, `claude doctor`, `.mcp.json` approval): https://code.claude.com/docs/en/debug-your-config
- CLI reference (`claude doctor`, `--setting-sources`, `--mcp-config`): https://code.claude.com/docs/en/cli-reference
- Commands (`/memory`, `/skills`, `/mcp`, `/import`): https://code.claude.com/docs/en/commands
- Permissions and workspace trust: https://code.claude.com/docs/en/permissions

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions | `CLAUDE.md` | Markdown (always-active rules inlined) |
| Rules (glob) | `.claude/rules/*.md` | YAML frontmatter: `paths` (glob list, or a comma-separated string) |
| Skills | `.claude/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description`, `allowed-tools` |
| Commands | `.claude/commands/*.md` | YAML frontmatter: `description`, `allowed-tools`, `model` |
| Agents | `.claude/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model`, `tools`, `color`, `permissionMode` |
| MCP | `.mcp.json` (merged) | JSON: `{ "mcpServers": { "<name>": { "type": "stdio", "command", "args" } } }` |
| Hooks | `.claude/settings.json` | JSON: `{ "hooks": { "PreToolUse": [...], "PostToolUse": [...] } }` |
| Settings | `.claude/settings.json` | JSON: `{ "permissions": { "allow": [...], "deny": [...] }, "model": "sonnet" }` |

## Activation modes

| Mode | Implementation |
|------|---------------|
| Always | No frontmatter, content in CLAUDE.md |
| GlobMatch | `.claude/rules/<name>.md` with `paths: [**/*.ts]` |
| AgentDecision | `.claude/rules/<name>.md` without `paths` (always loaded) |
| Manual | Same as AgentDecision (no native manual mode) |

## conforme adapter

- File: `src/adapters/claude.rs`
- ID: `claude`
- Capabilities: rules, skills, agents, MCP
- Read: CLAUDE.md + .claude/rules/ + .claude/skills/ + .claude/commands/ + .claude/agents/ + .mcp.json (and both `AGENTS.md` and `.claude/AGENTS.md`, root first, when the project has no `CLAUDE.md` nor `.claude/CLAUDE.md`)
- Write: CLAUDE.md + .claude/rules/ + .claude/skills/ + .claude/agents/ + .mcp.json

## Notes

- Claude Code keeps extending both frontmatter blocks beyond what conforme syncs. Subagents now also accept `disallowedTools`, `maxTurns`, `skills`, `mcpServers`, `hooks`, `memory`, `background`, `effort`, `isolation`, `initialPrompt`, `omitClaudeMd` and `experimental`; skills also accept `when_to_use`, `disallowed-tools`, `argument-hint`, `arguments`, `user-invocable`, `context`, `agent`, `effort`, `paths`, `shell`, `model`, `background`, `hooks`, `license` and `compatibility` (and `metadata`, which conforme writes for OpenCode). These are Claude-specific and have no cross-tool equivalent, so conforme neither emits nor maps them — it writes the portable subset and leaves hand-authored extras alone

- Commands (`.claude/commands/**/*.md`) are read as skills when Claude is source, propagated to other tools as SKILL.md. A nested command is namespaced by its folders, as Claude Code invokes it: `frontend/component.md` is the skill `frontend:component` (written elsewhere as `frontend-component`). A skill and a command with the same name read as the skill, the one Claude Code runs
- Hooks and permissions are Claude-specific, not synced to other tools
- `allowed-tools` accepts a space- or comma-separated string (or a YAML list); conforme writes the space-separated form `"Read Bash Write"` and parses both on read
- Rules without `paths` frontmatter are always-active (no agent-decision/manual distinction)
- `paths` accepts a YAML list or a comma-separated string; conforme reads both and writes a list
- `.claude/rules/` is discovered **recursively**, so rules may be organised under `frontend/`, `backend/`, … conforme reads nested rules too (they are written back flat, one file per rule name)
- `.claude/agents/` is scanned **recursively** as well; conforme reads nested agents and writes them back flat
- An agent file without `name` is documentation kept beside the agents ("Claude Code treats the file as documentation"), and one without `description` is skipped: conforme reads neither as an agent, and orphan cleanup never deletes a `.md` there that has no `name` (a README survives a sync)
- Subagent `tools` must resolve to Claude Code tools, or Claude Code refuses to launch the subagent ("would be spawned with zero tools"). Names from other hosts are translated (`read_file` → `Read`, `run_shell_command` → `Bash`, `codebase` → `Grep`, Gemini `mcp_github_list_issues` / Kiro `@github/list_issues` → `mcp__github__list_issues`, …), `Agent(type)` restrictions pass through, Kiro's `*` / `@builtin` become no `tools` (every tool), and a restricted list in which nothing translates becomes `tools: Read` rather than every tool
- Subagent `model` is an alias (`sonnet`, `opus`, `haiku`, `fable`), `inherit`, or a `claude-*` id; another vendor's id (`gpt-4o`) and a dotted id from another host (Kiro's `claude-sonnet-4.5`) are left out
- Boolean frontmatter fields accept `yes`/`no`, `on`/`off` and `1`/`0` besides `true`/`false`; conforme reads all of them (e.g. `disable-model-invocation: yes`)
- Since 2.1.277 (2026-09-18) Claude Code reads `AGENTS.md` and `.claude/AGENTS.md` itself (every one present) when the project has no `CLAUDE.md`, `.claude/CLAUDE.md` nor `CLAUDE.local.md`. In that case conforme's `read()` takes both too, concatenated root first, with the AGENTS.md convention (their `## Rule:` sections join `.claude/rules/`, whose files win on a name clash), and with Claude as the source those files are never regenerated, written by another target, deleted by `remove`/`migrate` nor gitignored (`reads_agents_md`, `source_files`). As a target conforme writes `CLAUDE.md` when there is content, after which Claude Code reads `CLAUDE.md` instead
- A personal `CLAUDE.local.md` does not turn that fallback off in conforme (deliberate deviation): Claude Code itself then skips `AGENTS.md`, but conforme never reads `CLAUDE.local.md` and keeps the shared `AGENTS.md` as the source rather than regenerating it
- The **Project instructions** setting (`/config`, or `pluginConfigs."cc-plugin-agents-md@builtin".options.instructionFiles`) chooses `claude-md-or-agents-md` (the default described above), `claude-md-and-agents-md`, `claude-md` or `managed-only` ("Only your organization's managed `CLAUDE.md` and auto memory at launch"). Claude Code reads it only from user, `--settings` or managed settings, never from project files, so conforme assumes the default (https://code.claude.com/docs/en/memory)
- Claude Code also counts a `CLAUDE.md` in a parent directory; conforme checks only the project root (known gap)
- A Claude source with only `.claude/AGENTS.md` (no root `AGENTS.md`) gives the tools that read `AGENTS.md` natively (Codex, OpenCode, DeepSeek, Mistral Vibe, Kilo Code) no instructions: `AGENTS.md` is not generated, since the source reads `AGENTS.md` itself (known gap)
- A skill or agent is always written with a `description`, falling back to its name when the source has none (other tools skip entries without one)
- Agent `name` may be up to 256 characters upstream; conforme's `sanitize_name` keeps every tool's names at 64 or fewer
- Claude Code skips skills named `synced` or `anthropic-skills` (and, outside a plugin, any name starting with `anthropic-skills:`); conforme's validation warns about such a skill. `claude-ai`, reserved in 2.1.282 only, loads again since 2.1.283
- A project `CLAUDE.md` may live at `./CLAUDE.md` **or** `./.claude/CLAUDE.md`. conforme prefers the root file, and falls back to `./.claude/CLAUDE.md` when only that one exists — for both reading and writing, so a project using the nested location is neither read as empty nor given a competing second instruction file
- MCP: `.mcp.json` is merged, not owned (`is_shared_file`): per-server keys conforme never writes (`oauth`, `headersHelper`, `timeout`, `alwaysLoad`) survive a sync, and `remove claude` / `migrate --source claude` keep the file (Copilot CLI reads it too). Environment references are `${VAR}` / `${VAR:-default}`, conforme's normalized spelling; `env` is written on stdio servers only
- MCP: in-process `type: "sdk"` servers have neither a command nor a URL; conforme skips them on read and keeps them untouched on write
- MCP: `type: "stdio"` is optional in `.mcp.json` (transport is inferred from `command`). HTTP transport accepts `"http"` (and the `"streamable-http"` alias); the older `"sse"` transport is deprecated, but conforme writes a legacy SSE server as `type: "sse"` with `url` and `headers` and reads `"sse"` back as SSE (https://code.claude.com/docs/en/mcp); the `"ws"` (WebSocket) transport is parsed as HTTP (`url` + `headers`)
- `tools` (subagents) and `allowed-tools` (skills/commands) accept a space-separated string, a comma-separated string, or a YAML list; conforme parses all three forms on read
- Subagent `color` (`red`/`blue`/`green`/`yellow`/`purple`/`orange`/`pink`/`cyan`) and `permissionMode` (`default`/`acceptEdits`/`auto`/`dontAsk`/`plan`/`manual`/`bypassPermissions`) are preserved on the Claude read→write round-trip (and carried through AGENTS.md as `<!-- color: -->` / `<!-- permission-mode: -->` comments); they are Claude-specific and not mapped to other tools

## Manual skill invocation

Manual skills preserve `disable-model-invocation: true`, `metadata.opencode/autoinvoke: "false"`, and Codex `agents/openai.yaml` with `policy.allow_implicit_invocation: false` through synchronization. In AGENTS.md, use `<!-- invocation: manual -->` in the skill section. conforme writes `metadata.opencode/autoinvoke` as a hint that OpenCode is not known to read (no reader exists on its `dev` branch), so OpenCode needs the corresponding `permission.skill` deny entries; skill synchronization does not change user permissions.
