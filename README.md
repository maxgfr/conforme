# conforme

Universal AI coding agent config synchronization. Treats [AGENTS.md](https://github.com/agentsmd/agents.md) as the source of truth and syncs to all tool-specific formats.

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
# Initialize in your project
conforme init

# Edit AGENTS.md with your instructions and rules
# Then sync to all detected tools
conforme sync

# Preview what would change
conforme sync --dry-run

# Sync only specific tools
conforme sync --only claude,cursor,windsurf

# Check if configs are in sync (for CI)
conforme check

# Remove generated files for specific tools
conforme remove cursor,windsurf

# See status of all tools
conforme status

# Show all supported tools and format details
conforme help-ai
```

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

| Mode | Syntax | Cursor | Windsurf | Copilot | Kiro |
|------|--------|--------|----------|---------|------|
| Always | `<!-- activation: always -->` | `alwaysApply: true` | `trigger: always_on` | in main file | `inclusion: always` |
| Glob | `<!-- activation: glob **/*.ts -->` | `globs: "..."` | `trigger: glob` | `applyTo: "..."` | `inclusion: fileMatch` |
| Agent Decision | `<!-- activation: agent-decision -->` | `description: "..."` | `trigger: model_decision` | in main file | `inclusion: auto` |
| Manual | `<!-- activation: manual -->` | `alwaysApply: false` | `trigger: manual` | in main file | `inclusion: manual` |

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
| Claude Code | `.claude/rules/*.md` | `.claude/skills/` | `.claude/agents/*.md` | `.mcp.json` |
| GitHub Copilot | `.github/instructions/*.md` | `.github/prompts/*.prompt.md` | `.github/agents/*.agent.md` | `.vscode/mcp.json` |
| Cursor | `.cursor/rules/*.mdc` | - | `.cursor/agents/*.mdc` | `.cursor/mcp.json` |
| Kiro (AWS) | `.kiro/steering/*.md` | `.kiro/skills/` | `.kiro/agents/*.md` | `.kiro/settings/mcp.json` |
| Windsurf | `.windsurf/rules/*.md` | - | - | `.windsurf/mcp.json` |
| Continue.dev | `.continue/rules/*.md` | - | - | `.continue/mcp.json` |
| Roo Code | `.roo/rules/*.md` | - | - | `.roo/mcp.json` |
| Amazon Q | `.amazonq/rules/*.md` | - | - | `.amazonq/mcp.json` |
| Gemini CLI | `GEMINI.md` | - | `.gemini/agents/*.md` | `.gemini/settings.json` |
| OpenCode | native (AGENTS.md) | - | `.opencode/agents.json` | `.opencode/mcp.json` |
| Zed AI | `.rules` | - | - | `.zed/settings.json` |
| Codex CLI | native (AGENTS.md) | `.agents/skills/` | - | - (global only) |
| Amp | native (AGENTS.md) | - | - | - |

## Tutorial: Node.js project

### 1. Initialize

```bash
cd my-node-project
npm init -y # if needed
git init    # if needed

# Install conforme
brew install maxgfr/tap/conforme

# Initialize
conforme init
```

### 2. Configure AGENTS.md

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

### 3. Sync and install hook

```bash
conforme sync
conforme hook install
```

### 4. Add to CI (GitHub Actions)

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  check-ai-configs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install conforme
        run: |
          curl -L -o conforme https://github.com/maxgfr/conforme/releases/latest/download/conforme-linux-x64
          chmod +x conforme
          sudo mv conforme /usr/local/bin/
      - name: Check AI configs in sync
        run: conforme check
```

### 5. Add to package.json (alternative to conforme hook)

```json
{
  "scripts": {
    "prepare": "conforme hook install",
    "conforme:sync": "conforme sync",
    "conforme:check": "conforme check"
  }
}
```

Now `npm install` will automatically install the pre-commit hook.

---

## Tutorial: Rust project

### 1. Initialize

```bash
cd my-rust-project
cargo init # if needed
git init   # if needed

# Install conforme
brew install maxgfr/tap/conforme

# Initialize
conforme init
```

### 2. Configure AGENTS.md

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
<!-- activation: glob **/*.rs -->
<!-- activation: agent-decision -->
<!-- description: Apply when reviewing code that uses unsafe blocks -->

- Every `unsafe` block must have a `// SAFETY:` comment
- Prefer safe abstractions — only use unsafe when necessary
- Document invariants that the caller must uphold

## Skill: release
<!-- description: Create a new release -->
<!-- tools: Bash -->

1. Run `cargo test`
2. Run `cargo clippy -- -D warnings`
3. Bump version in Cargo.toml
4. Commit and tag: `git tag v$(cargo pkgid | cut -d# -f2)`
5. Push: `git push && git push --tags`
```

### 3. Sync and install hook

```bash
conforme sync
conforme hook install
```

### 4. Add to CI (GitHub Actions)

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test
      - run: cargo clippy -- -D warnings

  check-ai-configs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install conforme
        run: |
          curl -L -o conforme https://github.com/maxgfr/conforme/releases/latest/download/conforme-linux-x64
          chmod +x conforme
          sudo mv conforme /usr/local/bin/
      - run: conforme check
```

### 5. Add to Makefile (alternative)

```makefile
.PHONY: setup sync check

setup:
	conforme hook install

sync:
	conforme sync

check:
	conforme check
```

---

## How it works

1. **Parse**: Reads `AGENTS.md` and extracts instructions, rules, skills, agents, and MCP servers
2. **Detect**: Scans for tool-specific directories (`.cursor/`, `.windsurf/`, `.kiro/`, etc.)
3. **Generate**: Converts normalized config to each tool's format (frontmatter, file structure, JSON)
4. **Write**: Creates/updates files only if content changed (SHA-256 hash comparison)

## License

MIT
