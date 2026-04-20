# conforme

Sync your AI coding config from any tool to all 13 others. Write once, apply everywhere.

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

1. **Write your config** in your preferred tool (Claude Code, Cursor, Windsurf, etc.) or directly in `AGENTS.md`
2. **Run `conforme sync`** — it reads from your chosen source and propagates to all detected tools
3. **Only changed files are updated** — content is compared using SHA-256 hashes, so unchanged files are never touched
4. **Orphan files are cleaned** — when you rename or remove a rule, the old generated files are automatically deleted

You can set your source tool once in `.conformerc.toml` or pass it on the command line with `--from`. If no source is specified, conforme defaults to `AGENTS.md`.

## Supported tools (13)

### Tools with per-rule config files

| Tool | Config format | Frontmatter | AGENTS.md |
|------|--------------|-------------|-----------|
| Claude Code | `CLAUDE.md` + `.claude/rules/*.md` | `paths` (glob array) | via `@AGENTS.md` include |
| Cursor | `.cursor/rules/*.mdc` | `alwaysApply`, `globs`, `description` | Native |
| Windsurf | `.windsurf/rules/*.md` | `trigger`, `description`, `globs` | Native |
| GitHub Copilot | `.github/copilot-instructions.md` + `.github/instructions/` | `applyTo`, `excludeAgent` | Native |
| Continue.dev | `.continue/rules/*.md` | `name`, `globs` (array), `alwaysApply` | Not yet |
| Kiro (AWS) | `.kiro/steering/*.md` | `inclusion`, `fileMatchPattern`, `name`, `description` | Native |
| Roo Code / Cline | `.roo/rules/*.md` | None (plain Markdown) | Native |
| Amazon Q | `.amazonq/rules/*.md` | None (plain Markdown) | N/A |

### Tools that read AGENTS.md natively (single-file sync)

| Tool | Primary file | Notes |
|------|-------------|-------|
| OpenAI Codex CLI | `AGENTS.md` | Also supports `AGENTS.override.md` |
| OpenCode | `AGENTS.md` | Falls back to `CLAUDE.md` |
| Gemini CLI | `GEMINI.md` | Configurable to read AGENTS.md via settings.json |
| Zed AI | `.rules` | Fallback chain: `.rules` → `.cursorrules` → `AGENTS.md` → `CLAUDE.md` |
| Amp (Sourcegraph) | `AGENTS.md` | Falls back to `AGENT.md` or `CLAUDE.md` |

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
conforme remove cursor,windsurf      # Remove generated files
conforme migrate --source gemini --output opencode  # Migrate between tools
conforme hook install                # Git pre-commit hook
conforme help-ai                     # Show tool format details
```

## `.conformerc.toml` configuration

Create a `.conformerc.toml` at your project root to customize conforme's behavior:

```toml
# Source tool — conforme reads config from here
source = "claude"

# Only sync to these tools (default: all detected)
only = ["cursor", "copilot", "windsurf"]

# Exclude these tools
exclude = ["zed", "amp"]

# Auto-generate AGENTS.md from source (default: true)
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

| Mode | AGENTS.md | Claude | Cursor | Windsurf | Copilot | Continue.dev | Kiro |
|------|-----------|--------|--------|----------|---------|-------------|------|
| Always | `<!-- activation: always -->` | no frontmatter (in CLAUDE.md) | `alwaysApply: true` | `trigger: always_on` | in main file | `alwaysApply: true` | `inclusion: always` |
| Glob | `<!-- activation: glob **/*.ts -->` | `paths: [**/*.ts]` | `globs: "**/*.ts"` | `trigger: glob` + `globs:` | `applyTo: "**/*.ts"` | `globs: ["**/*.ts"]` | `inclusion: fileMatch` + `fileMatchPattern:` |
| Agent Decision | `<!-- activation: agent-decision -->` | no frontmatter (.claude/rules/) | `description: "..."` | `trigger: model_decision` | in main file | `description: "..."` | `inclusion: auto` |
| Manual | `<!-- activation: manual -->` | no frontmatter (.claude/rules/) | `alwaysApply: false` | `trigger: manual` | in main file | `alwaysApply: false` | `inclusion: manual` |

Tools without activation modes (all rules always-on): Roo Code, Amazon Q, Gemini CLI, OpenCode, Codex CLI, Zed AI, Amp.

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
```

### Feature matrix

| Adapter | Rules | Skills | Agents | MCP |
|---------|-------|--------|--------|-----|
| Claude Code | `.claude/rules/*.md` | `.claude/skills/` + `.claude/commands/` | `.claude/agents/*.md` | `.mcp.json` |
| GitHub Copilot | `.github/instructions/*.md` | `.github/prompts/*.prompt.md` | `.github/agents/*.agent.md` | `.vscode/mcp.json` |
| Cursor | `.cursor/rules/*.mdc` | `.cursor/skills/` | `.cursor/agents/*.md` | `.cursor/mcp.json` |
| Kiro (AWS) | `.kiro/steering/*.md` | `.kiro/skills/` | `.kiro/agents/*.md` | `.kiro/settings/mcp.json` |
| Windsurf | `.windsurf/rules/*.md` | `.windsurf/skills/` | - | `.windsurf/mcp.json` |
| Continue.dev | `.continue/rules/*.md` | - | - | `.continue/mcp.json` |
| Roo Code | `.roo/rules/*.md` | `.roo/skills/` | - | `.roo/mcp.json` |
| Amazon Q | `.amazonq/rules/*.md` | - | `.amazonq/cli-agents/*.json` | `.amazonq/mcp.json` |
| Gemini CLI | `GEMINI.md` | `.gemini/skills/` | `.gemini/agents/*.md` | `.gemini/settings.json` |
| OpenCode | native (AGENTS.md) | `.opencode/skills/` | `opencode.json#agent` + `.opencode/agents/*.md` | `opencode.json#mcp` |
| Zed AI | `.rules` | - | - | `.zed/settings.json` |
| Codex CLI | native (AGENTS.md) | `.agents/skills/` | - | - (global only) |
| Amp | native (AGENTS.md) | `.agents/skills/` | - | `.amp/settings.json` |

### Skills format equivalence

Skills are reusable prompts with a description and optional tools. conforme uses the [Agent Skills](https://github.com/anthropics/skills) standard (YAML frontmatter + markdown body).

When using Claude Code as source (`source = "claude"`), conforme also reads **custom commands** from `.claude/commands/*.md` and syncs them as skills to all other tools:

| Tool | Path | Frontmatter |
|------|------|-------------|
| Claude Code | `.claude/skills/<name>/SKILL.md` | `name`, `description`, `allowed-tools` |
| Cursor | `.cursor/skills/<name>/SKILL.md` | `name`, `description` |
| Copilot | `.github/prompts/<name>.prompt.md` | `description`, `tools` |
| Kiro | `.kiro/skills/<name>/SKILL.md` | `name`, `description` |
| Windsurf | `.windsurf/skills/<name>/SKILL.md` | `name`, `description` |
| Roo Code | `.roo/skills/<name>/SKILL.md` | `name`, `description` |
| Gemini CLI | `.gemini/skills/<name>/SKILL.md` | `name`, `description` (no other fields) |
| OpenCode | `.opencode/skills/<name>/SKILL.md` | `name`, `description` (no `allowed-tools`) |
| Codex CLI | `.agents/skills/<name>/SKILL.md` | `name`, `description` |
| Amp | `.agents/skills/<name>/SKILL.md` | `name`, `description` (shared Codex format) |

Tools without skills support: Continue.dev, Zed AI, Amazon Q.

### Agents format equivalence

Agents (sub-agents) are custom AI assistants with a model, tools, and system prompt:

| Tool | Path | Format |
|------|------|--------|
| Claude Code | `.claude/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| Copilot | `.github/agents/<name>.agent.md` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| Cursor | `.cursor/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model` (no `tools` — inherited) |
| Kiro | `.kiro/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| Gemini CLI | `.gemini/agents/<name>.md` | YAML frontmatter: `name`, `description`, `kind: local`, `model`, `tools` |
| OpenCode | `opencode.json` (`agent` key) + `.opencode/agents/<name>.md` | JSON merged into `opencode.json`; markdown for per-project agents |
| Amazon Q | `.amazonq/cli-agents/<name>.json` | JSON per agent: `{ "description", "model", "tools", "prompt" }` |

Tools without agents support: Windsurf, Continue.dev, Roo Code, Codex CLI, Zed AI, Amp.

### MCP format equivalence

MCP ([Model Context Protocol](https://modelcontextprotocol.io/)) servers are synced to tool-specific JSON formats:

| Tool | Path | JSON key | Format notes |
|------|------|----------|-------------|
| Claude Code | `.mcp.json` | `mcpServers` | `type: stdio/http`, with `env`, `headers` |
| Cursor | `.cursor/mcp.json` | `mcpServers` | `type: stdio/http` |
| Windsurf | `.windsurf/mcp.json` (best-effort project) / `~/.codeium/windsurf/mcp_config.json` (global) | `mcpServers` | No `type` field; HTTP uses `serverUrl` (not `url`) |
| Copilot | `.vscode/mcp.json` | `servers` | Uses `servers` key (not `mcpServers`); supports `env` + `headers` |
| Continue.dev | `.continue/mcpServers/mcp.json` | `mcpServers` | `type: stdio/http` |
| Kiro | `.kiro/settings/mcp.json` | `mcpServers` | Standard format |
| Roo Code | `.roo/mcp.json` | `mcpServers` | Standard format |
| Amazon Q | `.amazonq/mcp.json` | `mcpServers` | Standard format (legacy workspace MCP file) |
| Gemini CLI | `.gemini/settings.json` | `mcpServers` | No `type` field, uses `httpUrl` (not `url`) for HTTP |
| OpenCode | `opencode.json` (merged) | `mcp` | `type: local/remote`; `command` as single array; env key is `environment` |
| Zed AI | `.zed/settings.json` | `context_servers` | No `type` field |
| Amp | `.amp/settings.json` | `amp.mcpServers` | Dotted key |

Tools without project-level MCP support: Codex CLI (global only via `~/.codex/config.toml` in TOML format).

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
conforme sync          # Syncs to Cursor, Copilot, Windsurf, Kiro, etc.
conforme hook install  # Pre-commit hook runs `conforme check`
```

This generates:
- `.cursor/rules/typescript.mdc` with `globs: "**/*.ts, **/*.tsx"`
- `.cursor/skills/deploy/SKILL.md`
- `.cursor/agents/reviewer.mdc`
- `.cursor/mcp.json`
- `.windsurf/rules/typescript.md` with `trigger: glob`
- `.github/copilot-instructions.md` + `.github/instructions/typescript.instructions.md`
- `.github/prompts/deploy.prompt.md`
- `.github/agents/reviewer.agent.md`
- `.kiro/steering/typescript.md` with `inclusion: fileMatch`
- `GEMINI.md`, `.rules`, `.roo/rules/`, `.amazonq/rules/`, etc.
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
only = ["cursor", "copilot", "windsurf", "kiro", "gemini"]
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

## License

MIT
