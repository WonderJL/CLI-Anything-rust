//! Append-only `trajectory.json` — permanent, replayable history.
//!
//! Each step binds an agent action (the command) to the preview bundle it
//! produced. [`summarize_trajectory`] returns a compact digest so an agent can
//! poll `preview live status --json` cheaply without reading every step.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::preview::TRAJECTORY_PROTOCOL_VERSION;

/// One recorded step: a command and the bundle it published.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// Stable id for this step.
    pub step_id: String,
    /// Zero-based position in the trajectory (assigned by [`Trajectory::append`]).
    pub step_index: usize,
    /// The command that produced this step.
    pub command: String,
    /// Command start time (epoch ms).
    pub command_started_at_ms: u64,
    /// Command finish time (epoch ms).
    pub command_finished_at_ms: u64,
    /// Why this bundle was published.
    pub publish_reason: String,
    /// Source fingerprint at publish time.
    pub source_fingerprint: String,
    /// The bundle id this step produced.
    pub bundle_id: String,
    /// The bundle directory.
    pub bundle_dir: String,
    /// The manifest path.
    pub manifest_path: String,
    /// The summary path.
    pub summary_path: String,
    /// Optional stage label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_label: Option<String>,
    /// Optional note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The append-only trajectory document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    /// Protocol version (`preview-trajectory/v1`).
    pub protocol_version: String,
    /// Software name.
    pub software: String,
    /// Recipe name.
    pub recipe: String,
    /// Ordered steps.
    pub steps: Vec<TrajectoryStep>,
}

impl Trajectory {
    /// A fresh, empty trajectory.
    pub fn new(software: &str, recipe: &str) -> Self {
        Self {
            protocol_version: TRAJECTORY_PROTOCOL_VERSION.to_string(),
            software: software.to_string(),
            recipe: recipe.to_string(),
            steps: Vec::new(),
        }
    }

    /// Load an existing trajectory file.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Load the trajectory at `path`, or create a fresh one if it doesn't exist.
    pub fn load_or_new(path: &Path, software: &str, recipe: &str) -> Result<Self> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::new(software, recipe))
        }
    }

    /// Persist the trajectory to `path` (pretty JSON).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Append a step, assigning its `step_index`. Returns the assigned index.
    pub fn append(&mut self, mut step: TrajectoryStep) -> usize {
        let index = self.steps.len();
        step.step_index = index;
        self.steps.push(step);
        index
    }

    /// The id of the most recent step, if any.
    pub fn current_step_id(&self) -> Option<&str> {
        self.steps.last().map(|s| s.step_id.as_str())
    }
}

/// A compact digest of a trajectory for cheap agent polling.
#[derive(Debug, Clone, Serialize)]
pub struct TrajectorySummary {
    /// Total number of steps.
    pub step_count: usize,
    /// Most recent step id.
    pub current_step_id: Option<String>,
    /// Most recent command.
    pub latest_command: Option<String>,
    /// Most recent publish reason.
    pub latest_publish_reason: Option<String>,
    /// Most recent bundle id.
    pub latest_bundle_id: Option<String>,
    /// The last `recent` steps (most recent last).
    pub recent_steps: Vec<TrajectoryStep>,
}

/// Summarize a trajectory, including its last `recent` steps.
pub fn summarize_trajectory(trajectory: &Trajectory, recent: usize) -> TrajectorySummary {
    let last = trajectory.steps.last();
    let start = trajectory.steps.len().saturating_sub(recent);
    TrajectorySummary {
        step_count: trajectory.steps.len(),
        current_step_id: last.map(|s| s.step_id.clone()),
        latest_command: last.map(|s| s.command.clone()),
        latest_publish_reason: last.map(|s| s.publish_reason.clone()),
        latest_bundle_id: last.map(|s| s.bundle_id.clone()),
        recent_steps: trajectory.steps[start..].to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str, cmd: &str, bundle: &str) -> TrajectoryStep {
        TrajectoryStep {
            step_id: id.into(),
            step_index: 0,
            command: cmd.into(),
            command_started_at_ms: 1,
            command_finished_at_ms: 2,
            publish_reason: "render".into(),
            source_fingerprint: "sha256:x".into(),
            bundle_id: bundle.into(),
            bundle_dir: format!("/b/{bundle}"),
            manifest_path: format!("/b/{bundle}/manifest.json"),
            summary_path: format!("/b/{bundle}/summary.json"),
            stage_label: None,
            note: None,
        }
    }

    #[test]
    fn append_assigns_increasing_indices() {
        let mut t = Trajectory::new("mermaid", "quick");
        assert_eq!(t.append(step("s1", "capture", "b1")), 0);
        assert_eq!(t.append(step("s2", "capture", "b2")), 1);
        assert_eq!(t.steps[0].step_index, 0);
        assert_eq!(t.steps[1].step_index, 1);
        assert_eq!(t.current_step_id(), Some("s2"));
    }

    #[test]
    fn summary_reports_recent_tail() {
        let mut t = Trajectory::new("mermaid", "quick");
        for i in 1..=5 {
            t.append(step(&format!("s{i}"), "capture", &format!("b{i}")));
        }
        let s = summarize_trajectory(&t, 3);
        assert_eq!(s.step_count, 5);
        assert_eq!(s.current_step_id.as_deref(), Some("s5"));
        assert_eq!(s.latest_bundle_id.as_deref(), Some("b5"));
        assert_eq!(s.recent_steps.len(), 3);
        assert_eq!(s.recent_steps[0].step_id, "s3");
        assert_eq!(s.recent_steps[2].step_id, "s5");
    }

    #[test]
    fn empty_summary_is_safe() {
        let t = Trajectory::new("mermaid", "quick");
        let s = summarize_trajectory(&t, 3);
        assert_eq!(s.step_count, 0);
        assert!(s.current_step_id.is_none());
        assert!(s.recent_steps.is_empty());
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("cli-anything-traj-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("trajectory.json");
        let mut t = Trajectory::new("mermaid", "quick");
        t.append(step("s1", "capture", "b1"));
        t.save(&path).unwrap();
        let back = Trajectory::load(&path).unwrap();
        assert_eq!(back.steps.len(), 1);
        assert_eq!(back.protocol_version, TRAJECTORY_PROTOCOL_VERSION);
    }
}
