# GitHub Copilot

> GitHub's AI coding assistant. Source: `--from copilot`

## Official docs

- Custom instructions: https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/add-custom-instructions/add-repository-instructions
- Custom agents config: https://docs.github.com/en/copilot/reference/custom-agents-configuration
- CLI skills: https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-skills
- Cloud agent skills: https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/customize-cloud-agent/add-skills
- MCP tutorial: https://docs.github.com/en/copilot/tutorials/enhance-agent-mode-with-mcp
- MCP config (VS Code): https://code.visualstudio.com/docs/agents/reference/mcp-configuration
- Custom instructions (VS Code): https://code.visualstudio.com/docs/agent-customization/custom-instructions
- Custom agents (VS Code): https://code.visualstudio.com/docs/agent-customization/custom-agents
- Agent skills (VS Code): https://code.visualstudio.com/docs/agent-customization/agent-skills
- MCP servers (VS Code): https://code.visualstudio.com/docs/agent-customization/mcp-servers
- CLI custom instructions: https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-custom-instructions
- MCP servers (Copilot CLI): https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-mcp-servers
- CLI changelog: https://github.com/github/copilot-cli/blob/main/changelog.md
- Hooks config: https://docs.github.com/en/copilot/reference/hooks-reference
- CLI hooks: https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-hooks
- CLI overview: https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/overview
- About Copilot: https://docs.github.com/en/copilot/get-started/about-github-copilot

## Config files

| Feature | Path | Format |
|---------|------|--------|
| Instructions (global) | `.github/copilot-instructions.md` | Plain markdown (always applied) |
| Instructions (per-file) | `.github/instructions/**/<name>.instructions.md` | YAML frontmatter: `applyTo` (glob) |
| Skills | `.github/skills/<name>/SKILL.md` | YAML frontmatter: `name`, `description` (required), `license`, `allowed-tools` |
| Prompts | `.github/prompts/<name>.prompt.md` | YAML frontmatter: `description`, `tools`, `agent` (VS Code prompt files — **not** skills) |
| Agents | `.github/agents/<name>.agent.md` | YAML frontmatter: `name`, `description`, `model`, `tools` |
| MCP (VS Code) | `.vscode/mcp.json` | JSON (JSONC): `{ "servers": { ... }, "inputs": [...] }` (**not** `mcpServers`) |
| MCP (Copilot CLI) | `.mcp.json` or `.github/mcp.json` | JSON: `{ "mcpServers": { ... } }` — the CLI has not read `.vscode/mcp.json` since 1.0.22 |

## Activation modes

| Mode | Implementation |
|------|---------------|
| Always | Content in `.github/copilot-instructions.md` (main file) |
| GlobMatch | `.github/instructions/<name>.instructions.md` with `applyTo: "**/*.ts"` |
| AgentDecision | Content in main file (VS Code alone loads a `description`-only file on demand) |
| Manual | Content in main file (VS Code alone lets a file with neither field be attached by hand) |

## conforme adapter

- File: `src/adapters/copilot.rs`
- ID: `copilot`
- Capabilities: activation_modes, skills, agents, MCP
- Always/AgentDecision/Manual rules -> inlined in `copilot-instructions.md`
- GlobMatch rules -> separate `.instructions.md` files
- Skills -> `.github/skills/<name>/SKILL.md`
- `read()` round-trips instructions/rules plus skills (`.github/skills/*/SKILL.md`), subagents (`.github/agents/*.agent.md`), and MCP (`.vscode/mcp.json`)

## Notes

- MCP uses `"servers"` key (NOT `"mcpServers"`) -- unique among all tools
- MCP: `env` on stdio servers and `headers` on HTTP ones (VS Code's remote fields are `type`, `url`, `headers`, `oauth`), which is what conforme writes. Environment references are written `${env:VAR}` (VS Code's predefined-variable syntax) and read back as `${VAR}`; `${input:…}` references are kept as they are
- Agent-decision and manual rules are inlined into `copilot-instructions.md` (deliberate): VS Code can load a `description`-only instructions file on demand, but GitHub.com and Copilot CLI only apply files with `applyTo`. On read, a hand-written file with a `description` and no `applyTo` becomes an agent-decision rule, and one with neither a manual rule
- `applyTo` is written comma-separated without spaces (the documented form), with brace groups expanded, and read back brace-aware
- Agent `model` is copied as is; VS Code expects names such as `GPT-5.2 (copilot)` and its docs do not say what an unknown value does (unverified)
- VS Code 1.140 (2026-09-30) lists `.vscode/mcp.json` as deprecated in favour of the portable workspace `.mcp.json` (`mcpServers`). VS Code still reads it; conforme keeps writing it until the move can be coordinated with the Claude Code target, which owns `.mcp.json`
- `.vscode/mcp.json` also holds VS Code's `inputs` (prompted secrets referenced as `${input:…}`) and `sandbox` settings, and VS Code parses it as JSONC. conforme **merges** only the `servers` key: comments, `inputs`, `sandbox`, and per-server keys it never emits (`envFile`, …) survive, a file it cannot parse is left untouched (sync fails), and `remove copilot` / `migrate --source copilot` keep the file
- Copilot CLI reads MCP servers from `.mcp.json` or `.github/mcp.json` (`mcpServers` key, added 1.0.61), not `.vscode/mcp.json`. conforme does not write a Copilot-specific CLI file; when Claude Code is also a target, its `.mcp.json` covers the CLI
- `.github/instructions/` may be organised in sub-directories; conforme reads nested `*.instructions.md` too (written back flat)
- Skills and agents are always written with a `description` (required upstream), falling back to the name
- Since CLI 1.0.89 (2026-09-28) Copilot CLI also reads `.claude/rules/`; since 1.0.86 agents accept `include-custom-instructions`, which conforme does not generate
- **Skills moved to `.github/skills/`**: GitHub documents Copilot skills as `SKILL.md` folders under `.github/skills/` (also `.claude/skills/` and `.agents/skills/`), for both Copilot CLI and the cloud agent. prompt files are a distinct, user-authored VS Code feature, so conforme writes `.github/skills/` only and never reads, writes, or cleans `.github/prompts/`. Anything you keep there is left untouched by orphan cleanup
- Prompts have optional `agent` field (values: `ask`, `edit`, `agent`, `plan`, or custom agent name)
- Additional optional fields on instructions: `name`, `description`, `excludeAgent`
- Additional optional fields on agents: `handoffs`, `mcp-servers`, `target` (`vscode` / `github-copilot`, both when unset), `user-invocable`, `disable-model-invocation`, `metadata`
- `handoffs` is not supported on the Copilot cloud agent on GitHub.com (CLI only)
- Agent files are accepted as `<name>.agent.md` **or** plain `<name>.md`; the filename minus extension is what deduplicates an agent across config levels. conforme writes the `.agent.md` form, and an agent file without a `name` reads back as `<name>` (the whole `.agent.md` suffix is stripped). Hand-written plain `.md` agents in `.github/agents/` are never deleted by orphan cleanup
- `infer` is a retired agent field, superseded by `disable-model-invocation`; conforme emits neither
- Agent prompt bodies are capped at 30,000 characters upstream
- Copilot now has hooks support (CLI and cloud agent) -- not synced by conforme
