---
description: Verify all 13 provider adapters against their official documentation and fix any discrepancies
tools:
- Read
- Grep
- Glob
- Bash
- WebFetch
- WebSearch
- Edit
- Write
---

Audit every conforme provider adapter against the latest upstream documentation.
For each of the 13 tools, follow these steps:

## 1. Read the adapter code and provider docs

For each tool (claude, cursor, copilot, windsurf, continuedev, kiro, amazonq, codex, opencode, gemini, zed, amp, roocode):

- Read `src/adapters/<tool>.rs`
- Read `docs/providers/<tool>.md`
- Note the official doc URLs listed in the provider doc

## 2. Fetch the latest official documentation

Use WebFetch on each official doc URL from `docs/providers/<tool>.md`.
Extract the current:
- Config file paths and names
- Frontmatter fields and their types
- Activation mode mappings
- Skills/agents/MCP format and file locations
- Any new features or breaking changes

## 3. Compare adapter code vs official docs

Check for discrepancies in:
- **File paths**: Are we writing to the correct locations?
- **Frontmatter fields**: Are we reading/writing all required fields? Using correct separators?
- **MCP format**: Correct JSON key (`mcpServers`, `servers`, `context_servers`, `amp.mcpServers`, `mcp`)? Correct transport types?
- **Skills format**: Correct frontmatter fields per tool?
- **Agents format**: Correct file extension and frontmatter?
- **Activation modes**: Correct mapping values?

## 4. Fix discrepancies

For each issue found:
1. Fix the adapter code in `src/adapters/` or `src/mcp.rs` or `src/skills.rs`
2. Update the provider doc in `docs/providers/`
3. Update `src/help_ai.rs` if the help text is affected
4. Update tests in `tests/` to match new behavior
5. Run `cargo test` to verify

## 5. Verify links

WebFetch each documentation URL to confirm it still resolves (not 404).
If a link is broken, search for the new URL and update `docs/providers/<tool>.md`.

## 6. Final checks

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

Report a summary table:
| Tool | Status | Issues found | Fixed |
