# Plugins

A plugin is a named, versioned package of rules, skills, subagents, MCP
servers (and, depending on the tool, commands and hooks) that a tool loads
together. conforme models one as `NormalizedPlugin` (`src/config.rs`), reads it
from AGENTS.md (`## Plugin:` sections, see the README) or from a tool's own
bundles, and writes it to every tool.

Last verified online: 2026-11-12 (Claude Code 2.1.322, Mistral Vibe 3.1.0 and
3.1.2 source, Antigravity CLI 1.5.3).

## Bundles: Claude Code, Mistral Vibe, Antigravity CLI

These three tools load a plugin bundle committed in the project. conforme
reads theirs and writes one per plugin in each (`plugins::PluginShape`,
`generate_plugins`, `read_plugins`):

| | Claude Code | Mistral Vibe | Antigravity CLI |
|---|---|---|---|
| Bundle folder | `.claude/skills/<plugin>/` | `.vibe/plugins/<plugin>/` | `.agents/plugins/<plugin>/` |
| Manifest | `.claude-plugin/plugin.json` (`author` an object) | `plugin.json`, Agent Plugins 1.0 `$schema`, no other top-level field | `plugin.json` with its own `$schema` (`.antigravity-plugin/` and `.gemini-plugin/` read too) |
| Skills | `skills/<name>/SKILL.md`, and `commands/*.md` read as skills | `skills/<name>/SKILL.md` | `skills/<name>/SKILL.md` |
| Agents | `agents/<name>.md` | `ai.mistral.vibe/agents/<name>.toml` (`schema_version = 1`; the manifest gets the `ai.mistral.vibe` block) | `agents/<name>.md` |
| Rules and instructions | none: they become the tool's own rules | none: they become the tool's own rules | `rules/<name>.md`, instructions in `rules/AGENTS.md` |
| MCP servers | `.mcp.json` | `mcp.json` (`$schema`, `stdio` / `streamable-http`; SSE written as streamable HTTP, warned) | `mcp_config.json` |
| Off switch | manifest `defaultEnabled: false`; the project's `enabledPlugins["<plugin>@skills-dir"]` read over it | `disabled_plugins` in `.vibe/config.toml` (Vibe 3.1.2 and later; an older one loads it, warned), `enabled_plugins` read | manifest `disabled: true` |
| Sources | [loading](https://code.claude.com/docs/en/plugins/loading.md), [reference](https://code.claude.com/docs/en/plugins-reference) | [plugins](https://docs.mistral.ai/vibe/code/cli/plugins), [`_enablement.py`](https://github.com/mistralai/mistral-vibe/blob/main/vibe/core/plugins/_enablement.py) | [build plugins](https://antigravity.google/docs/build-plugins/), [schema](https://antigravity.google/schemas/v1/plugin.json) |

The bundle's other text files (scripts, a README) are carried as they are;
files that are not UTF-8 text are not.

### Hooks in bundles

A bundle's hooks (`NormalizedHook`: event and matcher in Claude Code's
vocabulary, command, timeout) are translated into each bundle format
(`src/hooks.rs`):

| | Claude Code | Mistral Vibe | Antigravity CLI |
|---|---|---|---|
| File | `hooks/hooks.json`, `{"hooks": {Event: [{matcher, hooks: [{type: "command", command, timeout}]}]}}` ([components](https://code.claude.com/docs/en/plugins/components.md)) | `ai.mistral.vibe/hooks.toml`, `[[hooks]]` `name`, `type`, `match`, `command`, `timeout` (the manifest gets the `ai.mistral.vibe` block) | `hooks.json`, `{"conforme": {"PreToolUse": [{matcher, hooks: [...]}], "SessionStart": [{type, command, timeout}]}}` ([hooks](https://antigravity.google/docs/hooks/)) |
| Events | all | `PreToolUse` → `pre_tool`, `PostToolUse` → `post_tool`, `Stop` → `post_agent` (`HookType`, `vibe/core/hooks/models.py`, the mapping Vibe applies to a Claude Code plugin's hooks) | `PreToolUse`, `PostToolUse`, `SessionStart`, `Stop` |
| Matcher | as written | each tool name in Vibe's (`Bash` → `bash`, `Edit\|Write` → `re:^(edit\|write_file)$`) | each tool name in Antigravity's, as `agy plugin validate` names them (`Bash` → `run_command`, `Read` → `view_file\|read_file`, …) |
| Plugin root | `${CLAUDE_PLUGIN_ROOT}` | `${PLUGIN_ROOT}` (set for plugin hooks) | `${PLUGIN_ROOT}` (set in hook environments since 1.5.3) |

A hook on an event the format lacks, or whose matcher names a tool it has no
equivalent for (`TodoWrite`, an `mcp__…` pattern), is left out with a warning.
A hook script receives each tool's own JSON on stdin and blocks its own way
(Vibe: printing a `deny` decision, a non-zero exit being a warning unless
`strict`; Antigravity: printing a `decision`), so Vibe and Antigravity warn
that a translated guard must handle their payload. Codex bundles hold no hooks
(`plugin_hooks` is `removed` in Codex CLI 0.167.2), nor does any tool without
bundles: `sync` names the hooks they do not get. The translated hooks pass
`agy plugin validate --strict` and load in Vibe 3.1.0's resolver.

Each bundle conforme writes holds a `.conforme-plugin` marker, so it is kept in
step with the source and deleted once its plugin leaves it. A bundle folder
without the marker was written by hand: conforme never writes over it nor
deletes it, and warns that the plugin of that name is not written there. In
`.agents/plugins/` only bundle folders are conforme's: Codex's repository
marketplace (`marketplace.json`) beside them is left alone, and
`gitignore install` ignores `.agents/plugins/*/`, not the whole folder.

In Claude Code a skill and a plugin of the same name would share one folder:
the plugin is then not written there (warned). A folder holding both a root
`SKILL.md` and a manifest, which Claude Code loads as a skill and as a plugin,
is read as both, and the skill elsewhere carries none of the plugin's files.

`migrate` carries a plugin's content to its output as plain items, and never
deletes a source bundle: its hooks, scripts and the items that need its folder
have nowhere else to go.

The generated bundles were checked with `claude plugin validate --strict`,
`agy plugin validate --strict` and Vibe 3.1.0's own plugin resolver.

## Codex: a repository marketplace

Codex loads no bundle folder by itself, but discovers a marketplace at the
repository root (`.agents/plugins/marketplace.json`, also
`.claude-plugin/marketplace.json` and `.cursor-plugin/marketplace.json`:
`MARKETPLACE_MANIFEST_RELATIVE_PATHS` in
[`codex-rs/core-plugins/src/marketplace.rs`](https://github.com/openai/codex/blob/main/codex-rs/core-plugins/src/marketplace.rs))
and installs the plugins of it that the trusted project's `.codex/config.toml`
enables ([plugins](https://developers.openai.com/plugins/build/plugins)). A
session looks it up from its working directory; `codex plugin list` passes no
project root, so it does not show it. conforme writes, per plugin:

- the bundle in `.codex/plugins/<plugin>/` (`plugins::CODEX_PLUGINS`): an Agent
  Plugins 1.0 `plugin.json`, `skills/` and an Agent Plugins `mcp.json` (`$schema`,
  `type` per server; Codex has no SSE, so an SSE server is streamable HTTP).
  Codex gives `.codex/plugins/` no other meaning (its cache is
  `$CODEX_HOME/plugins/cache/`). A Codex plugin holds no subagents nor rules,
  so Codex gets those as its own agents and through AGENTS.md;
- an entry in `.agents/plugins/marketplace.json`: `name`, `source`
  `{"source": "local", "path": "./.codex/plugins/<plugin>"}` (resolved from the
  repository root, `./` required), `policy` `AVAILABLE` / `ON_INSTALL` and a
  `category`, the entries sorted by name. The file is merged: the user's own
  entries, its `name` and `interface` are kept (a new file is named
  `conforme`), and an entry the user gave the plugin's name wins;
- `[plugins."<plugin>@<marketplace>"] enabled = true` (`false` for a disabled
  plugin) in `.codex/config.toml`, merged like its MCP servers. Codex reads it
  only in a trusted project, which `sync` warns about.

Only entries pointing at a bundle of `.codex/plugins/` that conforme wrote (or
that is gone) are conforme's, and only the tables of plugins it listed: when a
plugin leaves the source, its bundle, entry and table go, nothing else. A
source with no plugin leaves both files untouched. The output was checked with
Codex CLI 0.167.2: `codex plugin marketplace add <project>` lists the plugin
and `codex plugin add` installs it.

GitHub Copilot can also install plugins from a marketplace a repository
declares (`.github/copilot/settings.json` `extraKnownMarketplaces` +
`enabledPlugins`), but the fields of a `directory` marketplace source are not
documented ([config directory](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference)),
so conforme does not write one: Copilot gets a plugin's content as its own
skills, agents and MCP servers, which it loads today.

## Plugins enabled from a marketplace

A project can also enable a plugin it does not hold, from a marketplace
(`NormalizedPluginRef`, id `<plugin>@<marketplace>`), and declare that
marketplace (`NormalizedMarketplace`, its source in Claude Code's shape). In
AGENTS.md:

```markdown
## Plugin: formatter
<!-- marketplace: team -->

## Marketplace: team
<!-- github: acme/tools -->
<!-- ref: v2 -->
```

A marketplace is `github: owner/repo`, `git: <url>` or `directory: <path>`
(`ref` and `path` for a sub-folder of a repository), or `source:` the whole
source object in JSON. `<!-- disabled: true -->` lists the plugin switched off.

| Tool | Where | Notes |
|---|---|---|
| Claude Code | `.claude/settings.json` `enabledPlugins` + `extraKnownMarketplaces` ([settings](https://code.claude.com/docs/en/settings-reference.md), [marketplaces](https://code.claude.com/docs/en/plugins/marketplace-reference.md)) | the `<plugin>@skills-dir` switches of its bundles stay apart: they are bundles', not marketplace plugins |
| GitHub Copilot | `.github/copilot/settings.json`, same keys ("auto-installs and activates in the declaring repository", [config directory](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference)) | Copilot CLI also reads Claude Code's file; writing its own is harmless, the same plugin is enabled once |
| Codex | `[plugins."<plugin>@<marketplace>"] enabled` in `.codex/config.toml` | Codex clones no marketplace a project names (only OpenAI's own, `startup_sync.rs`; a configured Git marketplace lives in `$CODEX_HOME/marketplaces/<name>`, `installed_marketplaces.rs`): `sync` warns with the `codex plugin marketplace add` each user runs. The tables of its repository marketplace (conforme's bundles) are left to that |
| every other tool | none | `sync` names the plugins it does not enable there (Devin's `requiredPlugins` names plugin repositories, not a marketplace's plugins) |

The files are merged: every other key stays, a marketplace entry keeps its own
keys besides `source` (`autoUpdate`). The source decides the list: a plugin it
no longer enables is dropped from the targets. A source with no marketplace
plugin leaves the files untouched, and one with plugins but no marketplace
(Codex) leaves `extraKnownMarketplaces` alone.

## Tools without bundles

Every other tool gets a plugin's content as its own configuration
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
