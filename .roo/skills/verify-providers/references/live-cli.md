# Live checks with the tools' own CLIs

Run these on a fixture conforme generated (step 6 of the skill) so the tool
itself, not conforme's reading of its docs, says what it loads. None of them
calls a model. Record each tool as **verified** (with the command), **not
installed**, or **needs login**; never log in on the user's behalf.

## Fixture

An `AGENTS.md` (or a tool source) holding: instructions; one glob rule; a
skill with a bundled `scripts/` file; a manual skill (`<!-- invocation: manual
-->`); an agent with `model` and `tools`; a stdio MCP server with an env
`${TOKEN}` reference; an HTTP MCP server with `Authorization: Bearer
${API_TOKEN}`. Create every tool's directory, run `conforme sync`, then
`scripts/skills_conformity.py <fixture>`.

## Witness

A validator that reports nothing may have checked nothing. Next to the
generated files, add one deliberately broken entry (an agent without
`description`, a Gemini agent with an unknown tool name), run the check, and
keep the result only if the witness is flagged. Remove the witness after.

## Per tool

Run from the fixture directory. Isolate each tool where it needs folder trust,
so the user's own configuration is never touched. Prefer the tool's own
variable (`CODEX_HOME`, `GEMINI_CLI_HOME`, `COPILOT_HOME`, `CURSOR_CONFIG_DIR`,
`VIBE_HOME`): a session isolated in a git worktree refuses a command that sets
`HOME`, because it redirects git's configuration too. Do not work around that
refusal. A tool that only honours `HOME` is reported as **not verified** with
the commands for the user to run, unless its checks are read-only.

- **Claude Code** (`claude`):
  - `claude mcp get <name>`: the server, its type, env and headers (project
    servers show as pending approval, which is fine).
  - Skills and agents: copy `.claude/skills` and `.claude/agents` into a
    temporary folder holding `.claude-plugin/plugin.json` (`name`, `version`,
    `description`, `author`), then `claude plugin validate --strict <folder>`.
    Without the manifest the validator checks nothing (`contents: []`).
- **Codex** (`codex`, with `CODEX_HOME=<tmp>`): project config loads only in a
  trusted project; pass the trust inline:
  `codex -c 'projects={"<abs path>"={trust_level="trusted"}}' …`.
  - `… mcp get <name> --json`: `env_vars`, `bearer_token_env_var`,
    `env_http_headers`.
  - `… debug prompt-input`: the model-visible skills list (a manual skill is
    absent from it) and the loaded `AGENTS.md`, which must hold no skill body
    (grep the manual skill's text: it must not appear).
- **OpenCode** (`opencode`, OpenCode 2): the debug commands talk to a
  background service and reject `--standalone`, so without a scratch `HOME` they
  would use the user's own service: report it as not verified then. Under the
  scratch `HOME`/`XDG_*`, run `opencode service set port <free port>` first (another
  service may hold the default one) and `opencode service stop` at the end.
  The first call for a new directory may come back empty while the project
  loads: run it again.
  - `opencode debug config`: each config source with its parsed content
    (`mcp.servers`: command array, `environment`, `{env:VAR}` masked).
  - `opencode debug agents`: every agent with its `mode` and `system`.
  - `opencode api skill.list -H "x-opencode-directory:$PWD"`: each skill with
    its `path`, and `autoinvoke: false` on a manual one (there is no
    `debug skill` any more).
  - `opencode mcp list`: each server and its connection status.
- **Gemini CLI** (`npx -y @google/gemini-cli@<version>`): with
  `GEMINI_CLI_HOME=<tmp>` holding `.gemini/trustedFolders.json` (`{"<abs path>":
  "TRUST_FOLDER"}`):
  - `gemini mcp list`: each server and its transport.
  - `gemini skills list`: each skill and the root it loads from; an
    "Agent loading error" line names an agent Gemini rejects; "Skill conflict
    detected" names a skill present in two roots.
- **Copilot CLI** (`npx -y @github/copilot@<version>`): with `COPILOT_HOME=<tmp>` holding `config.json`
  (`{"trusted_folders": ["<abs path>"]}`), and the Claude `.mcp.json` removed
  so it is not what makes a server appear:
  - `copilot mcp list`: the servers of `.github/mcp.json` (Copilot CLI ignores
    `.vscode/mcp.json`).
  - `copilot skill list` (under "Project skills") and `copilot instruction list`.
- **Cursor** (`cursor-agent`, `brew install --cask cursor-cli`; uninstall it
  after; with `CURSOR_CONFIG_DIR=<tmp>`): `cursor-agent mcp list` lists the project
  servers (a new one waits for approval, which is fine). Skills and agents have
  no listing command: check the files.
- **Mistral Vibe** (`vibe`): project files load only in a trusted folder. Load
  the project with Vibe's own loaders, in the Python of the installed package,
  run isolated (`-I`), with `VIBE_HOME=<tmp>` holding `trusted_folders.toml`
  (`trusted = ["<abs path>"]`): `ConfigOrchestrator` for `mcp_servers`,
  `SkillManager` for skills (and their model-invocation flag and parse
  issues), `AgentRegistry` for subagents, `HarnessFilesManager.load_project_docs`
  for `AGENTS.md`. Call `init_harness_files_manager("user", "project")`
  (`vibe.core.config.harness_files._harness_manager`) first and build the
  config with `asyncio.run(vibe.core.config.default_orchestrator.build_default_orchestrator()).config`.
  `SkillManager(lambda: config).available_skills` holds each skill, with
  `model_invocable = False` on a manual one; `load_project_docs()` takes no
  argument. Skills of the user's `~/.agents/skills` are listed too unless `HOME`
  is temporary: ignore them.
  `MCPStdio.argv()` shows the command Vibe will run. When Homebrew lags a
  release, install it in a throwaway venv (`uv pip install mistral-vibe==<v>`).
- **Kilo Code** (`kilo`): with `HOME` and `XDG_*` pointing at a temporary
  directory, or the real one when that is refused (these commands only read;
  the user's own skills are listed too, filter on the fixture path):
  - `kilo config check`: exit 1, naming the file, when Kilo refuses a project
    config (a `{env:VAR}` in `kilo.jsonc` or the root `opencode.json`).
  - `kilo debug config`: resolved `mcp` and `agent` entries.
  - `kilo debug skill`: each skill with its `location`.
  - `kilo agent list`: a synced agent as `<name> (subagent)`.
  `kilo mcp list` connects to every server; leave it out.
- **Antigravity CLI** (`agy`): `agy mcp list` and `agy agents` list only the
  user configuration, so "No MCP servers configured." proves nothing about the
  project's `.agents/`. Check the files and name it as not verified.
- **Needs a login**: Kiro (`kiro-cli`). **No CLI**: Devin, Zed, Zoo Code.
  **No check defined yet**: DeepSeek Harness (`dsh`, Homebrew
  `deepseek-harness`). Name them as not verified in the report.
