use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::Path;

use crate::adapters;
use crate::project_config;

const BLOCK_START: &str = "# ── conforme: generated tool configs ──";
const BLOCK_END: &str = "# ── end conforme ──";

/// Patterns for the files each adapter generates and owns outright.
///
/// - Root files are anchored (`/CLAUDE.md`), so a hand-written `CLAUDE.md`
///   or `AGENTS.md` in a sub-directory stays tracked.
/// - Rules and agents directories list only the file kind conforme writes at
///   their top level (`.cursor/rules/*.mdc`): nested rules, Kiro `.json`
///   agents or Copilot plain `.md` agents are the user's.
/// - Files conforme only merges into (`.mcp.json`, `.vscode/mcp.json`,
///   `opencode.json`, `.zed/settings.json`, …) also hold the user's own
///   settings and are never ignored.
fn adapter_gitignore_patterns(project_root: &Path, id: &str) -> Vec<String> {
    // Gemini writes its instructions to the context file `context.fileName`
    // names (GEMINI.md by default), or to none.
    if id == "gemini" {
        let mut patterns: Vec<String> = adapters::gemini::instructions_file(project_root)
            .map(|name| format!("/{name}"))
            .into_iter()
            .collect();
        patterns.extend([
            ".gemini/skills/".to_string(),
            ".gemini/agents/*.md".to_string(),
        ]);
        return patterns;
    }
    static_patterns(id).iter().map(|p| p.to_string()).collect()
}

fn static_patterns(id: &str) -> Vec<&'static str> {
    match id {
        "claude" => vec![
            "/CLAUDE.md",
            "/.claude/CLAUDE.md",
            ".claude/rules/*.md",
            ".claude/skills/",
            ".claude/agents/*.md",
        ],
        "cursor" => vec![
            ".cursor/rules/*.mdc",
            ".cursor/skills/",
            ".cursor/agents/*.md",
        ],
        "devin" => vec![".devin/rules/*.md", ".devin/skills/"],
        "copilot" => vec![
            "/.github/copilot-instructions.md",
            ".github/instructions/*.instructions.md",
            ".github/skills/",
            ".github/agents/*.agent.md",
        ],
        "codex" => vec![".agents/skills/", ".codex/agents/*.toml"],
        "opencode" => vec![".opencode/skills/", ".opencode/agents/*.md"],
        "zoocode" => vec![".roo/rules/*.md", ".roo/skills/"],
        "zed" => vec!["/.rules", ".agents/skills/"],
        "kiro" => vec![".kiro/steering/*.md", ".kiro/skills/", ".kiro/agents/*.md"],
        "deepseek" => vec![".dsh/skills/"],
        "vibe" => vec![".vibe/skills/", ".vibe/agents/*.toml"],
        "kilo" => vec![".kilo/skills/", ".kilo/agents/*.md"],
        "antigravity" => vec![
            ".agents/rules/*.md",
            ".agents/skills/",
            ".agents/agents/*.md",
        ],
        _ => vec![],
    }
}

/// Whether sync writes to `id`: `only` and `exclude` respected.
fn is_selected(config: &project_config::ProjectConfig, id: &str) -> bool {
    config
        .only
        .as_ref()
        .is_none_or(|only| only.iter().any(|o| o == id))
        && config
            .exclude
            .as_ref()
            .is_none_or(|exclude| !exclude.iter().any(|e| e == id))
}

/// Build the gitignore block content based on project config.
fn build_gitignore_block(project_root: &Path, config: &project_config::ProjectConfig) -> String {
    let source_id = config.source.as_deref().unwrap_or("agents.md");

    let all = adapters::all_adapters();
    let mut lines = vec![
        BLOCK_START.to_string(),
        format!("# Source: {source_id} — only generated outputs are ignored."),
        "# Managed by `conforme gitignore install`. Do not edit this block.".to_string(),
    ];

    // Collect patterns for the tools sync writes to (`only` / `exclude`
    // respected: an excluded tool's files are the user's). A location the
    // source also uses stays tracked (`.agents/skills/` is Codex's and Zed's),
    // so does a file the source reads (a Kilo Code source reading
    // `CLAUDE.md`), and a pattern another adapter already listed is not
    // repeated.
    let mut listed = adapter_gitignore_patterns(project_root, source_id);
    let source_files: Vec<String> = all
        .iter()
        .find(|a| a.id() == source_id)
        .map(|a| a.source_files(project_root))
        .unwrap_or_default()
        .iter()
        .filter_map(|p| p.strip_prefix(project_root).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    let reads_pattern = |pattern: &str| {
        let pattern = pattern.trim_start_matches('/').trim_end_matches('/');
        source_files.iter().any(|f| f == pattern)
    };
    for adapter in &all {
        if adapter.id() == source_id {
            continue;
        }
        if !is_selected(config, adapter.id()) {
            continue;
        }

        let patterns: Vec<String> = adapter_gitignore_patterns(project_root, adapter.id())
            .into_iter()
            .filter(|p| !listed.contains(p) && !reads_pattern(p))
            .collect();
        if patterns.is_empty() {
            continue;
        }

        lines.push(format!("# {}", adapter.name()));
        for pat in patterns {
            lines.push(pat.clone());
            listed.push(pat);
        }
    }

    // AGENTS.md is generated when using a tool source (not agents.md)
    // — but not when the source tool reads AGENTS.md itself: it is the source.
    let source_reads_agents_md = all
        .iter()
        .any(|a| a.id() == source_id && a.reads_agents_md(project_root));
    if source_id != "agents.md" && !source_reads_agents_md && config.generate_agents_md {
        lines.push("# Generated AGENTS.md".to_string());
        lines.push("/AGENTS.md".to_string());
    }

    lines.push(BLOCK_END.to_string());

    lines.join("\n")
}

/// Install conforme-managed entries into .gitignore.
pub fn install(project_root: &Path, verbose: bool) -> Result<()> {
    let config = project_config::ProjectConfig::load(project_root)?;
    let gitignore_path = project_root.join(".gitignore");
    let block = build_gitignore_block(project_root, &config);

    let (existing, updated) = if gitignore_path.exists() {
        let existing = std::fs::read_to_string(&gitignore_path)
            .with_context(|| format!("failed to read {}", gitignore_path.display()))?;
        let updated = if existing.contains(BLOCK_START) {
            if verbose {
                println!("  Replacing the existing conforme block");
            }
            replace_block(&existing, &block)?
        } else {
            if verbose {
                println!("  Appending the conforme block");
            }
            let mut content = existing.trim_end().to_string();
            if !content.is_empty() {
                content.push_str("\n\n");
            }
            content.push_str(&block);
            content.push('\n');
            content
        };
        (Some(existing), updated)
    } else {
        (None, format!("{block}\n"))
    };

    let source_id = config.source.as_deref().unwrap_or("agents.md");
    if existing.as_deref() == Some(updated.as_str()) {
        println!("{} .gitignore already up to date.", "=".green());
        return Ok(());
    }
    std::fs::write(&gitignore_path, &updated)?;
    println!(
        "{} .gitignore {}.",
        "+".green(),
        if existing.is_some() {
            "updated"
        } else {
            "created"
        }
    );

    let ignored_count = adapters::all_adapters()
        .iter()
        .filter(|a| a.id() != source_id && is_selected(&config, a.id()))
        .count();
    println!(
        "  {} generated tool config(s) ignored (source: {}).",
        ignored_count, source_id
    );

    Ok(())
}

/// Uninstall conforme-managed entries from .gitignore.
pub fn uninstall(project_root: &Path, verbose: bool) -> Result<()> {
    let gitignore_path = project_root.join(".gitignore");

    if !gitignore_path.exists() {
        println!("{} No .gitignore found.", "=".dimmed());
        return Ok(());
    }

    let content = std::fs::read_to_string(&gitignore_path)?;

    if !content.contains(BLOCK_START) {
        println!("{} .gitignore has no conforme-managed block.", "=".dimmed());
        return Ok(());
    }

    let updated = remove_block(&content)?;
    std::fs::write(&gitignore_path, updated)?;

    println!("{} Removed conforme block from .gitignore.", "+".green());
    if verbose {
        println!("  .gitignore cleaned up");
    }

    Ok(())
}

/// Refuse a conforme block whose end marker is missing: everything after its
/// start would be taken for conforme's and dropped, the user's own lines too.
fn check_block_closed(content: &str) -> Result<()> {
    let mut open = false;
    for line in content.lines() {
        match line.trim() {
            BLOCK_START => open = true,
            BLOCK_END => open = false,
            _ => {}
        }
    }
    if open {
        anyhow::bail!(
            ".gitignore has a conforme block that starts with `{BLOCK_START}` but has no \
             `{BLOCK_END}` line; nothing was changed. Add the end line where the block ends \
             (or delete the block), then run the command again."
        );
    }
    Ok(())
}

/// Replace the conforme block in a gitignore string.
fn replace_block(content: &str, new_block: &str) -> Result<String> {
    check_block_closed(content)?;
    let mut result = String::new();
    let mut in_block = false;
    let mut replaced = false;

    for line in content.lines() {
        if line.trim() == BLOCK_START {
            in_block = true;
            if !replaced {
                result.push_str(new_block);
                result.push('\n');
                replaced = true;
            }
            continue;
        }
        if line.trim() == BLOCK_END {
            in_block = false;
            continue;
        }
        if !in_block {
            result.push_str(line);
            result.push('\n');
        }
    }

    Ok(result)
}

/// Remove the conforme block from a gitignore string.
fn remove_block(content: &str) -> Result<String> {
    check_block_closed(content)?;
    let mut result = String::new();
    let mut in_block = false;

    for line in content.lines() {
        if line.trim() == BLOCK_START {
            in_block = true;
            continue;
        }
        if line.trim() == BLOCK_END {
            in_block = false;
            continue;
        }
        if !in_block {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Clean up extra blank lines at end
    let trimmed = result.trim_end().to_string();
    Ok(if trimmed.is_empty() {
        String::new()
    } else {
        trimmed + "\n"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_block() {
        let content = "# My stuff\n*.log\n\n# ── conforme: generated tool configs ──\n# old stuff\n.cursor/\n# ── end conforme ──\n\n# Other\n*.tmp\n";
        let new_block = "# ── conforme: generated tool configs ──\n# new stuff\n.windsurf/\n# ── end conforme ──";
        let result = replace_block(content, new_block).unwrap();
        assert!(result.contains(".windsurf/"));
        assert!(!result.contains(".cursor/"));
        assert!(result.contains("*.log"));
        assert!(result.contains("*.tmp"));
    }

    #[test]
    fn test_remove_block() {
        let content = "# My stuff\n*.log\n\n# ── conforme: generated tool configs ──\n.cursor/\n.windsurf/\n# ── end conforme ──\n\n# Other\n*.tmp\n";
        let result = remove_block(content).unwrap();
        assert!(!result.contains("conforme"));
        assert!(!result.contains(".cursor/"));
        assert!(result.contains("*.log"));
        assert!(result.contains("*.tmp"));
    }

    #[test]
    fn test_gitignore_block_without_end_is_refused() {
        let content = "*.log\n# ── conforme: generated tool configs ──\n.cursor/\nmy-secret.env\n";
        assert!(replace_block(content, "x").is_err());
        assert!(remove_block(content).is_err());
    }

    #[test]
    fn test_remove_block_only_conforme() {
        let content = "# ── conforme: generated tool configs ──\n.cursor/\n# ── end conforme ──\n";
        let result = remove_block(content).unwrap();
        assert!(result.is_empty());
    }

    /// Whether a pattern of [`adapter_gitignore_patterns`] matches a path
    /// relative to the project root, with gitignore semantics for the three
    /// forms used there: `/file`, `dir/`, and `dir/*suffix`.
    fn pattern_matches(pattern: &str, rel: &str) -> bool {
        if let Some(file) = pattern.strip_prefix('/') {
            rel == file
        } else if pattern.ends_with('/') {
            rel.starts_with(pattern)
        } else if let Some((dir, suffix)) = pattern.split_once("/*") {
            rel.strip_prefix(dir)
                .and_then(|r| r.strip_prefix('/'))
                .is_some_and(|name| !name.contains('/') && name.ends_with(suffix))
        } else {
            rel == pattern
        }
    }

    fn full_config() -> crate::config::NormalizedConfig {
        use crate::config::*;
        NormalizedConfig {
            instructions: "Be helpful.".to_string(),
            rules: vec![NormalizedRule {
                name: "ts".to_string(),
                content: "Use TS.".to_string(),
                activation: ActivationMode::GlobMatch(vec!["**/*.ts".to_string()]),
            }],
            skills: vec![NormalizedSkill {
                name: "deploy".to_string(),
                description: "Deploy".to_string(),
                content: "Run.".to_string(),
                ..Default::default()
            }],
            agents: vec![NormalizedAgent {
                name: "reviewer".to_string(),
                description: "Review".to_string(),
                content: "Review.".to_string(),
                ..Default::default()
            }],
            mcp_servers: vec![NormalizedMcpServer {
                name: "fs".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec![],
                },
                env: Default::default(),
            }],
        }
    }

    /// Every file an adapter owns is ignored, no file it merges into (and
    /// that therefore holds user settings) is, and no pattern is stale.
    #[test]
    fn test_patterns_match_generated_files_exactly() {
        for adapter in adapters::all_adapters() {
            // A project without settings: Gemini's default GEMINI.md.
            let patterns = adapter_gitignore_patterns(Path::new("/nonexistent"), adapter.id());
            assert!(!patterns.is_empty(), "{} has no patterns", adapter.id());
            let mut all_rels = Vec::new();
            // The default layout, and one where Claude Code's instructions
            // live in `.claude/CLAUDE.md`.
            for nested_claude_md in [false, true] {
                let dir = tempfile::TempDir::new().unwrap();
                if nested_claude_md {
                    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
                    std::fs::write(dir.path().join(".claude/CLAUDE.md"), "x").unwrap();
                }
                let files = adapter.generate(dir.path(), &full_config()).unwrap();
                for (path, _) in &files {
                    let rel = path
                        .strip_prefix(dir.path())
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    let ignored = patterns.iter().any(|p| pattern_matches(p, &rel));
                    if adapter.is_shared_file(path) {
                        assert!(!ignored, "{}: shared {rel} is ignored", adapter.id());
                    } else {
                        assert!(ignored, "{}: {rel} is not ignored", adapter.id());
                    }
                    all_rels.push(rel);
                }
            }
            for pattern in &patterns {
                assert!(
                    all_rels.iter().any(|rel| pattern_matches(pattern, rel)),
                    "{}: pattern {pattern} matches nothing conforme writes",
                    adapter.id()
                );
            }
        }
    }

    #[test]
    fn test_block_keeps_the_source_locations_tracked() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join(".conformerc.toml"), "source = \"codex\"\n").unwrap();
        let block = build_gitignore_block(
            dir.path(),
            &project_config::ProjectConfig::load(dir.path()).unwrap(),
        );
        // Zed also writes `.agents/skills/`, which is Codex's source.
        assert!(!block.contains(".agents/skills/"), "{block}");
        // Codex reads AGENTS.md itself: it is the source, not an output.
        assert!(!block.contains("AGENTS.md\n"), "{block}");
        assert!(!block.contains(".github/prompts"), "{block}");
        assert!(!block.contains("settings.json"), "{block}");

        std::fs::write(dir.path().join(".conformerc.toml"), "source = \"claude\"\n").unwrap();
        let block = build_gitignore_block(
            dir.path(),
            &project_config::ProjectConfig::load(dir.path()).unwrap(),
        );
        assert_eq!(block.matches(".agents/skills/").count(), 1, "{block}");
        assert!(!block.contains("CLAUDE.md"), "{block}");
        assert!(block.contains("\n/AGENTS.md\n"), "{block}");
    }
}
