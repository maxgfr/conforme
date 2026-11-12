# conforme-dev

A Claude Code plugin for working on conforme, kept in this repository as an
example of plugin sync: Claude Code loads it from `.claude/skills/conforme-dev/`
(as `conforme-dev@skills-dir`), and `conforme sync` writes it to every other
tool. Mistral Vibe, Antigravity CLI and Codex get it as a bundle of their own
format, and the other tools get its skill and subagent as their own.

- `skills/pre-push`: the checks conforme's CI runs, before a push.
- `agents/docs-sync-reviewer`: checks a change against the docs it must keep
  in step.

Edit it here; the other tools' copies are generated.
