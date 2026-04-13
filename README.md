# conforme

Universal AI coding agent config synchronization. Treats [AGENTS.md](https://github.com/agentsmd/agents.md) as the source of truth and syncs to all tool-specific formats.

## Install

```bash
# Homebrew
brew install maxgfr/tap/conforme

# From source
cargo install --path .

# Pre-built binaries
# Download from GitHub Releases
```

## Supported tools

| Tool | Config format | Status |
|------|--------------|--------|
| Claude Code | `CLAUDE.md` + `.claude/rules/*.md` | Supported |
| Cursor | `.cursor/rules/*.mdc` | Supported |
| Windsurf | `.windsurf/rules/*.md` | Supported |
| GitHub Copilot | `.github/copilot-instructions.md` + `.github/instructions/` | Supported |
| OpenAI Codex CLI | `AGENTS.md` (native) | Supported |
| OpenCode | `AGENTS.md` (native) | Supported |
| Roo Code / Cline | `.roo/rules/*.md` | Supported |
| Gemini CLI | `GEMINI.md` | Supported |
| Continue.dev | `.continue/rules/*.md` | Supported |
| Zed AI | `.rules` | Supported |
| Amazon Q | `.amazonq/rules/*.md` | Supported |

## Quick start

```bash
# Initialize in your project
conforme init

# Edit AGENTS.md with your instructions and rules
# Then sync to all detected tools
conforme sync

# Check if configs are in sync (for CI)
conforme check

# See status
conforme status
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

| Mode | Syntax | Description |
|------|--------|-------------|
| Always | `<!-- activation: always -->` | Applied in every session |
| Glob | `<!-- activation: glob **/*.ts -->` | Applied when matching files are in context |
| Agent Decision | `<!-- activation: agent-decision -->` | Agent decides based on description |
| Manual | `<!-- activation: manual -->` | Only when explicitly referenced |

## License

MIT
