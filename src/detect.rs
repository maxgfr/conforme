use std::path::Path;

use crate::adapters;

/// Information about a detected tool.
pub struct DetectedTool {
    pub name: String,
    pub detected: bool,
}

/// Scan the project root and return detection status for all adapters.
pub fn detect_tools(project_root: &Path) -> Vec<DetectedTool> {
    let adapters = adapters::all_adapters();
    adapters
        .iter()
        .map(|a| DetectedTool {
            name: a.name().to_string(),
            detected: a.detect(project_root),
        })
        .collect()
}

/// Check if AGENTS.md exists in the project root.
pub fn has_agents_md(project_root: &Path) -> bool {
    project_root.join("AGENTS.md").exists()
}
