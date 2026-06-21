//! `preview live status --json` payload builder.
//!
//! This is the agent-cheap poll: it answers "is a live session active, what is
//! the current bundle, and how is it progressing?" by combining the mutable
//! [`SessionHead`] with a compact [`TrajectorySummary`] — without making the
//! agent read the whole trajectory file each loop.

use std::path::Path;

use serde::Serialize;

use crate::preview::session_head::SessionHead;
use crate::preview::trajectory::{summarize_trajectory, Trajectory, TrajectorySummary};

/// The `preview live status --json` payload.
#[derive(Debug, Clone, Serialize)]
pub struct LiveStatus {
    /// `active` / `inactive` / `none`.
    pub status: String,
    /// Whether a live session is currently active.
    pub active: bool,
    /// The session directory.
    pub session_dir: Option<String>,
    /// The `session.json` path.
    pub session_path: Option<String>,
    /// Current bundle id.
    pub current_bundle_id: Option<String>,
    /// Current bundle directory.
    pub current_bundle_dir: Option<String>,
    /// Current manifest path.
    pub current_manifest_path: Option<String>,
    /// Current summary path.
    pub current_summary_path: Option<String>,
    /// Trajectory file path.
    pub trajectory_path: Option<String>,
    /// Current step id.
    pub current_step_id: Option<String>,
    /// Most recent command.
    pub latest_command: Option<String>,
    /// Most recent publish reason.
    pub latest_publish_reason: Option<String>,
    /// Compact trajectory digest.
    pub trajectory_summary: TrajectorySummary,
}

/// Build a [`LiveStatus`] from the session head and trajectory.
///
/// `session` is `None` when no `session.json` exists yet (status `none`).
pub fn build_live_status(
    session_dir: &Path,
    session: Option<&SessionHead>,
    trajectory: &Trajectory,
    recent: usize,
) -> LiveStatus {
    let summary = summarize_trajectory(trajectory, recent);
    let session_path = session_dir.join("session.json");

    match session {
        Some(head) => LiveStatus {
            status: if head.active { "active" } else { "inactive" }.to_string(),
            active: head.active,
            session_dir: Some(session_dir.to_string_lossy().into_owned()),
            session_path: Some(session_path.to_string_lossy().into_owned()),
            current_bundle_id: head.current_bundle_id.clone(),
            current_bundle_dir: head.current_bundle_dir.clone(),
            current_manifest_path: head.current_manifest_path.clone(),
            current_summary_path: head.current_summary_path.clone(),
            trajectory_path: head.trajectory_path.clone(),
            current_step_id: head
                .current_step_id
                .clone()
                .or_else(|| summary.current_step_id.clone()),
            latest_command: summary.latest_command.clone(),
            latest_publish_reason: summary.latest_publish_reason.clone(),
            trajectory_summary: summary,
        },
        None => LiveStatus {
            status: "none".to_string(),
            active: false,
            session_dir: Some(session_dir.to_string_lossy().into_owned()),
            session_path: None,
            current_bundle_id: None,
            current_bundle_dir: None,
            current_manifest_path: None,
            current_summary_path: None,
            trajectory_path: None,
            current_step_id: None,
            latest_command: None,
            latest_publish_reason: None,
            trajectory_summary: summary,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::trajectory::TrajectoryStep;

    fn traj_with_one() -> Trajectory {
        let mut t = Trajectory::new("mermaid", "quick");
        t.append(TrajectoryStep {
            step_id: "s1".into(),
            step_index: 0,
            command: "preview capture".into(),
            command_started_at_ms: 1,
            command_finished_at_ms: 2,
            publish_reason: "render".into(),
            source_fingerprint: "sha256:x".into(),
            bundle_id: "b1".into(),
            bundle_dir: "/b/b1".into(),
            manifest_path: "/b/b1/manifest.json".into(),
            summary_path: "/b/b1/summary.json".into(),
            stage_label: None,
            note: None,
        });
        t
    }

    #[test]
    fn none_when_no_session() {
        let t = Trajectory::new("mermaid", "quick");
        let s = build_live_status(Path::new("/sess"), None, &t, 3);
        assert_eq!(s.status, "none");
        assert!(!s.active);
        assert!(s.session_path.is_none());
        assert_eq!(s.trajectory_summary.step_count, 0);
    }

    #[test]
    fn active_session_surfaces_latest_command() {
        let mut head = SessionHead::new("mermaid", "quick");
        head.active = true;
        head.current_bundle_id = Some("b1".into());
        let t = traj_with_one();
        let s = build_live_status(Path::new("/sess"), Some(&head), &t, 3);
        assert_eq!(s.status, "active");
        assert!(s.active);
        assert_eq!(s.current_bundle_id.as_deref(), Some("b1"));
        assert_eq!(s.latest_command.as_deref(), Some("preview capture"));
        assert_eq!(s.trajectory_summary.step_count, 1);
    }

    #[test]
    fn inactive_session_reports_inactive() {
        let head = SessionHead::new("mermaid", "quick"); // active = false
        let t = traj_with_one();
        let s = build_live_status(Path::new("/sess"), Some(&head), &t, 3);
        assert_eq!(s.status, "inactive");
        assert!(!s.active);
    }
}
