# GitHub Copilot

> GitHub's AI coding assistant. Source: `--from copilot`

**Last verified online:** 2026-10-09, against Copilot CLI 1.0.94 and VS Code 1.141 (https://github.com/github/copilot-cli/blob/main/changelog.md, https://code.visualstudio.com/updates)

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
- Custom instructions support (which surface reads which file): https://docs.github.com/en/copilot/reference/custom-instructions-support
- Customization cheat sheet (paths for instructions, agents, skills, MCP): https://docs.github.com/en/copilot/reference/customization-cheat-sheet
- Copilot CLI command reference (`instruction list`, `mcp list`, `skill list`): https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference
- Copilot CLI config directory (`~/.copilot`, precedence, `disabledSkills`): https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference
- Copilot CLI custom agents (paths, `dir--name` ids): https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/create-custom-agents-for-cli
- Configure Copilot CLI (trusted folders, `COPILOT_HOME`): https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/configure-copilot-cli
- VS Code AI settings (`chat.useAgentsMdFile`, `chat.useClaudeMdFile`, discovery): https://code.visualstudio.com/docs/agents/reference/ai-settings
- VS Code: migrate customizations (`.vscode/mcp.json` to `.mcp.json`): https://code.visualstudio.com/docs/agent-customization/migrate-customizations
- VS Code trust and safety (MCP servers follow Workspace Trust): https://code.visualstudio.com/docs/agents/concepts/trust-and-safety
- VS Code troubleshooting (Chat Diagnostics lists loaded instruction files): https://code.visualstudio.com/docs/agents/agent-troubleshooting/troubleshooting

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
- Detection: `.github/copilot-instructions.md`, or a `.github/instructions/`, `.github/agents/` or `.github/skills/` directory
- `read()` round-trips instructions/rules plus skills (`.github/skills/*/SKILL.md`), subagents (`.github/agents/*.agent.md`), and MCP (`.vscode/mcp.json`)

## Notes

- MCP uses `"servers"` key (NOT `"mcpServers"`) -- unique among all tools
- MCP: `env` on stdio servers and `headers` on HTTP ones (VS Code's remote fields are `type`, `url`, `headers`, `oauth`), which is what conforme writes. Environment references are written `${env:VAR}` (VS Code's predefined-variable syntax) and read back as `${VAR}`; `${input:…}` references are kept as they are
- MCP: conforme maps every remote server to its HTTP transport, so a `type: "sse"` server is written as `http` (known gap)
- Agent-decision and manual rules are inlined into `copilot-instructions.md` (deliberate): VS Code can load a `description`-only instructions file on demand, but GitHub.com and Copilot CLI only apply files with `applyTo`. On read, a hand-written file with a `description` and no `applyTo` becomes an agent-decision rule, and one with neither a manual rule
- `applyTo` is written comma-separated without spaces (the documented form), with brace groups expanded, and read back brace-aware
- Agent `model` is a string or an array; VS Code matches it by display name such as `GPT-5.2 (copilot)`, and ignores an unknown value ("Unknown model '{0}' will be ignored", `promptValidator.ts`), so the subagent keeps the parent model. conforme leaves out another host's alias (`sonnet`, `opus`, `haiku`, `fable`, `pro`, `flash`, `flash-lite`), any `provider/model` id and `inherit`, and copies other values as they are
- VS Code 1.140 (2026-09-30) lists `.vscode/mcp.json` as deprecated in favour of the portable workspace `.mcp.json` (`mcpServers`). VS Code still reads it; conforme keeps writing it until the move can be coordinated with the Claude Code target, which owns `.mcp.json`
- `.vscode/mcp.json` also holds VS Code's `inputs` (prompted secrets referenced as `${input:…}`) and `sandbox` settings, and VS Code parses it as JSONC. conforme **merges** only the `servers` key: comments, `inputs`, `sandbox`, and per-server keys it never emits (`envFile`, …) survive, a file it cannot parse is left untouched (sync fails), and `remove copilot` / `migrate --source copilot` keep the file
- Copilot CLI reads MCP servers from `.mcp.json` or `.github/mcp.json` (`mcpServers` key), never `.vscode/mcp.json` (checked live with `copilot mcp list`, Copilot CLI 1.0.94: with only `.vscode/mcp.json` it reports no server). conforme therefore also merges the servers into `.github/mcp.json` in the Claude Code shape (`type: stdio/http`, `${VAR}`); a server present in both `.mcp.json` and `.github/mcp.json` is listed once. `read()` takes `.vscode/mcp.json` first, then any server only `.github/mcp.json` holds
- The Copilot cloud agent (and Copilot code review) reads no MCP file from the repository: its servers are entered as JSON in the repository settings on GitHub.com, with secrets named `COPILOT_MCP_*` (https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/configure-mcp-servers). conforme cannot write that configuration
- VS Code now calls `.vscode/mcp.json` deprecated and prefers the portable project `.mcp.json` (`mcpServers`, the file Claude Code reads); it still reads `.vscode/mcp.json` for compatibility (https://code.visualstudio.com/docs/agent-customization/mcp-servers). conforme keeps writing `.vscode/mcp.json` (known gap: moving VS Code to `.mcp.json` means sharing that file with the Claude Code target)
- `.github/instructions/` may be organised in sub-directories; conforme reads nested `*.instructions.md` too (written back flat)
- Skills and agents are always written with a `description` (required upstream), falling back to the name
- Since CLI 1.0.89 (2026-09-28) Copilot CLI also reads `.claude/rules/`; since 1.0.86 agents accept `include-custom-instructions`, which conforme does not generate
- **Skills moved to `.github/skills/`**: GitHub documents Copilot skills as `SKILL.md` folders under `.github/skills/` (also `.claude/skills/` and `.agents/skills/`), for both Copilot CLI and the cloud agent. prompt files are a distinct, user-authored VS Code feature, so conforme writes `.github/skills/` only and never reads, writes, or cleans `.github/prompts/`. Anything you keep there is left untouched by orphan cleanup
- Prompts have optional `agent` field (values: `ask`, `edit`, `agent`, `plan`, or custom agent name)
- Additional optional fields on instructions: `name`, `description`, `excludeAgent`
- Additional optional fields on agents: `target` (`vscode` / `github-copilot`, both when unset), `agents`, `argument-hint`, `handoffs`, `hooks`, `mcp-servers`, `metadata`, `user-invocable`, `disable-model-invocation`, CLI `reasoning-effort` and `model-policy`; conforme does not generate them
- VS Code skills also accept `argument-hint`, `user-invocable`, `disable-model-invocation`, `context`, `license` and `allowed-tools`; VS Code honours `disable-model-invocation`, so a manual skill conforme writes stays manual there
- `handoffs` is not supported on the Copilot cloud agent on GitHub.com (CLI only)
- Agent files are accepted as `<name>.agent.md` **or** plain `<name>.md`; the filename minus extension is what deduplicates an agent across config levels. conforme writes the `.agent.md` form, and an agent file without a `name` reads back as `<name>` (the whole `.agent.md` suffix is stripped). Hand-written plain `.md` agents in `.github/agents/` are never deleted by orphan cleanup
- `infer` is a retired agent field, superseded by `disable-model-invocation`; conforme emits neither
- Agent prompt bodies are capped at 30,000 characters upstream
- Copilot now has hooks support (CLI and cloud agent) -- not synced by conforme
- Copilot (cloud agent, CLI and VS Code) also loads `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` and `.claude/rules/`, so in a project that keeps those files (or syncs Claude Code / Gemini CLI too) the instructions reach the model more than once
