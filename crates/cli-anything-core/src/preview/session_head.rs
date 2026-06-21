//! Mutable `session.json` — the "current live head" for a project + recipe.
//!
//! Unlike immutable bundles, this file is rewritten as the live view advances.
//! It is the stable entry point for "what is current right now" and points at
//! the current bundle plus the append-only trajectory.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::preview::SESSION_PROTOCOL_VERSION;

/// The current live head for a software+recipe preview session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHead {
    /// Protocol version (`preview-session/v1`).
    pub protocol_version: String,
    /// Software name.
    pub software: String,
    /// Recipe name.
    pub recipe: String,
    /// Whether the live session is active.
    pub active: bool,
    /// Current bundle id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_bundle_id: Option<String>,
    /// Current bundle directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_bundle_dir: Option<String>,
    /// Current manifest path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_manifest_path: Option<String>,
    /// Current summary path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_summary_path: Option<String>,
    /// Current trajectory step id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_step_id: Option<String>,
    /// Path to the trajectory file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trajectory_path: Option<String>,
    /// Hint for how to inspect the bundle (consumer command).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer_hint: Option<String>,
}

impl SessionHead {
    /// A fresh, inactive session head.
    pub fn new(software: &str, recipe: &str) -> Self {
        Self {
            protocol_version: SESSION_PROTOCOL_VERSION.to_string(),
            software: software.to_string(),
            recipe: recipe.to_string(),
            active: false,
            current_bundle_id: None,
            current_bundle_dir: None,
            current_manifest_path: None,
            current_summary_path: None,
            current_step_id: None,
            trajectory_path: None,
            viewer_hint: None,
        }
    }

    /// Load an existing `session.json`.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Load the head at `path`, or create a fresh one if it doesn't exist.
    pub fn load_or_new(path: &Path, software: &str, recipe: &str) -> Result<Self> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::new(software, recipe))
        }
    }

    /// Persist the head to `path` (pretty JSON).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_defaults_to_new_when_missing() {
        let dir = std::env::temp_dir().join(format!("cli-anything-head-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session-test.json");
        let _ = std::fs::remove_file(&path);

        let mut head = SessionHead::load_or_new(&path, "mermaid", "quick").unwrap();
        assert!(!head.active);
        head.active = true;
        head.current_bundle_id = Some("b1".into());
        head.save(&path).unwrap();

        let back = SessionHead::load(&path).unwrap();
        assert!(back.active);
        assert_eq!(back.current_bundle_id.as_deref(), Some("b1"));
        assert_eq!(back.protocol_version, SESSION_PROTOCOL_VERSION);
    }
}
