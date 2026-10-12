use anyhow::{bail, Context, Result};
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

const BLOCK_START: &str = "# >>> conforme >>>";
const BLOCK_END: &str = "# <<< conforme <<<";
/// The first line of the block earlier versions appended at the end of the
/// hook, where an `exit` or `exec` before it skipped the check.
const LEGACY_MARKER: &str = "# conforme pre-commit hook";

/// The git repository `project_root` belongs to: its hooks directory (as git
/// resolves it: a worktree's common directory, `core.hooksPath`) and the
/// project's path relative to the top level of the working tree.
struct Repo {
    hooks_dir: PathBuf,
    subdir: Option<String>,
}

fn git_output(project_root: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim_end_matches(['\n', '\r']);
    (!text.is_empty()).then(|| text.to_string())
}

fn find_repo(project_root: &Path) -> Result<Repo> {
    if let Some(hooks) = git_output(project_root, &["rev-parse", "--git-path", "hooks"]) {
        let hooks_dir = project_root.join(hooks);
        let subdir = git_output(project_root, &["rev-parse", "--show-prefix"])
            .map(|prefix| prefix.trim_end_matches('/').to_string())
            .filter(|prefix| !prefix.is_empty());
        return Ok(Repo { hooks_dir, subdir });
    }
    // Without git (or in a repository it cannot read), the usual layout.
    if project_root.join(".git").is_dir() {
        return Ok(Repo {
            hooks_dir: project_root.join(".git").join("hooks"),
            subdir: None,
        });
    }
    bail!(
        "No .git directory found in {}. Run this in a git repository.",
        project_root.display()
    );
}

fn block(subdir: Option<&str>) -> String {
    let command = match subdir {
        Some(dir) => format!("conforme check -C '{}'", dir.replace('\'', r"'\''")),
        None => "conforme check".to_string(),
    };
    format!(
        "{BLOCK_START}\n\
         # Installed by conforme (https://github.com/maxgfr/conforme): checks that the\n\
         # AI tool configs are in sync. `conforme hook uninstall` removes this block.\n\
         {command} || exit 1\n\
         {BLOCK_END}\n"
    )
}

/// Whether a hook's first line runs it with a POSIX shell (or none, which git
/// runs with `sh`), where the conforme block can go.
fn is_shell_hook(first_line: &str) -> bool {
    let Some(interpreter) = first_line.strip_prefix("#!") else {
        return true;
    };
    let mut words = interpreter.split_whitespace();
    let mut program = words.next().unwrap_or("");
    if program.rsplit('/').next() == Some("env") {
        program = words.find(|w| !w.starts_with('-')).unwrap_or("");
    }
    matches!(
        program.rsplit('/').next().unwrap_or(""),
        "sh" | "bash" | "dash" | "zsh" | "ksh" | "ash"
    )
}

/// Install a git pre-commit hook that runs `conforme check`.
pub fn install(project_root: &Path, verbose: bool) -> Result<()> {
    let repo = find_repo(project_root)?;
    std::fs::create_dir_all(&repo.hooks_dir)
        .with_context(|| format!("failed to create {}", repo.hooks_dir.display()))?;
    let hook_path = repo.hooks_dir.join("pre-commit");
    let block = block(repo.subdir.as_deref());

    let content = if hook_path.exists() {
        let existing = std::fs::read_to_string(&hook_path)
            .with_context(|| format!("failed to read {}", hook_path.display()))?;
        if existing.contains(&block) {
            println!("{} Pre-commit hook already installed.", "=".green());
            return Ok(());
        }
        let existing = remove_blocks(&existing);
        let first_line = existing.lines().next().unwrap_or("");
        if !is_shell_hook(first_line) {
            bail!(
                "{} runs with `{}`, not a shell: conforme cannot add its check to it. \
                 Call `conforme check` from it yourself; nothing was changed.",
                hook_path.display(),
                first_line.trim_start_matches("#!").trim()
            );
        }
        // Right after the shebang: an `exit` or `exec` further down would
        // otherwise end the hook before the check.
        let updated = match existing.split_once('\n') {
            Some((shebang, rest)) if shebang.starts_with("#!") => {
                format!("{shebang}\n{block}{rest}")
            }
            None if existing.starts_with("#!") => format!("{existing}\n{block}"),
            _ => format!("{block}{existing}"),
        };
        if verbose {
            println!("  Added conforme check to the existing pre-commit hook");
        }
        updated
    } else {
        format!("#!/bin/sh\n{block}")
    };
    std::fs::write(&hook_path, content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&hook_path)?.permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        std::fs::set_permissions(&hook_path, permissions)?;
    }

    println!(
        "{} Pre-commit hook installed at {}",
        "+".green(),
        hook_path
            .strip_prefix(project_root)
            .unwrap_or(&hook_path)
            .display()
    );
    println!("  Configs will be checked automatically before each commit.");

    Ok(())
}

/// The hook without conforme's marked block, nor the block earlier versions
/// appended (with the `#!/bin/sh` they put before it).
fn remove_blocks(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut kept: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim() == BLOCK_START {
            match lines[i..].iter().position(|l| l.trim() == BLOCK_END) {
                Some(end) => {
                    i += end + 1;
                    continue;
                }
                None => {
                    kept.push(line);
                    i += 1;
                    continue;
                }
            }
        }
        if line.trim() == LEGACY_MARKER {
            // The `#!/bin/sh` and blank line the legacy block brought along.
            while kept.len() > 1 && kept.last().is_some_and(|l| l.trim().is_empty()) {
                kept.pop();
            }
            if kept.len() > 1 && kept.last().is_some_and(|l| l.trim() == "#!/bin/sh") {
                kept.pop();
            }
            i += 1;
            while i < lines.len() {
                let trimmed = lines[i].trim();
                let ours = trimmed.is_empty()
                    || trimmed == "conforme check"
                    || (trimmed.starts_with('#')
                        && (trimmed.contains("conforme")
                            || trimmed.contains("do not edit this block")));
                if !ours {
                    break;
                }
                i += 1;
            }
            continue;
        }
        kept.push(line);
        i += 1;
    }
    while kept.len() > 1 && kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let mut result = kept.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

/// Uninstall the conforme pre-commit hook.
pub fn uninstall(project_root: &Path, verbose: bool) -> Result<()> {
    let hook_path = find_repo(project_root)?.hooks_dir.join("pre-commit");

    if !hook_path.exists() {
        println!("{} No pre-commit hook found.", "=".dimmed());
        return Ok(());
    }

    let content = std::fs::read_to_string(&hook_path)?;
    if !content.contains(BLOCK_START) && !content.contains(LEGACY_MARKER) {
        println!(
            "{} Pre-commit hook exists but was not installed by conforme.",
            "!".yellow()
        );
        return Ok(());
    }

    let rest = remove_blocks(&content);
    let only_shebang = rest
        .lines()
        .all(|line| line.trim().is_empty() || line.starts_with("#!"));
    if only_shebang {
        std::fs::remove_file(&hook_path)?;
        if verbose {
            println!("  Removed pre-commit hook file");
        }
    } else {
        std::fs::write(&hook_path, rest)?;
        if verbose {
            println!("  Removed conforme block from pre-commit hook");
        }
    }

    println!("{} Pre-commit hook uninstalled.", "+".green());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_shell_detection() {
        assert!(is_shell_hook("#!/bin/sh"));
        assert!(is_shell_hook("#!/usr/bin/env bash"));
        assert!(is_shell_hook("npm test"));
        assert!(!is_shell_hook("#!/usr/bin/env node"));
        assert!(!is_shell_hook("#!/usr/bin/python3"));
    }

    #[test]
    fn test_hook_remove_blocks_keeps_the_rest() {
        let hook = format!("#!/bin/sh\n{}npm test\n", block(None));
        assert_eq!(remove_blocks(&hook), "#!/bin/sh\nnpm test\n");
    }
}
