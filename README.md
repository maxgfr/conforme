# conforme

Sync your AI coding config from any tool to 13 others, or switch a project from one tool to another. Write once, apply everywhere.

AGENTS.md is governed by the [Agentic AI Foundation](https://www.linuxfoundation.org/press/linux-foundation-announces-the-formation-of-the-agentic-ai-foundation) (Linux Foundation) with 146+ member organizations including Anthropic, OpenAI, Google, AWS, and Microsoft.

## Install

```bash
# Homebrew (macOS & Linux)
brew install maxgfr/tap/conforme

# From source
cargo install --path .

# Pre-built binaries (GitHub Releases)
# Available for: macOS (ARM64, x64), Linux (ARM64, x64), Windows (ARM64, x64)
# Download from https://github.com/maxgfr/conforme/releases
```

## How it works

1. **Write your config** in your preferred tool (Claude Code, Cursor, Devin, etc.) or directly in `AGENTS.md`
2. **Run `conforme sync`** — it reads from your chosen source and propagates to all detected tools (filtered by `only` / `exclude`). `check`, `diff` and `status` use the same set of tools, so an excluded tool is never reported out of sync, and they compare the generated `AGENTS.md` too; `check` also validates the config
3. **Only changed files are updated** — content is compared using SHA-256 hashes, so unchanged files are never touched
4. **Orphan files are cleaned** — when you rename or remove a rule, the old generated files are automatically deleted. Only files of the kind conforme writes are touched: files a tool accepts but conforme never generates (Kiro `.json` agents, flat DeepSeek skills, hand-written `.md` Copilot agents) are left alone. A source that reads back empty (no instructions, rules, skills, agents or MCP servers) warns "nothing to sync" and writes and cleans nothing. A skill removed from the source is deleted from every tool's copy (conforme marks the copies it generates with a `.conforme` file; an unmarked skill folder is the user's and stays), and `check` reports what `sync` would remove (as does an agent removed from the source in `opencode.json`). `migrate` validates the source and refuses an empty one, removes the source tool's files under the same rule, and never a file or directory the output or another detected tool still uses (`.agents/skills/` shared by Codex and Zed included), a skill folder holding a file conforme cannot carry (not text), nor skills or agents the output cannot hold. Migrating to a tool that keeps its instructions in `AGENTS.md` (Codex, OpenCode, DeepSeek, Mistral Vibe, Kilo Code, Antigravity CLI, or Gemini CLI loading only `AGENTS.md`) writes `AGENTS.md`, and an existing `AGENTS.md` that differs and that the source does not read is refused before anything changes
5. **Shared settings are merged, never replaced** — settings and MCP files that also hold your own configuration (`.mcp.json`, `.cursor/mcp.json`, `.kiro/settings/mcp.json`, `.devin/mcp_config.json`, `opencode.json`, `.zed/settings.json`, `.gemini/settings.json`, `.vscode/mcp.json`, `.github/mcp.json`, `.roo/mcp.json`, `kilo.jsonc`, `.codex/config.toml`, `.vibe/config.toml`, `.agents/mcp_config.json`) only have conforme's key updated. JSONC comments and trailing commas are kept, per-server options a tool added (Kiro `autoApprove`, Zoo Code approvals, Gemini `trust`, …) survive, a file conforme cannot parse is never overwritten, and `remove`/`migrate` never delete one
6. **Each tool gets values it accepts** — agent tool names are translated into each tool's vocabulary (Claude Code, Gemini CLI and Kiro reject or ignore names they do not know), a model id another tool cannot use is left out, skill and agent names are written as kebab-case ASCII (rules, which only become files, keep other letters), and environment-variable references in MCP configs are rewritten to each tool's syntax (`${VAR}`, `${env:VAR}`, `{env:VAR}`)
7. **The source's own files stay the source's** — what the source reads outside its own directories (an `AGENTS.md` or `CLAUDE.md` it reads natively or as a fallback, Gemini CLI's context files, Devin's `global_rules.md` and `.windsurfrules`, Zoo Code's `.roorules`, DeepSeek's `.agents/skills` fallback) is never written by another tool's target, deleted by `remove` or `migrate`, nor ignored by `gitignore install`, which also skips tools left out by `only` / `exclude`

You can set your source tool once in `.conformerc.toml` or pass it on the command line with `--from`. If no source is specified, conforme defaults to `AGENTS.md`.

## Supported tools (14)

### Tools with per-rule config files

| Tool | Config format | Frontmatter | AGENTS.md |
|------|--------------|-------------|-----------|
| Claude Code | `CLAUDE.md` + `.claude/rules/**/*.md` | `paths` (glob list or comma-separated string) | Native (`AGENTS.md` and `.claude/AGENTS.md`) when no `CLAUDE.md` exists |
| Cursor | `.cursor/rules/*.mdc` | `alwaysApply`, `globs`, `description` | Native |
| Devin Desktop (formerly Windsurf) | `.devin/rules/*.md` (legacy `.windsurf/rules/*.md` is read too) | `trigger`, `description`, `globs` | Native |
| GitHub Copilot | `.github/copilot-instructions.md` + `.github/instructions/**/*.instructions.md` | `applyTo`, `excludeAgent` | Native |
| Kiro (AWS) | `.kiro/steering/*.md` | `inclusion`, `fileMatchPattern`, `name`, `description` | Native |
| Zoo Code (Roo Code fork) | `.roo/rules/*.md` | None (plain Markdown) | Native |
| Antigravity CLI | `.agents/rules/*.md` | `trigger`, `description`, `globs` | Native (`AGENTS.md`, else `GEMINI.md`) |

### Tools that read AGENTS.md natively (single-file sync)

| Tool | Primary file | Notes |
|------|-------------|-------|
| OpenAI Codex CLI | `AGENTS.md` | Also supports `AGENTS.override.md` (personal; not used as a conforme source) |
| OpenCode | `AGENTS.md` | Falls back to `CLAUDE.md`, then the deprecated `CONTEXT.md` |
| Gemini CLI | `GEMINI.md` | Reads AGENTS.md when `context.fileName` in `.gemini/settings.json` names it; conforme writes the first other `context.fileName` entry (none when it names only `AGENTS.md`) |
| Zed AI | `.rules` | First match wins: `.rules` → `.cursorrules` → `.windsurfrules` → `.clinerules` → `.github/copilot-instructions.md` → `AGENT.md` → `AGENTS.md` → `CLAUDE.md` → `GEMINI.md` |
| DeepSeek Harness (`dsh`) | `AGENTS.md` | Also loads `CLAUDE.md` when present; skills in `.dsh/skills/` |
| Mistral Vibe | `AGENTS.md` | Project files load only in a trusted folder (`~/.vibe/trusted_folders.toml`) |
| Kilo Code | `AGENTS.md` | Falls back to `CLAUDE.md`, then `CONTEXT.md`; also loads `.kilo/rules/*.md` (and the legacy `.kilocode/`) |

## Quick start

```bash
conforme init                        # Initialize and sync
conforme sync                        # Sync source to all tools
conforme sync --from claude          # Use Claude Code as source
conforme sync --dry-run              # Preview changes with diffs
conforme diff                        # Show what would change
conforme check                       # CI check (exit 1 if out of sync)
conforme status                      # Show tools and sync state
conforme add rule "Name" --activation "glob **/*.ts"
conforme watch                       # Auto-sync on file changes
conforme remove cursor,devin         # Remove generated files
conforme migrate --source gemini --output opencode  # Migrate between tools
conforme hook install                # Git pre-commit hook
conforme gitignore install           # Ignore generated outputs (merged settings stay tracked)
conforme help-ai                     # Show tool format details
```

## Switch from one tool to another

```bash
conforme migrate --source cursor --output claude
```

`migrate` reads the project in the source tool's format, writes it in the output tool's, then removes the source tool's files. What the output can hold comes across: instructions and rules (with their glob scope where the output has activation modes), skills with every bundled file, manual invocation, agents, and MCP servers with their environment-variable references respelled for the output. What it cannot hold stays where it is (a tool without agents leaves the source's agents in place), and nothing the output or another detected tool still reads is deleted.

Switch with `migrate`, not by editing `source`: the `AGENTS.md` that `sync` generates names its source on its first line, and `sync` and `check` refuse a source changed by hand since then (the new source would rewrite every tool, the old one included, and delete what it does not hold). `migrate` moves that line to its output, so setting `source` to the output afterwards is accepted.

Every pair of tools is tested both ways (`tests/migrate_matrix.rs`), including a round trip through all 14 tools and back. Each output is also checked with the tool's own CLI where one exists (Claude Code, Codex, OpenCode, Gemini CLI, Copilot CLI, Cursor, Mistral Vibe, Kilo Code): the CLI lists the migrated MCP servers, skills and agents.

## `.conformerc.toml` configuration

Create a `.conformerc.toml` at your project root to customize conforme's behavior:

```toml
# Source tool — conforme reads config from here
source = "claude"

# Only sync to these tools (default: all detected)
only = ["cursor", "copilot", "devin"]

# Exclude these tools
exclude = ["zed", "kilo"]

# Auto-generate AGENTS.md from source (default: true; never applies when the
# source reads AGENTS.md itself: codex, opencode, deepseek, vibe, kilo, antigravity; claude when the
# project has no CLAUDE.md; gemini when context.fileName names AGENTS.md)
generate_agents_md = true

# Clean orphan files on sync (default: true)
clean = true
```

When `source` is set, conforme reads your rules, skills, agents, and MCP servers from that tool's config files instead of `AGENTS.md`. This means you can author your config in whichever tool you prefer and have it propagated everywhere else.

The `AGENTS.md` generated from a tool source holds only the instructions and `## Rule:` sections. Codex, OpenCode, Kilo, Vibe, DeepSeek and most other tools load that file whole as instructions, so a skill, agent or MCP section there would sit in every conversation, a manual skill included; each tool gets those in its own files instead. `migrate` also writes the skills, agents or MCP servers the output cannot hold (agents for Codex, agents and MCP for DeepSeek) there, so a later switch still has them. Codex reads only the first 32 KiB of `AGENTS.md` (`project_doc_max_bytes`): `sync` and `migrate` warn when it is larger.

`conforme remove <tools>` deletes the files conforme generated for those tools (merged settings files stay) and the directories it leaves empty. A tool still detected afterwards (its `mcp.json` stays, for example) gets its files back on the next sync: add it to `exclude` to stop syncing it, as `remove` says.

## AGENTS.md format

conforme uses `## Rule:` headings with HTML comments to define rules and their activation:

```markdown
# Project Instructions

General instructions that apply everywhere.

## Rule: TypeScript Conventions
<!-- activation: glob **/*.ts,**/*.tsx -->

- Use strict TypeScript
- Prefer interfaces over type aliases

## Rule: Security Review
<!-- activation: agent-decision -->
<!-- description: Apply when reviewing security-sensitive code -->

- Check for XSS vulnerabilities
- Validate all user inputs

## Rule: Always Apply
<!-- activation: always -->

- Keep functions under 50 lines
```

### Activation modes

conforme normalizes 4 activation modes across all tools that support them:

| Mode | AGENTS.md | Claude | Cursor | Devin | Antigravity | Copilot | Kiro |
|------|-----------|--------|--------|----------|-------------|---------|------|
| Always | `<!-- activation: always -->` | no frontmatter (in CLAUDE.md) | `alwaysApply: true` | `trigger: always_on` | `trigger: always_on` | in main file | `inclusion: always` |
| Glob | `<!-- activation: glob **/*.ts -->` | `paths: [**/*.ts]` | `globs: "**/*.ts"` | `trigger: glob` + `globs:` | `trigger: glob` + `globs:` | `applyTo: "**/*.ts"` | `inclusion: fileMatch` + `fileMatchPattern:` |
| Agent Decision | `<!-- activation: agent-decision -->` | no frontmatter (.claude/rules/) | `description: "..."` | `trigger: model_decision` | `trigger: model_decision` + `description:` | in main file | `inclusion: auto` |
| Manual | `<!-- activation: manual -->` | no frontmatter (.claude/rules/) | `alwaysApply: false` | `trigger: manual` | `trigger: manual` | in main file | `inclusion: manual` |

Tools without activation modes (all rules always-on): Zoo Code, Gemini CLI, OpenCode, Codex CLI, Zed AI, DeepSeek Harness, Mistral Vibe, Kilo Code.

## Skills, Agents, and MCP sync

Beyond rules, conforme syncs **skills** (reusable prompts), **custom agents**, and **MCP server configs**:

```markdown
## Skill: deploy
<!-- description: Deploy the application to production -->
<!-- tools: Bash -->

Run `npm run build && npm run deploy`.

## Agent: reviewer
<!-- description: Code review agent -->
<!-- model: gpt-4o -->
<!-- tools: codebase, terminal -->

Review all changes for correctness and security.

## MCP: filesystem
<!-- command: npx -->
<!-- args: -y, @modelcontextprotocol/server-filesystem, /workspace -->

## MCP: github
<!-- url: https://api.githubcopilot.com/mcp/ -->
<!-- headers: Authorization=Bearer ${GITHUB_TOKEN} -->
```

`env` and `headers` take comma-separated `Name=Value` pairs, so a value cannot itself contain a comma.

### Feature matrix

| Adapter | Rules | Skills | Agents | MCP |
|---------|-------|--------|--------|-----|
| Claude Code | `.claude/rules/*.md` | `.claude/skills/` + `.claude/commands/` | `.claude/agents/*.md` | `.mcp.json` (merged) |
| GitHub Copilot | `.github/instructions/*.instructions.md` | `.github/skills/` | `.github/agents/*.agent.md` | `.vscode/mcp.json` (merged) |
| Cursor | `.cursor/rules/*.mdc` | `.cursor/skills/` | `.cursor/agents/*.md` | `.cursor/mcp.json` (merged) |
| Kiro (AWS) | `.kiro/steering/*.md` | `.kiro/skills/` | `.kiro/agents/*.md` | `.kiro/settings/mcp.json` (merged) |
| Devin Desktop | `.devin/rules/*.md` | `.devin/skills/` | - | `.devin/mcp_config.json` (merged) |
| Zoo Code | `.roo/rules/*.md` | `.roo/skills/` | - | `.roo/mcp.json` (merged) |
| Gemini CLI | `GEMINI.md` | `.gemini/skills/` | `.gemini/agents/*.md` | `.gemini/settings.json` (merged) |
| OpenCode | native (AGENTS.md) | `.opencode/skills/` | `opencode.json#agent` (merged) + `.opencode/agents/*.md` | `.opencode/opencode.json#mcp` or an existing root `opencode.json` (merged) |
| Zed AI | `.rules` | `.agents/skills/` | - | `.zed/settings.json` (merged) |
| Codex CLI | native (AGENTS.md) | `.agents/skills/` | `.codex/agents/*.toml` | `.codex/config.toml` (merged) |
| DeepSeek Harness | native (AGENTS.md) | `.dsh/skills/` | - | - (user-level `cordis.patch.yml`) |
| Mistral Vibe | native (AGENTS.md) | `.vibe/skills/` | `.vibe/agents/*.toml` | `.vibe/config.toml` (merged) |
| Kilo Code | native (AGENTS.md) | `.kilo/skills/` | `.kilo/agents/*.md` | `.kilo/kilo.jsonc` (merged) |
| Antigravity CLI | `.agents/rules/*.md` | `.agents/skills/` | `.agents/agents/*.md` | `.agents/mcp_config.json` (merged) |

### Skills format equivalence

Skills are reusable prompts with a description and optional tools. conforme uses the [Agent Skills](https://github.com/anthropics/skills) standard (YAML frontmatter + markdown body).

When using Claude Code as source (`source = "claude"`), conforme also reads **custom commands** from `.claude/commands/**/*.md` (a nested `frontend/component.md` becomes `frontend:component`) and syncs them as skills to all other tools:

| Tool | Path | Frontmatter |
|------|------|-------------|
| Claude Code | `.claude/skills/<name>/SKILL.md` | `name`, `description`, `allowed-tools` |
| Cursor | `.cursor/skills/<name>/SKILL.md` | `name`, `description` |
| Copilot | `.github/skills/<name>/SKILL.md` | `name`, `description`, `allowed-tools` |
| Kiro | `.kiro/skills/<name>/SKILL.md` | `name`, `description` |
| Devin Desktop | `.devin/skills/<name>/SKILL.md` | `name`, `description`, `triggers: [user]` for manual skills |
| Zoo Code | `.roo/skills/<name>/SKILL.md` | `name`, `description` |
| Gemini CLI | `.gemini/skills/<name>/SKILL.md` | `name`, `description` (no other fields) |
| OpenCode | `.opencode/skills/<name>/SKILL.md` | `name`, `description` (no `allowed-tools`) |
| Codex CLI | `.agents/skills/<name>/SKILL.md` | `name`, `description` |
| DeepSeek Harness | `.dsh/skills/<name>/SKILL.md` | `name`, `description` (kebab-case name) |
| Mistral Vibe | `.vibe/skills/<name>/SKILL.md` | `name`, `description`, `disable-model-invocation` (`vibe` and `skill-creator` are reserved) |
| Kilo Code | `.kilo/skills/<name>/SKILL.md` | `name`, `description` |
| Antigravity CLI | `.agents/skills/<name>/SKILL.md` | `name`, `description` (shared with Codex and Zed) |

Every supported tool syncs skills, with the files bundled beside `SKILL.md` (scripts, references, templates): each tool's copy is the whole skill folder, so a skill that runs `scripts/check.py` works everywhere. Each copy carries a small `.conforme` marker: a bundled file removed from the source is removed from every copy, a skill removed from the source is deleted from every tool, and `check` reports a copy that is behind. A skill folder without the marker (one written by hand in a tool) is never touched, nor is a directory the source reads its own skills from. Files that are not text (images, binaries) are not copied.

### Agents format equivalence

Agents (sub-agents) are custom AI assistants with a model, tools, and system prompt:

| Tool | Path | Format |
|------|------|--------|
| Claude Code | `.claude/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model` (Claude ids only), `tools` (Claude tool names) |
| Copilot | `.github/agents/<name>.agent.md` | YAML frontmatter: `name`, `description`, `model` (no other host's alias), `tools` |
| Cursor | `.cursor/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model` (no other host's alias; no `tools` — inherited) |
| Kiro | `.kiro/agents/<name>.md` | YAML frontmatter: `description`, `model` (no other host's alias, no `inherit`), `tools` (Kiro tools and tags: `read`, `grep`, `shell`, `@server/tool`, …; name from the file) |
| Gemini CLI | `.gemini/agents/<name>.md` | YAML frontmatter: `name`, `description`, `kind: local`, `model` (Gemini ids only), `tools` (Gemini tool names) |
| OpenCode | `opencode.json` (`agent` key) + `.opencode/agents/<name>.md` | JSON merged into `opencode.json`; markdown for per-project agents; `model` only when written as `provider/model` |
| Codex CLI | `.codex/agents/<name>.toml` | TOML: `name`, `description`, `developer_instructions` (no `model` or tools; loaded only in a trusted project; the built-in `default`, `worker` and `explorer` are skipped) |
| Mistral Vibe | `.vibe/agents/<name>.toml` | TOML: `agent_type = "subagent"`, `description`, `instructions`, `enabled_tools` (Vibe tool names) |
| Kilo Code | `.kilo/agents/<name>.md` | OpenCode format (`description`, `mode: subagent`, `provider/model`); Kilo's built-in names (`code`, `ask`, `debug`, `orchestrator`, …) are skipped |
| Antigravity CLI | `.agents/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model` (`inherit`, `flash` or `pro` only); no `tools`; the built-in `research`, `browser`, `self` and `image-generator` are never written |

Tools without agents support: Devin Desktop, Zoo Code, Zed AI, DeepSeek Harness.

Claude Code, Gemini CLI, Kiro and Mistral Vibe only accept their own tool names, so common names are translated (`Read` / `read_file` / `read`, `Bash` / `run_shell_command` / `shell`, `WebFetch` / `web_fetch`, …), MCP tools are respelled (`mcp__github__list_issues` / `mcp_github_list_issues` / `@github/list_issues`, and every MCP tool as Gemini `mcp_*` / Kiro `@mcp`), and names with no equivalent are dropped; a restricted list in which nothing translates becomes read-only access (`Read` / `read_file` / `read`) rather than every tool. A skill or agent without a description gets its name as description, since several tools skip one that has none.

### MCP format equivalence

MCP ([Model Context Protocol](https://modelcontextprotocol.io/)) servers are synced to each tool's native format:

| Tool | Path | Key / table | Format notes |
|------|------|----------|-------------|
| Claude Code | `.mcp.json` (merged) | `mcpServers` | `type: stdio/http`; `env` on stdio, `headers` on HTTP; `${VAR}` references |
| Cursor | `.cursor/mcp.json` (merged) | `mcpServers` | `type: stdio` locally, remote is just `url` + `headers`; `${env:VAR}` references |
| Devin Desktop | `.devin/mcp_config.json` (merged) | `mcpServers` | No `type`; remote is `url` + `transport: http` + `headers`; `${env:VAR}` references |
| Copilot | `.vscode/mcp.json` + `.github/mcp.json` (both merged) | `servers` / `mcpServers` | VS Code reads `.vscode/mcp.json` (`servers` key, `env` on stdio, `headers` on HTTP, `inputs`/`sandbox` preserved, `${env:VAR}`); Copilot CLI ignores it and reads `.github/mcp.json` (`mcpServers`, Claude Code shape, `${VAR}`); the cloud agent reads neither, its servers are set in the repository settings on GitHub.com |
| Kiro | `.kiro/settings/mcp.json` (merged) | `mcpServers` | Standard format; `autoApprove`/`disabledTools`/`oauth` preserved |
| Zoo Code | `.roo/mcp.json` (merged) | `mcpServers` | HTTP uses `type: streamable-http` (not `http`), no `env` on remote servers; Zoo's `alwaysAllow`/`disabledTools` preserved |
| Gemini CLI | `.gemini/settings.json` (merged) | `mcpServers` | No `type` field, uses `httpUrl` (not `url`) for HTTP |
| OpenCode | `.opencode/opencode.json`, or an existing root `opencode.json` (merged) | `mcp` | `type: local/remote`; `command` as single array; env key is `environment` (local only); `{env:VAR}` references; a new file goes to `.opencode/`, which Kilo Code does not read |
| Kilo Code | `.kilo/kilo.jsonc`, or the existing `kilo.json(c)` (merged) | `mcp` | OpenCode shape with no variable reference: Kilo refuses `{env:VAR}` in a project config, so `NAME=${NAME}` is left out (a local server inherits the environment) and any other `${VAR}` is written as is, with a warning |
| Zed AI | `.zed/settings.json` (merged) | `context_servers` | No `type` field; remote uses `url` + `headers`; no variable expansion |
| Codex CLI | `.codex/config.toml` (merged) | `[mcp_servers.<name>]` | TOML; no `${VAR}` expansion: `NAME=${NAME}` → `env_vars`, `Authorization: Bearer ${VAR}` → `bearer_token_env_var`, a `${VAR}` header → `env_http_headers`; atomic merge preserves unrelated settings, comments, target-only servers, and Codex-specific options |
| DeepSeek Harness | _(not project-scoped)_ | — | MCP servers are `@deepseek-ai/dsh-mcp-client` plugin entries in the user-level `cordis.patch.yml` under `$DSH_HOME`, so conforme generates nothing |
| Mistral Vibe | `.vibe/config.toml` (merged) | `[[mcp_servers]]` | TOML array; `transport = "stdio"` / `"streamable-http"`; `Authorization: Bearer ${VAR}` → static `auth` with `api_key_env`; any other `${VAR}` is written as is, with a warning; OAuth `auth` and other settings preserved |
| Antigravity CLI | `.agents/mcp_config.json` (merged) | `mcpServers` | No `type` field; local `command`, `args`, `env`; remote `serverUrl` + `headers` (SSE uses the same key, so it reads back as HTTP); `${VAR}` written as is, with a warning (Antigravity documents no expansion); JSONC accepted |

The source decides which servers exist: a server only the target lists is dropped (Codex keeps target-only servers), and a synced server is re-enabled (`disabled` / `enabled: false` are reset) so `check` never passes while a tool hides it. Entries conforme cannot express (a Zed extension server configured only through `settings`, a Claude Code `type: "sdk"` server, an OpenCode `{ "enabled": false }` toggle) are skipped on read and kept as they are on write. A source with no MCP server at all leaves every MCP file untouched, so servers kept by hand in a tool survive when conforme only syncs rules. conforme writes environment-variable references in each tool's syntax (Codex, which expands none, through its `env_vars` / `bearer_token_env_var` / `env_http_headers` keys) and reads them back to `${VAR}`. A reference a tool cannot express (a Codex `NAME=${OTHER}`, a `${VAR}` mixed into other text there, any reference for Zed, which expands none) is written literally; for Kilo Code, Mistral Vibe and Antigravity CLI, `sync` and `migrate` print a warning naming the server. A legacy SSE server (`type: "sse"`, or `<!-- transport: sse -->` in AGENTS.md) stays SSE in Claude Code, Cursor, Copilot, Zoo Code, Gemini CLI and Devin, is written as a bare `url` for Kiro (which reads it back as streamable HTTP), as `type: "remote"` for OpenCode and Kilo Code (which try SSE themselves), and as streamable HTTP with a warning for Codex, Zed AI and Mistral Vibe. Antigravity CLI takes one `serverUrl` key for both transports, so an SSE server is written like an HTTP one and read back as streamable HTTP.

## Examples

All examples use **Claude Code as source** — write your config once in `.claude/`, and conforme syncs to all other tools.

<details>
<summary><strong>Node.js / TypeScript</strong> — Full setup with rules, skills, agents, MCP</summary>

**1. Configure Claude Code as source:**

```toml
# .conformerc.toml
source = "claude"
```

**2. Write your rules in `.claude/rules/`:**

`.claude/rules/typescript.md`:
```markdown
---
paths:
  - "**/*.ts"
  - "**/*.tsx"
---
- Use strict TypeScript (`"strict": true` in tsconfig)
- Prefer `interface` over `type` for object shapes
- Use explicit return types on exported functions
```

`.claude/rules/testing.md`:
```markdown
---
paths:
  - "**/*.test.ts"
  - "**/*.spec.ts"
---
- Use Vitest for unit tests
- Mock external APIs, never call them in tests
- Aim for >80% coverage on business logic
```

**3. Add your main instructions in `CLAUDE.md`:**

```markdown
Use TypeScript with strict mode. Follow ESLint rules.
Run `npm test` before suggesting changes are complete.
```

**4. Add a skill in `.claude/skills/deploy/SKILL.md`:**

```markdown
---
name: deploy
description: Deploy to production
allowed-tools: Bash
---
Run `npm run build && npm run deploy`.
```

**5. Add an agent in `.claude/agents/reviewer.md`:**

```markdown
---
name: reviewer
description: Code review agent
model: sonnet
tools: Read, Bash
---
Review all TypeScript changes for correctness, type safety, and test coverage.
```

**6. Add MCP servers in `.mcp.json`:**

```json
{
  "mcpServers": {
    "filesystem": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
    }
  }
}
```

**7. Sync and install hook:**

```bash
brew install maxgfr/tap/conforme
conforme sync          # Syncs to Cursor, Copilot, Devin, Kiro, etc.
conforme hook install  # Pre-commit hook runs `conforme check`
```

This generates:
- `.cursor/rules/typescript.mdc` with `globs: "**/*.ts, **/*.tsx"`
- `.cursor/skills/deploy/SKILL.md`
- `.cursor/agents/reviewer.md`
- `.cursor/mcp.json`
- `.devin/rules/typescript.md` with `trigger: glob`
- `.github/copilot-instructions.md` + `.github/instructions/typescript.instructions.md`
- `.github/skills/deploy/SKILL.md`
- `.github/agents/reviewer.agent.md`
- `.kiro/steering/typescript.md` with `inclusion: fileMatch`
- `GEMINI.md`, `.rules`, `.roo/rules/`, etc.
- `AGENTS.md` (auto-generated from source)

**CI (GitHub Actions):**

```yaml
- name: Check AI configs in sync
  run: conforme check
```

</details>

<details>
<summary><strong>Python</strong> — FastAPI project with agent-decision rules</summary>

**`.conformerc.toml`:**

```toml
source = "claude"
only = ["cursor", "copilot", "devin", "kiro", "gemini"]
```

**`CLAUDE.md`:**

```markdown
Use Python 3.12+. Follow PEP 8 and type all functions.
Run `pytest` and `ruff check .` before suggesting changes.
```

**`.claude/rules/type-hints.md`:**

```markdown
---
paths:
  - "**/*.py"
---
- Add type annotations to all function signatures
- Prefer `list[str]` over `List[str]` (3.12+ native generics)
- Use `TypedDict` for complex dict structures
```

**`.claude/rules/testing.md`:**

```markdown
---
paths:
  - "**/test_*"
  - "**/*_test.py"
---
- Use pytest with fixtures
- Use `pytest.raises` for expected exceptions
- Mock external services with `unittest.mock`
```

**`.claude/rules/fastapi.md`:**

```markdown
---
paths:
  - "**/api/**"
  - "**/routes/**"
---
- Use Pydantic models for request/response validation
- Return proper HTTP status codes
- Add OpenAPI descriptions to endpoints
```

**`.claude/skills/venv/SKILL.md`:**

```markdown
---
name: venv
description: Set up virtual environment
allowed-tools: Bash
---
Run `python -m venv .venv && source .venv/bin/activate && pip install -e ".[dev]"`.
```

**`.claude/agents/security-reviewer.md`:**

```markdown
---
name: security-reviewer
description: Review for security vulnerabilities in Python code
model: sonnet
tools: Read, Bash
---
Check for SQL injection, SSRF, path traversal, and insecure deserialization.
Run `bandit -r src/` and review the results.
```

**Sync:**

```bash
conforme sync          # Syncs to 5 selected tools
conforme status        # Show sync state
```

</details>

<details>
<summary><strong>Rust</strong> — Multiple activation modes + pre-commit hook</summary>

**`.conformerc.toml`:**

```toml
source = "claude"
clean = true
```

**`CLAUDE.md`:**

```markdown
Use idiomatic Rust. Run `cargo clippy -- -D warnings` and `cargo test`
before suggesting changes are complete.
```

**`.claude/rules/error-handling.md`:**

```markdown
---
paths:
  - "**/*.rs"
---
- Use `anyhow::Result` for application code, `thiserror` for libraries
- Never use `.unwrap()` in production code — use `?` or `.expect("reason")`
- Return `Result` from all public functions that can fail
```

**`.claude/rules/testing.md`:**

```markdown
---
paths:
  - "**/tests/**"
  - "**/*_test.rs"
---
- Use `#[test]` for unit tests, `tests/` directory for integration
- Use `assert_eq!` with descriptive messages
- Test error cases, not just happy paths
```

**`.claude/rules/unsafe-code.md`** (no paths = always loaded):

```markdown
- Every `unsafe` block must have a `// SAFETY:` comment
- Prefer safe abstractions — only use unsafe when necessary
- Audit all `unsafe` usage before merge
```

**`.claude/skills/release/SKILL.md`:**

```markdown
---
name: release
description: Create a new release
allowed-tools: Bash
---
Run `cargo test && cargo clippy -- -D warnings`, bump version in Cargo.toml, create git tag, push.
```

**`.mcp.json`:**

```json
{
  "mcpServers": {
    "github": {
      "type": "http",
      "url": "https://api.githubcopilot.com/mcp/",
      "headers": { "Authorization": "Bearer ${GITHUB_TOKEN}" }
    }
  }
}
```

Every target tool gets this server in its own file, and those files are usually committed: keep secrets in environment variables (`${GITHUB_TOKEN}`), which conforme rewrites to each tool's syntax, never as literal values (`validate` warns about one).

**Setup with pre-commit hook:**

```bash
conforme sync && conforme hook install
# Now every commit runs `conforme check` automatically
```

</details>

<details>
<summary><strong>Go</strong> — Team workflow with CI</summary>

**`.conformerc.toml`:**

```toml
source = "claude"
exclude = ["zed", "kilo"]
generate_agents_md = true
```

**`CLAUDE.md`:**

```markdown
Use idiomatic Go. Run `go test ./...` and `golangci-lint run`
before suggesting changes are complete.
```

**`.claude/rules/error-handling.md`:**

```markdown
---
paths:
  - "**/*.go"
---
- Always check returned errors — never use `_`
- Wrap errors with `fmt.Errorf("context: %w", err)`
- Use sentinel errors for expected cases
```

**`.claude/rules/testing.md`:**

```markdown
---
paths:
  - "**/*_test.go"
---
- Use table-driven tests
- Use `testify/assert` for assertions
- Test both success and error paths
```

**`.claude/rules/api-design.md`:**

```markdown
---
paths:
  - "**/handler/**"
  - "**/api/**"
---
- Use `net/http` or chi router
- Return structured JSON errors
- Log with `slog` (structured logging)
```

**`.claude/skills/build/SKILL.md`:**

```markdown
---
name: build
description: Build and test the project
allowed-tools: Bash
---
Run `go build ./... && go test ./... && golangci-lint run`.
```

**`.claude/agents/db-reviewer.md`:**

```markdown
---
name: db-reviewer
description: Review database migrations and queries
model: sonnet
tools: Read, Bash
---
Review SQL migrations for correctness. Check for missing indexes, N+1 queries, and unsafe migrations.
Run `go test ./internal/db/...` after any migration change.
```

**Setup:**

```bash
conforme sync && conforme hook install
```

**CI (GitHub Actions):**

```yaml
- name: Check AI configs in sync
  run: conforme check
```

</details>

---

## Pre-commit hook

conforme can act as a pre-commit hook (like [Husky](https://github.com/typicode/husky)) to ensure configs stay in sync:

```bash
# Install the git hook
conforme hook install

# Remove it
conforme hook uninstall
```

The hook runs `conforme check` before each commit and blocks the commit if configs are out of sync.

## CI/CD integration

Add to your CI pipeline:

```yaml
# GitHub Actions
- name: Check AI configs in sync
  run: conforme check
```

Or use the pre-commit hook for local enforcement.

## Manual skill invocation

Manual skills preserve `disable-model-invocation: true`, `metadata.opencode/autoinvoke: "false"`, and Codex `agents/openai.yaml` with `policy.allow_implicit_invocation: false` through synchronization. In AGENTS.md, use `<!-- invocation: manual -->` in the skill section. conforme writes `metadata.opencode/autoinvoke` as a hint that OpenCode is not known to read (no reader exists on its `dev` branch), so OpenCode needs the corresponding `permission.skill` deny entries; skill synchronization does not change user permissions.

These skills run when explicitly invoked: `verify-providers`. Use `$name` in Codex or `/name` in Claude Code and OpenCode (with the plugin namespace when installed as a Claude plugin).

The skill bundle disables implicit selection in Codex and Claude Code. Its `metadata.opencode/autoinvoke: "false"` is only a hint OpenCode is not known to read, so for OpenCode merge these entries into `permission.skill` in `~/.config/opencode/opencode.json` or the project configuration; retain unrelated permissions:

```json
{
  "permission": {
    "skill": {
      "verify-providers": "deny"
    }
  }
}
```

On OpenCode 1.18.30, these rules hide the skills from the agent and reject skill-tool loading, while explicit `/name` commands remain available. Installation with `skills add` does not apply this OpenCode V1 configuration.

## License

MIT
