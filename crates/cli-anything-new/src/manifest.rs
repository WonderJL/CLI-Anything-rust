//! Idempotent registration of a new crate in the workspace `Cargo.toml`.

use std::fs;
use std::path::Path;

use anyhow::{bail, Result};

/// Add `member` (e.g. `crates/cli-anything-mermaid`) to the workspace
/// `[workspace].members` array if not already present.
///
/// Returns `true` if it added the entry, `false` if it was already present or
/// the target has no workspace members array (a standalone crate).
pub fn register_member(workspace_root: &Path, member: &str) -> Result<bool> {
    let manifest_path = workspace_root.join("Cargo.toml");
    let text = match fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(_) => return Ok(false), // no workspace manifest → standalone crate
    };

    let needle = format!("\"{member}\"");
    if text.contains(&needle) {
        return Ok(false);
    }

    let Some(members_at) = text.find("members") else {
        return Ok(false);
    };
    let Some(open) = text[members_at..].find('[').map(|i| members_at + i) else {
        return Ok(false);
    };
    let Some(close_rel) = text[open..].find(']') else {
        bail!(
            "malformed [workspace].members array in {}",
            manifest_path.display()
        );
    };
    let close = open + close_rel;

    let inner = text[open + 1..close].trim();
    let insertion = if inner.is_empty() {
        needle
    } else {
        format!(", {needle}")
    };
    let new_text = format!("{}{}{}", &text[..close], insertion, &text[close..]);
    fs::write(&manifest_path, new_text)?;
    Ok(true)
}
