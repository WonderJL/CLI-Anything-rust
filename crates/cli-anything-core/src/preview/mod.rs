//! Full preview subsystem (port of `preview_bundle.py` + `preview-methodology.md`).
//!
//! Three-layer model:
//! - [`bundle`]       — immutable `preview-bundle/v1` snapshot (manifest + summary + artifacts)
//! - [`session_head`] — mutable `session.json` "current live head"
//! - [`trajectory`]   — append-only `trajectory.json` permanent history
//! - [`fingerprint`]  — sha256 data/file fingerprints + content-addressed cache key
//! - [`live_status`]  — cheap `preview live status --json` payload builder
//!
//! Timestamps are epoch milliseconds (sortable, dependency-free) rather than the
//! Python original's calendar strings — intra-Rust ordering/uniqueness is what
//! matters; cross-language fingerprint equality is explicitly NOT a goal.

pub mod bundle;
pub mod fingerprint;
pub mod live_status;
pub mod session_head;
pub mod trajectory;

use std::path::PathBuf;

/// Bundle manifest/summary protocol version.
pub const PROTOCOL_VERSION: &str = "preview-bundle/v1";
/// Trajectory protocol version.
pub const TRAJECTORY_PROTOCOL_VERSION: &str = "preview-trajectory/v1";
/// Session-head protocol version.
pub const SESSION_PROTOCOL_VERSION: &str = "preview-session/v1";

pub use bundle::{
    artifact_record, bundle_root, finalize_bundle, find_cached_manifest, find_latest_manifest,
    prepare_bundle, ArtifactRecord, FinalizeInputs, Manifest, PrepareResult, Summary,
};
pub use fingerprint::{
    build_cache_key, fingerprint_data, fingerprint_file, hash_data, CacheKeyInputs,
};
pub use live_status::{build_live_status, LiveStatus};
pub use session_head::SessionHead;
pub use trajectory::{summarize_trajectory, Trajectory, TrajectoryStep, TrajectorySummary};

/// Slugify a value to `[a-z0-9-]` (lowercase, runs collapsed), defaulting to
/// `"preview"` — mirrors the Python `_slug`.
pub(crate) fn slug(value: &str) -> String {
    let lower = value.trim().to_ascii_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut prev_dash = false;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "preview".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Milliseconds since the Unix epoch (0 if the clock is before the epoch).
pub fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A zero-padded, lexically sortable timestamp for bundle ids.
pub fn make_stamp() -> String {
    format!("{:013}", now_epoch_ms())
}

/// Best-effort home directory from the environment.
pub(crate) fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_normalizes() {
        assert_eq!(slug("Quick Render!"), "quick-render");
        assert_eq!(slug("  a__b  "), "a-b");
        assert_eq!(slug("***"), "preview");
        assert_eq!(slug("Mermaid"), "mermaid");
    }

    #[test]
    fn stamp_is_sortable_and_padded() {
        let s = make_stamp();
        assert_eq!(s.len(), 13);
        assert!(s.chars().all(|c| c.is_ascii_digit()));
    }
}
