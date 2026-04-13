use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::Path;

use crate::adapters;
use crate::project_config;

const BLOCK_START: &str = "# ── conforme: generated tool configs ──";
const BLOCK_END: &str = "# ── end conforme ──";

/// Patterns that each adapter's generated output produces.
/// Returns (adapter_id, vec of gitignore patterns).
fn adapter_gitignore_patterns(id: &str) -> Vec<&'static str> {
    match id {
        "claude" => vec!["CLAUDE.md", ".claude/rules/", ".claude/skills/", ".claude/agents/"],
        "cursor" => vec![".cursor/rules/", ".cursor/skills/", ".cursor/agents/", ".cursor/mcp.json"],
        "windsurf" => vec![
            ".windsurf/rules/",
            ".windsurf/skills/",
            ".windsurf/mcp.json",
        ],
        "copilot" => vec![
            ".github/copilot-instructions.md",
            ".github/instructions/",
            ".github/prompts/",
            ".github/agents/",
            ".vscode/mcp.json",
        ],
        "codex" => vec![".agents/skills/"],
        "opencode" => vec![
            ".opencode/skills/",
            ".opencode/mcp.json",
            ".opencode/agents.json",
        ],
        "roocode" => vec![".roo/rules/", ".roo/skills/", ".roo/mcp.json"],
        "gemini" => vec![
            "GEMINI.md",
            ".gemini/skills/",
            ".gemini/agents/",
            ".gemini/settings.json",
        ],
        "continue" => vec![".continue/rules/", ".continue/mcpServers/"],
        "zed" => vec![".rules", ".zed/settings.json"],
        "amazonq" => vec![
            ".amazonq/rules/",
            ".amazonq/cli-agents/",
            ".amazonq/mcp.json",
        ],
        "kiro" => vec![
            ".kiro/steering/",
            ".kiro/skills/",
            ".kiro/agents/",
            ".kiro/settings/",
        ],
        "amp" => vec![".agents/skills/", ".amp/settings.json"],
        _ => vec![],
    }
}

/// Build the gitignore block content based on project config.
fn build_gitignore_block(project_root: &Path) -> String {
    let config = project_config::ProjectConfig::load(project_root);
    let source_id = config.source.as_deref().unwrap_or("agents.md");

    let all = adapters::all_adapters();
    let mut lines = vec![
        BLOCK_START.to_string(),
        format!("# Source: {source_id} — only generated outputs are ignored."),
        "# Managed by `conforme gitignore install`. Do not edit this block.".to_string(),
    ];

    // Collect patterns for non-source adapters
    for adapter in &all {
        if adapter.id() == source_id {
            continue;
        }

        let patterns = adapter_gitignore_patterns(adapter.id());
        if patterns.is_empty() {
            continue;
        }

        lines.push(format!("# {}", adapter.name()));
        for pat in patterns {
            lines.push(pat.to_string());
        }
    }

    // AGENTS.md is generated when using a tool source (not agents.md)
    if source_id != "agents.md" && config.generate_agents_md {
        lines.push("# Generated AGENTS.md".to_string());
        lines.push("AGENTS.md".to_string());
    }

    lines.push(BLOCK_END.to_string());

    lines.join("\n")
}

/// Install conforme-managed entries into .gitignore.
pub fn install(project_root: &Path, verbose: bool) -> Result<()> {
    let gitignore_path = project_root.join(".gitignore");
    let block = build_gitignore_block(project_root);

    if gitignore_path.exists() {
        let existing = std::fs::read_to_string(&gitignore_path)
            .with_context(|| format!("failed to read {}", gitignore_path.display()))?;

        if existing.contains(BLOCK_START) {
            // Replace existing block
            let updated = replace_block(&existing, &block);
            std::fs::write(&gitignore_path, updated)?;
            println!("{} .gitignore updated.", "+".green());
            if verbose {
                println!("  Replaced existing conforme block");
            }
        } else {
            // Append block
            let mut content = existing.trim_end().to_string();
            content.push_str("\n\n");
            content.push_str(&block);
            content.push('\n');
            std::fs::write(&gitignore_path, content)?;
            println!("{} .gitignore updated.", "+".green());
            if verbose {
                println!("  Appended conforme block");
            }
        }
    } else {
        // Create new .gitignore
        std::fs::write(&gitignore_path, format!("{block}\n"))?;
        println!("{} .gitignore created.", "+".green());
    }

    // Print summary
    let config = project_config::ProjectConfig::load(project_root);
    let source_id = config.source.as_deref().unwrap_or("agents.md");
    let all = adapters::all_adapters();
    let ignored_count = all.iter().filter(|a| a.id() != source_id).count();
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
        println!(
            "{} .gitignore has no conforme-managed block.",
            "=".dimmed()
        );
        return Ok(());
    }

    let updated = remove_block(&content);
    std::fs::write(&gitignore_path, updated)?;

    println!("{} Removed conforme block from .gitignore.", "+".green());
    if verbose {
        println!("  .gitignore cleaned up");
    }

    Ok(())
}

/// Replace the conforme block in a gitignore string.
fn replace_block(content: &str, new_block: &str) -> String {
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

    result
}

/// Remove the conforme block from a gitignore string.
fn remove_block(content: &str) -> String {
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
    if trimmed.is_empty() {
        String::new()
    } else {
        trimmed + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_block() {
        let content = "# My stuff\n*.log\n\n# ── conforme: generated tool configs ──\n# old stuff\n.cursor/\n# ── end conforme ──\n\n# Other\n*.tmp\n";
        let new_block = "# ── conforme: generated tool configs ──\n# new stuff\n.windsurf/\n# ── end conforme ──";
        let result = replace_block(content, new_block);
        assert!(result.contains(".windsurf/"));
        assert!(!result.contains(".cursor/"));
        assert!(result.contains("*.log"));
        assert!(result.contains("*.tmp"));
    }

    #[test]
    fn test_remove_block() {
        let content = "# My stuff\n*.log\n\n# ── conforme: generated tool configs ──\n.cursor/\n.windsurf/\n# ── end conforme ──\n\n# Other\n*.tmp\n";
        let result = remove_block(content);
        assert!(!result.contains("conforme"));
        assert!(!result.contains(".cursor/"));
        assert!(result.contains("*.log"));
        assert!(result.contains("*.tmp"));
    }

    #[test]
    fn test_remove_block_only_conforme() {
        let content = "# ── conforme: generated tool configs ──\n.cursor/\n# ── end conforme ──\n";
        let result = remove_block(content);
        assert!(result.is_empty());
    }

    #[test]
    fn test_adapter_patterns_coverage() {
        let all = adapters::all_adapters();
        for adapter in &all {
            let patterns = adapter_gitignore_patterns(adapter.id());
            assert!(
                !patterns.is_empty(),
                "adapter {} has no gitignore patterns",
                adapter.id()
            );
        }
    }
}
