<!-- Intended scope: **/*.rs -->

- Run `cargo clippy --all-targets -- -D warnings` before considering any change complete
- Run `cargo test` after modifying adapter logic, MCP generation, or skills generation
- Use `anyhow::Result` for fallible functions, `anyhow::Context` for error messages
- Use `BTreeMap` (not `HashMap`) for deterministic output ordering in generated files
- Parse frontmatter fields defensively: use `.and_then(|v| v.as_str())` chains, never `.unwrap()` on user input
- When parsing tool-separated lists (e.g. `allowed-tools`, `tools`), use `skills::parse_frontmatter_tool_list`: it handles space- and comma-separated strings and YAML lists, and keeps `Bash(git add:*)` whole
- Keep adapter `read()` and `generate()` symmetric: if `generate()` writes a field, `read()` must parse it back correctly (round-trip guarantee)
