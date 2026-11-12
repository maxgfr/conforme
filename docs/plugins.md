# Plugins

A plugin is a named, versioned package of rules, skills, subagents, MCP
servers (and, depending on the tool, commands and hooks) that a tool loads
together. conforme models one as `NormalizedPlugin` (`src/config.rs`), reads it
from AGENTS.md (`## Plugin:` sections, see the README), and writes its content
to every tool.

Last verified online: 2026-11-12.

## What conforme writes

Every tool gets a plugin's content as its own configuration
(`plugins::flatten`):

- the plugin's instructions become an always-on rule named after the plugin;
- its rules, skills, agents and MCP servers keep their names, or become
  `<plugin>-<name>` when the project (or an earlier plugin) already has an
  item of that name; when that is taken too, the item is left out;
- an item that uses the plugin's own folder (`${PLUGIN_ROOT}`,
  `${PLUGIN_DATA}`, `${CLAUDE_PLUGIN_ROOT}`, `${CLAUDE_PLUGIN_DATA}`,
  `${CURSOR_PLUGIN_ROOT}`, `${COPILOT_PLUGIN_ROOT}`) only works inside a
  loaded bundle and is left out;
- a disabled plugin is left out whole, since a tool without bundles has no
  switch to keep it off;
- files of the bundle outside its components have no place outside it.

Each of these cases prints a warning. Two plugins of the same name are an
error, in `sync` and in `conforme add`. A hand-written AGENTS.md is compared
with the source once flattened, so one whose plugin holds a skill the tool
source lacks is refused rather than replaced.

## How each tool loads plugins

Two mechanisms exist in a repository: a **bundle** committed in a folder the
tool loads, and a **marketplace reference** (a setting that enables a plugin
installed from a marketplace).

| Tool | Bundle committed in the project | Marketplace reference committed in the project |
|---|---|---|
| Claude Code | `.claude/skills/<plugin>/.claude-plugin/plugin.json`, loaded as `<plugin>@skills-dir` after the trust check ([loading](https://code.claude.com/docs/en/plugins/loading.md)) | `.claude/settings.json` `enabledPlugins` + `extraKnownMarketplaces` ([settings](https://code.claude.com/docs/en/settings-reference.md), [marketplaces](https://code.claude.com/docs/en/plugins/marketplace-reference.md)) |
| Mistral Vibe | `.vibe/plugins/<plugin>/plugin.json` (Agent Plugins 1.0), trusted folders only ([plugins](https://docs.mistral.ai/vibe/code/cli/plugins)) | none; `enabled_plugins` / `disabled_plugins` in `.vibe/config.toml` switch plugins by name |
| Antigravity CLI | `.agents/plugins/<plugin>/plugin.json` or `.antigravity-plugin/plugin.json` ([build plugins](https://antigravity.google/docs/build-plugins/), [discovery](https://antigravity.google/docs/customization-discovery)) | none |
| Codex | a folder listed in `.agents/plugins/marketplace.json`, enabled by `[plugins."<plugin>@<marketplace>"]` in a trusted `.codex/config.toml` ([plugins](https://developers.openai.com/plugins/build/plugins)) | `[plugins."<name>@<marketplace>"]` + `[marketplaces.<name>]` |
| GitHub Copilot | a folder listed in a repository `marketplace.json` (root, `.plugin/`, `.github/plugin/` or `.claude-plugin/`) ([CLI plugins](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference)) | `.github/copilot/settings.json` `enabledPlugins` + `extraKnownMarketplaces`, also read from `.claude/settings.json` ([config directory](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference)) |
| Devin | none: plugins are installed per user ([plugins](https://docs.devin.ai/cli/extensibility/plugins/overview)) | `.devin/config.json` `requiredPlugins` / `optionalPlugins` / `forbiddenPlugins`, naming plugin repositories, not `name@marketplace` |
| Cursor | none: local plugins live in `~/.cursor/plugins/local/` ([plugins](https://cursor.com/docs/plugins)) | a project scope exists, but its file is undocumented |
| Kiro | none: Powers live in `~/.kiro/powers/` ([configuration](https://kiro.dev/docs/configuration/)) | none |
| Gemini CLI | none: extensions live in `~/.gemini/extensions/` ([extensions](https://github.com/google-gemini/gemini-cli/blob/main/docs/extensions/reference.md)) | none: workspace enablement is stored in `~/.gemini` |
| OpenCode, Kilo Code | plugins are JavaScript/TypeScript modules ([OpenCode](https://opencode.ai/docs/plugins/), [Kilo](https://kilo.ai/docs/automate/extending/plugins)); conforme never reads nor writes them | the `plugins` / `plugin` array of their config, left untouched |
| Zed | extensions are WebAssembly, installed per user ([extensions](https://zed.dev/docs/extensions/developing-extensions)) | none |
| Zoo Code, DeepSeek Harness | no project plugin ([Zoo marketplace](https://docs.zoocode.dev/features/marketplace), [dsh plugin market](https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/user/guide/plugin-market.md)) | none |

The open [Agent Plugins](https://agent-plugins.org) 1.0 format
([spec](https://github.com/agentplugins/agent-plugins-spec/blob/main/spec/1.0.0.md)),
loaded by Codex, Cursor, Copilot, Kiro, Devin and Vibe, defines a root
`plugin.json` (`$schema` and `name` required), `skills/<name>/SKILL.md` and an
`mcp.json`; any other component lives in a vendor folder
(`com.github.copilot/`, `ai.mistral.vibe/`).
