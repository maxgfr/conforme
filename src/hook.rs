use anyhow::{bail, Context, Result};
use owo_colors::OwoColorize;
use std::path::Path;

const HOOK_MARKER: &str = "# conforme pre-commit hook";

fn hook_script() -> String {
    format!(
        r#"#!/bin/sh
{HOOK_MARKER}
# Automatically installed by conforme — do not edit this block.
# See: https://github.com/maxgfr/conforme

conforme check
"#
    )
}

/// Install a git pre-commit hook that runs `conforme check`.
pub fn install(project_root: &Path, verbose: bool) -> Result<()> {
    let git_dir = project_root.join(".git");
    if !git_dir.is_dir() {
        bail!(
            "No .git directory found in {}. Run this in a git repository.",
            project_root.display()
        );
    }

    let hooks_dir = git_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    let hook_path = hooks_dir.join("pre-commit");

    if hook_path.exists() {
        let existing = std::fs::read_to_string(&hook_path)
            .with_context(|| format!("failed to read {}", hook_path.display()))?;

        if existing.contains(HOOK_MARKER) {
            println!("{} Pre-commit hook already installed.", "=".green());
            return Ok(());
        }

        // Append to existing hook
        let updated = format!("{}\n\n{}", existing.trim_end(), hook_script());
        std::fs::write(&hook_path, updated)?;

        if verbose {
            println!("  Appended conforme check to existing pre-commit hook");
        }
    } else {
        std::fs::write(&hook_path, hook_script())?;
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))?;
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

/// Uninstall the conforme pre-commit hook.
pub fn uninstall(project_root: &Path, verbose: bool) -> Result<()> {
    let hook_path = project_root.join(".git").join("hooks").join("pre-commit");

    if !hook_path.exists() {
        println!("{} No pre-commit hook found.", "=".dimmed());
        return Ok(());
    }

    let content = std::fs::read_to_string(&hook_path)?;

    if !content.contains(HOOK_MARKER) {
        println!(
            "{} Pre-commit hook exists but was not installed by conforme.",
            "!".yellow()
        );
        return Ok(());
    }

    // Check if the hook ONLY contains our script
    let lines: Vec<&str> = content.lines().collect();
    let non_conforme_lines: Vec<&&str> = lines
        .iter()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with('#')
                && !trimmed.starts_with("conforme check")
                && trimmed != "#!/bin/sh"
        })
        .collect();

    if non_conforme_lines.is_empty() {
        // Hook is only conforme — remove the file
        std::fs::remove_file(&hook_path)?;
        if verbose {
            println!("  Removed pre-commit hook file");
        }
    } else {
        // Other hooks exist — remove only our block
        let mut result = String::new();
        let mut in_conforme_block = false;

        for line in content.lines() {
            if line.contains(HOOK_MARKER) {
                in_conforme_block = true;
                continue;
            }
            if in_conforme_block {
                // Skip lines until we hit a non-conforme line
                let trimmed = line.trim();
                if trimmed.starts_with('#')
                    || trimmed.starts_with("conforme ")
                    || trimmed.is_empty()
                {
                    // Check if this comment is part of our block
                    if trimmed.contains("conforme") || trimmed.contains("do not edit this block") {
                        continue;
                    }
                    if trimmed.is_empty() && in_conforme_block {
                        continue;
                    }
                }
                in_conforme_block = false;
            }
            if !in_conforme_block {
                result.push_str(line);
                result.push('\n');
            }
        }

        let result = result.trim_end().to_string() + "\n";
        std::fs::write(&hook_path, result)?;

        if verbose {
            println!("  Removed conforme block from pre-commit hook");
        }
    }

    println!("{} Pre-commit hook uninstalled.", "+".green());
    Ok(())
}
