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

## How it works

1. **Parse**: Reads `AGENTS.md` and extracts instructions + rules with activation metadata
2. **Detect**: Scans for tool-specific directories (`.cursor/`, `.windsurf/`, `.kiro/`, etc.)
3. **Generate**: Converts normalized rules to each tool's format (frontmatter, file structure)
4. **Write**: Creates/updates files only if content changed (SHA-256 hash comparison)

## License

MIT
