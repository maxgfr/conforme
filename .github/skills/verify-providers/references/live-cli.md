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

Run from the fixture directory. Use an isolated `HOME` where a tool needs
folder trust, so the user's own configuration is never touched.

- **Claude Code** (`claude`):
  - `claude mcp get <name>`: the server, its type, env and headers (project
    servers show as pending approval, which is fine).
  - Skills and agents: copy `.claude/skills` and `.claude/agents` into a
    temporary folder holding `.claude-plugin/plugin.json` (`name`, `version`,
    `description`, `author`), then `claude plugin validate --strict <folder>`.
    Without the manifest the validator checks nothing (`contents: []`).
- **Codex** (`codex`): project config loads only in a trusted project; pass
  the trust inline: `codex -c 'projects={"<abs path>"={trust_level="trusted"}}' …`.
  - `… mcp get <name> --json`: `env_vars`, `bearer_token_env_var`,
    `env_http_headers`.
  - `… debug prompt-input`: the model-visible skills list (a manual skill is
    absent from it) and the loaded `AGENTS.md`, which must hold no skill body
    (grep the manual skill's text: it must not appear).
- **OpenCode** (`opencode`): its output includes every global skill and
  plugin, megabytes that take a minute: redirect it to a file and grep the
  file, with no short `timeout` (a cut-off pipe reads as "nothing loaded").
  - `opencode debug config`: resolved `mcp` (command array, `environment`,
    `{env:VAR}`) and `agent` entries.
  - `opencode debug skill`: each skill with its `location`.
- **Gemini CLI** (`npx -y @google/gemini-cli@<version>`): with
  `HOME=<tmp>` holding `.gemini/trustedFolders.json` (`{"<abs path>":
  "TRUST_FOLDER"}`):
  - `gemini mcp list`: each server and its transport.
  - `gemini skills list`: each skill and the root it loads from; an
    "Agent loading error" line names an agent Gemini rejects; "Skill conflict
    detected" names a skill present in two roots.
- **Copilot CLI** (`npx -y @github/copilot@<version>`): with `HOME=<tmp>` holding `.copilot/config.json`
  (`{"trusted_folders": ["<abs path>"]}`), and the Claude `.mcp.json` removed
  so it is not what makes a server appear:
  - `copilot mcp list`: the servers of `.github/mcp.json` (Copilot CLI ignores
    `.vscode/mcp.json`).
  - `copilot skill list` (under "Project skills") and `copilot instruction list`.
- **Cursor** (`cursor-agent`, `brew install --cask cursor-cli`; uninstall it
  after): `cursor-agent mcp list` lists the project
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
  config with `vibe.core.config.default_orchestrator.build_default_orchestrator`;
  set `HOME` to the temporary directory too, or `~/.agents/skills` leaks in.
  `MCPStdio.argv()` shows the command Vibe will run. When Homebrew lags a
  release, install it in a throwaway venv (`uv pip install mistral-vibe==<v>`).
- **Kilo Code** (`kilo`): with `HOME` and `XDG_*` pointing at a temporary
  directory:
  - `kilo config check`: exit 1, naming the file, when Kilo refuses a project
    config (a `{env:VAR}` in `kilo.jsonc` or the root `opencode.json`).
  - `kilo debug config`: resolved `mcp` and `agent` entries.
  - `kilo debug skill`: each skill with its `location`.
  - `kilo agent list`: a synced agent as `<name> (subagent)`.
  `kilo mcp list` connects to every server; leave it out.
- **Needs a login**: Kiro (`kiro-cli`). **No CLI**: Devin, Zed. Name them as
  not verified in the report.
