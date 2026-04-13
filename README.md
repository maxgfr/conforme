# conforme

Sync your AI coding config from any tool to all 13 others. Write once, apply everywhere.

AGENTS.md is governed by the [Agentic AI Foundation](https://www.linuxfoundation.org/press/linux-foundation-launches-agentic-ai-initiative) (Linux Foundation) with 146+ member organizations including Anthropic, OpenAI, Google, AWS, and Microsoft.

## Install

```bash
# Homebrew
brew install maxgfr/tap/conforme

# From source
cargo install --path .

# Pre-built binaries
# Download from GitHub Releases
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
| Cursor | `.cursor/rules/*.mdc` | `.cursor/skills/` | `.cursor/agents/*.mdc` | `.cursor/mcp.json` |
| Kiro (AWS) | `.kiro/steering/*.md` | `.kiro/skills/` | `.kiro/agents/*.md` | `.kiro/settings/mcp.json` |
| Windsurf | `.windsurf/rules/*.md` | `.windsurf/skills/` | - | `.windsurf/mcp.json` |
| Continue.dev | `.continue/rules/*.md` | - | - | `.continue/mcp.json` |
| Roo Code | `.roo/rules/*.md` | `.roo/skills/` | - | `.roo/mcp.json` |
| Amazon Q | `.amazonq/rules/*.md` | - | `.amazonq/cli-agents/*.json` | `.amazonq/mcp.json` |
| Gemini CLI | `GEMINI.md` | `.gemini/skills/` | `.gemini/agents/*.md` | `.gemini/settings.json` |
| OpenCode | native (AGENTS.md) | `.opencode/skills/` | `.opencode/agents.json` | `.opencode/mcp.json` |
| Zed AI | `.rules` | - | - | `.zed/settings.json` |
| Codex CLI | native (AGENTS.md) | `.agents/skills/` | - | - (global only) |
| Amp | native (AGENTS.md) | `.agents/skills/` | - | `.amp/settings.json` |

### Skills format equivalence

Skills are reusable prompts with a description and optional tools. conforme uses the [SKILL.md](https://github.com/anthropics/SKILL.md) standard (YAML frontmatter + markdown body).

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
| OpenCode | `.opencode/skills/<name>/SKILL.md` | `name`, `description`, `allowed-tools` |
| Codex CLI | `.agents/skills/<name>/SKILL.md` | `name`, `description` |
| Amp | `.agents/skills/<name>/SKILL.md` | `name`, `description` (shared Codex format) |

Tools without skills support: Continue.dev, Zed AI, Amazon Q.

### Agents format equivalence

Agents (sub-agents) are custom AI assistants with a model, tools, and system prompt:

| Tool | Path | Format |
|------|------|--------|
| Claude Code | `.claude/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| Copilot | `.github/agents/<name>.agent.md` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| Cursor | `.cursor/agents/<name>.mdc` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| Kiro | `.kiro/agents/<name>.md` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| Gemini CLI | `.gemini/agents/<name>.md` | YAML frontmatter: `name`, `description`, `kind: local`, `model`, `tools` |
| OpenCode | `.opencode/agents.json` | JSON: `{ "agent": { "<name>": { "mode": "subagent", "model", "prompt" } } }` |
| Amazon Q | `.amazonq/cli-agents/<name>.json` | JSON per agent: `{ "description", "model", "tools", "prompt" }` |

Tools without agents support: Windsurf, Continue.dev, Roo Code, Codex CLI, Zed AI, Amp.

### MCP format equivalence

MCP ([Model Context Protocol](https://modelcontextprotocol.io/)) servers are synced to tool-specific JSON formats:

| Tool | Path | JSON key | Format notes |
|------|------|----------|-------------|
| Claude Code | `.mcp.json` | `mcpServers` | `type: stdio/http`, with `env`, `headers` |
| Cursor | `.cursor/mcp.json` | `mcpServers` | `type: stdio/http` |
| Windsurf | `.windsurf/mcp.json` | `mcpServers` | `type: stdio/http` |
| Copilot | `.vscode/mcp.json` | `servers` | Uses `servers` key (not `mcpServers`), no headers |
| Continue.dev | `.continue/mcp.json` | `mcpServers` | `type: stdio/http` |
| Kiro | `.kiro/settings/mcp.json` | `mcpServers` | Standard format with `disabled` field |
| Roo Code | `.roo/mcp.json` | `mcpServers` | Standard format with `alwaysAllow` |
| Amazon Q | `.amazonq/mcp.json` | `mcpServers` | Standard format |
| Gemini CLI | `.gemini/settings.json` | `mcpServers` | No `type` field, uses `httpUrl` (not `url`) for HTTP |
| OpenCode | `.opencode/mcp.json` | `mcp` | `type: local/remote` (not stdio/http) |
| Zed AI | `.zed/settings.json` | `context_servers` | `source: "custom"` required, no `type` field |
| Amp | `.amp/settings.json` | `mcpServers` | Standard format |

Tools without project-level MCP support: Codex CLI (global only via `~/.codex/config.toml` in TOML format).

## Examples

<details>
<summary><strong>Node.js / TypeScript</strong></summary>

**AGENTS.md**

```markdown
# My Node.js Project

Use TypeScript with strict mode. Follow ESLint rules.
Run `npm test` before suggesting changes are complete.

## Rule: TypeScript
<!-- activation: glob **/*.ts,**/*.tsx -->

- Use strict TypeScript (`"strict": true` in tsconfig)
- Prefer `interface` over `type` for object shapes
- Use explicit return types on exported functions

## Rule: Testing
<!-- activation: glob **/*.test.ts,**/*.spec.ts -->

- Use Vitest for unit tests
- Mock external APIs, never call them in tests
- Aim for >80% coverage on business logic

## Skill: deploy
<!-- description: Deploy to production -->
<!-- tools: Bash -->

Run `npm run build && npm run deploy`.

## MCP: filesystem
<!-- command: npx -->
<!-- args: -y, @modelcontextprotocol/server-filesystem, . -->
```

**Setup**

```bash
brew install maxgfr/tap/conforme
cd my-node-project
conforme init          # Creates AGENTS.md + .conformerc.toml
conforme sync          # Syncs to all detected tools
conforme hook install  # Git pre-commit hook
```

**Or use Claude Code as source** — add to `.conformerc.toml`:

```toml
source = "claude"
```

Then write your rules in `.claude/rules/` and run `conforme sync`.

**CI (GitHub Actions)**

```yaml
- name: Check AI configs
  run: |
    curl -L -o conforme https://github.com/maxgfr/conforme/releases/latest/download/conforme-linux-x64
    chmod +x conforme && sudo mv conforme /usr/local/bin/
    conforme check
```

</details>

<details>
<summary><strong>Python</strong></summary>

**AGENTS.md**

```markdown
# My Python Project

Use Python 3.12+. Follow PEP 8 and type all functions.
Run `pytest` and `ruff check .` before suggesting changes.

## Rule: Type Hints
<!-- activation: glob **/*.py -->

- Add type annotations to all function signatures
- Use `from __future__ import annotations` for forward references
- Prefer `list[str]` over `List[str]` (Python 3.9+)

## Rule: Testing
<!-- activation: glob **/test_*,**/*_test.py -->

- Use pytest with fixtures
- Use `pytest.raises` for expected exceptions
- Mock external services with `unittest.mock`

## Rule: FastAPI
<!-- activation: glob **/api/**,**/routes/** -->

- Use Pydantic models for request/response validation
- Return proper HTTP status codes
- Add OpenAPI descriptions to endpoints

## Skill: venv
<!-- description: Set up virtual environment -->
<!-- tools: Bash -->

Run `python -m venv .venv && source .venv/bin/activate && pip install -e ".[dev]"`.
```

**Setup**

```bash
conforme init && conforme sync && conforme hook install
```

</details>

<details>
<summary><strong>Rust</strong></summary>

**AGENTS.md**

```markdown
# My Rust Project

Use idiomatic Rust. Run `cargo clippy -- -D warnings` and `cargo test`
before suggesting changes are complete.

## Rule: Error Handling
<!-- activation: glob **/*.rs -->

- Use `anyhow::Result` for application code, `thiserror` for libraries
- Never use `.unwrap()` in production code — use `?` or `.expect("reason")`
- Return `Result` from all public functions that can fail

## Rule: Testing
<!-- activation: glob **/tests/**,**/*_test.rs -->

- Use `#[test]` for unit tests, `tests/` directory for integration
- Use `assert_eq!` with descriptive messages
- Test error cases, not just happy paths

## Rule: Unsafe Code
<!-- activation: agent-decision -->
<!-- description: Apply when reviewing code that uses unsafe blocks -->

- Every `unsafe` block must have a `// SAFETY:` comment
- Prefer safe abstractions — only use unsafe when necessary

## Skill: release
<!-- description: Create a new release -->
<!-- tools: Bash -->

Run `cargo test && cargo clippy -- -D warnings`, bump version, tag, push.
```

**Setup**

```bash
conforme init && conforme sync && conforme hook install
```

**Or use Cursor as source:**

```toml
# .conformerc.toml
source = "cursor"
only = ["claude", "copilot", "windsurf", "kiro"]
```

</details>

<details>
<summary><strong>Go</strong></summary>

**AGENTS.md**

```markdown
# My Go Project

Use idiomatic Go. Run `go test ./...` and `golangci-lint run`
before suggesting changes are complete.

## Rule: Error Handling
<!-- activation: glob **/*.go -->

- Always check returned errors — never use `_`
- Wrap errors with `fmt.Errorf("context: %w", err)`
- Use sentinel errors for expected cases

## Rule: Testing
<!-- activation: glob **/*_test.go -->

- Use table-driven tests
- Use `testify/assert` for assertions
- Test both success and error paths

## Rule: API Design
<!-- activation: glob **/handler/**,**/api/** -->

- Use `net/http` or chi router
- Return structured JSON errors
- Log with `slog` (structured logging)

## Skill: build
<!-- description: Build and test the project -->
<!-- tools: Bash -->

Run `go build ./... && go test ./... && golangci-lint run`.
```

**Setup**

```bash
conforme init && conforme sync && conforme hook install
```

**Makefile integration:**

```makefile
.PHONY: setup sync check
setup: ; conforme hook install
sync:  ; conforme sync
check: ; conforme check
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
