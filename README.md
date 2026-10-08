# conforme

Sync your AI coding config from any tool to 11 others. Write once, apply everywhere.

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
2. **Run `conforme sync`** — it reads from your chosen source and propagates to all detected tools
3. **Only changed files are updated** — content is compared using SHA-256 hashes, so unchanged files are never touched
4. **Orphan files are cleaned** — when you rename or remove a rule, the old generated files are automatically deleted. Only files of the kind conforme writes are touched: files a tool accepts but conforme never generates (Kiro `.json` agents, flat DeepSeek skills, hand-written `.md` Copilot agents) are left alone. A source that reads back empty (no instructions, rules, skills, agents or MCP servers) warns "nothing to sync" and writes and cleans nothing. `migrate` removes the source tool's files under the same rule, and never a file or directory the output or another detected tool still uses (`.agents/skills/` shared by Codex, Amp and Zed included)
5. **Shared settings are merged, never replaced** — settings and MCP files that also hold your own configuration (`.mcp.json`, `.cursor/mcp.json`, `.kiro/settings/mcp.json`, `.devin/mcp_config.json`, `opencode.json`, `.zed/settings.json`, `.gemini/settings.json`, `.amp/settings.json`, `.vscode/mcp.json`, `.roo/mcp.json`, `.codex/config.toml`) only have conforme's key updated. JSONC comments and trailing commas are kept, per-server options a tool added (Kiro `autoApprove`, Zoo Code approvals, Gemini `trust`, …) survive, a file conforme cannot parse is never overwritten, and `remove`/`migrate` never delete one
6. **Each tool gets values it accepts** — agent tool names are translated into each tool's vocabulary (Claude Code, Gemini CLI and Kiro reject or ignore names they do not know), a model id another tool cannot use is left out, skill and agent names are written as kebab-case ASCII (rules, which only become files, keep other letters), and environment-variable references in MCP configs are rewritten to each tool's syntax (`${VAR}`, `${env:VAR}`, `{env:VAR}`)

You can set your source tool once in `.conformerc.toml` or pass it on the command line with `--from`. If no source is specified, conforme defaults to `AGENTS.md`.

## Supported tools (12)

### Tools with per-rule config files

| Tool | Config format | Frontmatter | AGENTS.md |
|------|--------------|-------------|-----------|
| Claude Code | `CLAUDE.md` + `.claude/rules/**/*.md` | `paths` (glob list or comma-separated string) | Native when no `CLAUDE.md` exists |
| Cursor | `.cursor/rules/*.mdc` | `alwaysApply`, `globs`, `description` | Native |
| Devin Desktop (formerly Windsurf) | `.devin/rules/*.md` (legacy `.windsurf/rules/*.md` is read too) | `trigger`, `description`, `globs` | Native |
| GitHub Copilot | `.github/copilot-instructions.md` + `.github/instructions/**/*.instructions.md` | `applyTo`, `excludeAgent` | Native |
| Kiro (AWS) | `.kiro/steering/*.md` | `inclusion`, `fileMatchPattern`, `name`, `description` | Native |
| Zoo Code (Roo Code fork) | `.roo/rules/*.md` | None (plain Markdown) | Native |

### Tools that read AGENTS.md natively (single-file sync)

| Tool | Primary file | Notes |
|------|-------------|-------|
| OpenAI Codex CLI | `AGENTS.md` | Also supports `AGENTS.override.md` (personal; not used as a conforme source) |
| OpenCode | `AGENTS.md` | Falls back to `CLAUDE.md` |
| Gemini CLI | `GEMINI.md` | Reads AGENTS.md when `context.fileName` in `.gemini/settings.json` names it |
| Zed AI | `.rules` | First match wins: `.rules` → `.cursorrules` → `.windsurfrules` → `.clinerules` → `.github/copilot-instructions.md` → `AGENT.md` → `AGENTS.md` → `CLAUDE.md` → `GEMINI.md` |
| Amp | `AGENTS.md` | Falls back to `AGENT.md` or `CLAUDE.md` |
| DeepSeek Harness (`dsh`) | `AGENTS.md` | Also loads `CLAUDE.md` when present; skills in `.dsh/skills/` |

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

## `.conformerc.toml` configuration

Create a `.conformerc.toml` at your project root to customize conforme's behavior:

```toml
# Source tool — conforme reads config from here
source = "claude"

# Only sync to these tools (default: all detected)
only = ["cursor", "copilot", "devin"]

# Exclude these tools
exclude = ["zed", "amp"]

# Auto-generate AGENTS.md from source (default: true; never applies when the
# source reads AGENTS.md itself: codex, opencode, amp, deepseek; claude when the
# project has no CLAUDE.md; gemini when context.fileName names AGENTS.md)
generate_agents_md = true

# Clean orphan files on sync (default: true)
clean = true
```

When `source` is set, conforme reads your rules, skills, agents, and MCP servers from that tool's config files instead of `AGENTS.md`. This means you can author your config in whichever tool you prefer and have it propagated everywhere else.

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

| Mode | AGENTS.md | Claude | Cursor | Devin | Copilot | Kiro |
|------|-----------|--------|--------|----------|---------|------|
| Always | `<!-- activation: always -->` | no frontmatter (in CLAUDE.md) | `alwaysApply: true` | `trigger: always_on` | in main file | `inclusion: always` |
| Glob | `<!-- activation: glob **/*.ts -->` | `paths: [**/*.ts]` | `globs: "**/*.ts"` | `trigger: glob` + `globs:` | `applyTo: "**/*.ts"` | `inclusion: fileMatch` + `fileMatchPattern:` |
| Agent Decision | `<!-- activation: agent-decision -->` | no frontmatter (.claude/rules/) | `description: "..."` | `trigger: model_decision` | in main file | `inclusion: auto` |
| Manual | `<!-- activation: manual -->` | no frontmatter (.claude/rules/) | `alwaysApply: false` | `trigger: manual` | in main file | `inclusion: manual` |

Tools without activation modes (all rules always-on): Zoo Code, Gemini CLI, OpenCode, Codex CLI, Zed AI, Amp.

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
| OpenCode | native (AGENTS.md) | `.opencode/skills/` | `opencode.json#agent` (merged) + `.opencode/agents/*.md` | `opencode.json#mcp` (merged) |
| Zed AI | `.rules` | `.agents/skills/` | - | `.zed/settings.json` (merged) |
| Codex CLI | native (AGENTS.md) | `.agents/skills/` | - | `.codex/config.toml` (merged) |
| Amp | native (AGENTS.md) | `.agents/skills/` | - | `.amp/settings.json` or `.amp/settings.jsonc` (merged) |
| DeepSeek Harness | native (AGENTS.md) | `.dsh/skills/` | - | - (user-level `cordis.patch.yml`) |

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
| Amp | `.agents/skills/<name>/SKILL.md` | `name`, `description` (shared Codex format) |
| DeepSeek Harness | `.dsh/skills/<name>/SKILL.md` | `name`, `description` (kebab-case name) |

Every supported tool syncs skills.

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

Tools without agents support: Devin Desktop, Zoo Code, Codex CLI, Zed AI, Amp, DeepSeek Harness.

Claude Code, Gemini CLI and Kiro only accept their own tool names, so common names are translated (`Read` / `read_file` / `read`, `Bash` / `run_shell_command` / `shell`, `WebFetch` / `web_fetch`, …), MCP tools are respelled (`mcp__github__list_issues` / `mcp_github_list_issues` / `@github/list_issues`, and every MCP tool as Gemini `mcp_*` / Kiro `@mcp`), and names with no equivalent are dropped; a restricted list in which nothing translates becomes read-only access (`Read` / `read_file` / `read`) rather than every tool. A skill or agent without a description gets its name as description, since several tools skip one that has none.

### MCP format equivalence

MCP ([Model Context Protocol](https://modelcontextprotocol.io/)) servers are synced to each tool's native format:

| Tool | Path | Key / table | Format notes |
|------|------|----------|-------------|
| Claude Code | `.mcp.json` (merged) | `mcpServers` | `type: stdio/http`; `env` on stdio, `headers` on HTTP; `${VAR}` references |
| Cursor | `.cursor/mcp.json` (merged) | `mcpServers` | `type: stdio` locally, remote is just `url` + `headers`; `${env:VAR}` references |
| Devin Desktop | `.devin/mcp_config.json` (merged) | `mcpServers` | No `type`; remote is `url` + `transport: http` + `headers`; `${env:VAR}` references |
| Copilot | `.vscode/mcp.json` (merged) | `servers` | VS Code format: `servers` key (not `mcpServers`), `env` on stdio, `headers` on HTTP, `inputs`/`sandbox` preserved; `${env:VAR}` references. The Copilot CLI reads `.mcp.json` instead, which the Claude Code target writes |
| Kiro | `.kiro/settings/mcp.json` (merged) | `mcpServers` | Standard format; `autoApprove`/`disabledTools`/`oauth` preserved |
| Zoo Code | `.roo/mcp.json` (merged) | `mcpServers` | HTTP uses `type: streamable-http` (not `http`), no `env` on remote servers; Zoo's `alwaysAllow`/`disabledTools` preserved |
| Gemini CLI | `.gemini/settings.json` (merged) | `mcpServers` | No `type` field, uses `httpUrl` (not `url`) for HTTP |
| OpenCode | `opencode.json` (merged) | `mcp` | `type: local/remote`; `command` as single array; env key is `environment` (local only); `{env:VAR}` references |
| Zed AI | `.zed/settings.json` (merged) | `context_servers` | No `type` field; remote uses `url` + `headers`; no variable expansion |
| Amp | `.amp/settings.json` or `.amp/settings.jsonc` (merged) | `amp.mcpServers` | Dotted key; no `type` field |
| Codex CLI | `.codex/config.toml` (merged) | `[mcp_servers.<name>]` | TOML; no `${VAR}` expansion: `NAME=${NAME}` → `env_vars`, `Authorization: Bearer ${VAR}` → `bearer_token_env_var`, a `${VAR}` header → `env_http_headers`; atomic merge preserves unrelated settings, comments, target-only servers, and Codex-specific options |
| DeepSeek Harness | _(not project-scoped)_ | — | MCP servers are `@deepseek-ai/dsh-mcp-client` plugin entries in the user-level `cordis.patch.yml` under `$DSH_HOME`, so conforme generates nothing |

The source decides which servers exist: a server only the target lists is dropped (Codex keeps target-only servers), and a synced server is re-enabled (`disabled` / `enabled: false` are reset) so `check` never passes while a tool hides it. Entries conforme cannot express (a Zed extension server configured only through `settings`, a Claude Code `type: "sdk"` server, an OpenCode `{ "enabled": false }` toggle) are skipped on read and kept as they are on write. A source with no MCP server at all leaves every MCP file untouched, so servers kept by hand in a tool survive when conforme only syncs rules. conforme writes environment-variable references in each tool's syntax (Codex, which expands none, through its `env_vars` / `bearer_token_env_var` / `env_http_headers` keys) and reads them back to `${VAR}`.

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
    "context7": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@upstash/context7-mcp"]
    }
  }
}
```

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
exclude = ["zed", "amp"]
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
