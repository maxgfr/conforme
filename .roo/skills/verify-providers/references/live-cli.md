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
    absent from it) and the loaded `AGENTS.md`.
- **OpenCode** (`opencode`):
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
- **Amp** (`npx -y @sourcegraph/amp@latest`, `HOME=<tmp>`): `amp mcp list`
  lists workspace servers. `amp skill list` needs a login.
- **Not scriptable without an install or a login**: Kiro (`kiro-cli`), Cursor
  (`cursor-agent`), Devin, Zed, Copilot CLI. Name them as not verified in the
  report.
