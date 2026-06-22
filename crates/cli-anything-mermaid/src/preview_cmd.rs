//! `preview` command logic: drive the core preview subsystem from the mermaid
//! render backend.
//!
//! Each `capture` renders the current diagram into an immutable
//! `preview-bundle/v1` directory (content-addressed, so identical source reuses
//! the cached bundle), records the artifact, advances the mutable `session.json`
//! live head, and appends a step to the append-only `trajectory.json`. `status`
//! is the cheap agent poll; `list` enumerates the recipe's bundles.

use std::path::Path;

use anyhow::{Context, Result};
use cli_anything_core::preview::{
    artifact_record, build_live_status, bundle_root, finalize_bundle, fingerprint_data, make_stamp,
    now_epoch_ms, prepare_bundle, CacheKeyInputs, FinalizeInputs, LiveStatus, Manifest,
    SessionHead, Trajectory, TrajectoryStep, PROTOCOL_VERSION,
};
use serde::Serialize;

use crate::backend;
use crate::cli::Format;
use crate::domain::project::Project;
use crate::{SOFTWARE, VERSION};

const BUNDLE_KIND: &str = "static";

/// The content fingerprint of a mermaid project's render-relevant state.
///
/// Only `code` + `mermaid` config affect the rendered artifact, so the cache key
/// depends on exactly those — editing live-viewer-only flags won't invalidate a
/// bundle.
pub fn source_fingerprint(project: &Project) -> Result<String> {
    Ok(fingerprint_data(&serde_json::json!({
        "code": project.code,
        "mermaid": project.mermaid,
    }))?)
}

/// Outcome of `preview capture`, surfaced in the `--json` envelope.
#[derive(Debug, Clone, Serialize)]
pub struct CaptureResult {
    /// Recipe this capture belongs to.
    pub recipe: String,
    /// Rendered format.
    pub format: String,
    /// Whether an identical cached bundle was reused (no re-render).
    pub cached: bool,
    /// The bundle id.
    pub bundle_id: String,
    /// The bundle directory.
    pub bundle_dir: String,
    /// The manifest path.
    pub manifest_path: String,
    /// The hero artifact's path relative to the bundle (if recorded).
    pub artifact: Option<String>,
    /// Render method when freshly rendered (`mmdc` / `http`); `None` on cache hit.
    pub render_method: Option<String>,
    /// The trajectory step id this capture appended.
    pub step_id: String,
    /// The live session head path.
    pub session_path: String,
    /// The trajectory path.
    pub trajectory_path: String,
}

/// Render the current diagram into a preview bundle and advance the live session.
pub fn capture(
    project: &Project,
    project_path: Option<&Path>,
    recipe: &str,
    format: Format,
    force: bool,
) -> Result<CaptureResult> {
    let root = bundle_root(SOFTWARE, recipe, project_path, None);
    std::fs::create_dir_all(&root)
        .with_context(|| format!("creating preview root {}", root.display()))?;

    let fingerprint = source_fingerprint(project)?;
    let inputs = CacheKeyInputs {
        protocol_version: PROTOCOL_VERSION,
        software: SOFTWARE,
        recipe,
        bundle_kind: BUNDLE_KIND,
        source_fingerprint: &fingerprint,
        options: serde_json::json!({ "format": format.as_str() }),
        harness_version: VERSION,
    };

    let stamp = make_stamp();
    // Bracket the render: read the start instant before prepare/render so the
    // recorded trajectory window actually spans the (possibly slow) backend call.
    let started = now_epoch_ms();
    let prep = prepare_bundle(&root, &inputs, &stamp, force)?;

    let mut render_method = None;
    let artifact_rel;
    if prep.cached {
        // Reuse the cached bundle's recorded hero artifact path, if any.
        artifact_rel = cached_hero(&prep.manifest_path);
    } else {
        let ext = format.as_str();
        let artifact_path = prep.artifacts_dir.join(format!("diagram.{ext}"));
        let render = backend::render(project, &artifact_path, format, true)
            .context("rendering preview artifact")?;
        render_method = Some(render.method.clone());
        let media_type = match format {
            Format::Svg => "image/svg+xml",
            Format::Png => "image/png",
        };
        let record = artifact_record(
            &prep.bundle_dir,
            &artifact_path,
            "hero",
            "hero",
            "image",
            "diagram",
            media_type,
        )?;
        artifact_rel = Some(record.path.clone());
        finalize_bundle(
            &prep,
            &inputs,
            FinalizeInputs {
                status: "ok".into(),
                generator: format!("{SOFTWARE} {VERSION}"),
                created_at_epoch_ms: now_epoch_ms(),
                artifacts: vec![record],
                warnings: vec![],
                labels: vec![recipe.to_string()],
                note: Some(format!("rendered via {}", render.method)),
                summary_headline: format!("{recipe} {ext} preview"),
                summary_metrics: serde_json::json!({
                    "bytes": render.file_size,
                    "method": render.method,
                }),
            },
        )?;
    }

    // Append a trajectory step, then advance the live head to point at it.
    let trajectory_path = root.join("trajectory.json");
    let session_path = root.join("session.json");
    let mut trajectory = Trajectory::load_or_new(&trajectory_path, SOFTWARE, recipe)?;
    let step_id = format!("{}-{}", stamp, trajectory.steps.len());
    trajectory.append(TrajectoryStep {
        step_id: step_id.clone(),
        step_index: 0, // reassigned by `append`
        command: format!("preview capture --format {}", format.as_str()),
        command_started_at_ms: started,
        command_finished_at_ms: now_epoch_ms(),
        publish_reason: if prep.cached { "cache-hit" } else { "render" }.to_string(),
        source_fingerprint: fingerprint.clone(),
        bundle_id: prep.bundle_id.clone(),
        bundle_dir: prep.bundle_dir.display().to_string(),
        manifest_path: prep.manifest_path.display().to_string(),
        summary_path: prep.summary_path.display().to_string(),
        stage_label: None,
        note: None,
    });
    trajectory.save(&trajectory_path)?;

    let mut head = SessionHead::load_or_new(&session_path, SOFTWARE, recipe)?;
    head.active = true;
    head.current_bundle_id = Some(prep.bundle_id.clone());
    head.current_bundle_dir = Some(prep.bundle_dir.display().to_string());
    head.current_manifest_path = Some(prep.manifest_path.display().to_string());
    head.current_summary_path = Some(prep.summary_path.display().to_string());
    head.current_step_id = Some(step_id.clone());
    head.trajectory_path = Some(trajectory_path.display().to_string());
    head.viewer_hint = Some("cli-anything-mermaid preview status --json".into());
    head.save(&session_path)?;

    Ok(CaptureResult {
        recipe: recipe.to_string(),
        format: format.as_str().to_string(),
        cached: prep.cached,
        bundle_id: prep.bundle_id,
        bundle_dir: prep.bundle_dir.display().to_string(),
        manifest_path: prep.manifest_path.display().to_string(),
        artifact: artifact_rel,
        render_method,
        step_id,
        session_path: session_path.display().to_string(),
        trajectory_path: trajectory_path.display().to_string(),
    })
}

/// The first recorded artifact path of a (cached) bundle's manifest, if readable.
fn cached_hero(manifest_path: &Path) -> Option<String> {
    let bytes = std::fs::read(manifest_path).ok()?;
    let m: Manifest = serde_json::from_slice(&bytes).ok()?;
    m.artifacts.into_iter().next().map(|a| a.path)
}

/// Build the live-status payload (the cheap agent poll).
pub fn status(project_path: Option<&Path>, recipe: &str, recent: usize) -> Result<LiveStatus> {
    let root = bundle_root(SOFTWARE, recipe, project_path, None);
    let session_path = root.join("session.json");
    let trajectory_path = root.join("trajectory.json");
    let head = if session_path.exists() {
        Some(SessionHead::load(&session_path)?)
    } else {
        None
    };
    let trajectory = Trajectory::load_or_new(&trajectory_path, SOFTWARE, recipe)?;
    Ok(build_live_status(&root, head.as_ref(), &trajectory, recent))
}

/// A listing of a recipe's preview bundles.
#[derive(Debug, Clone, Serialize)]
pub struct BundleListing {
    /// Recipe inspected.
    pub recipe: String,
    /// The recipe's bundle root.
    pub root: String,
    /// Number of bundles found.
    pub count: usize,
    /// Bundles, newest first.
    pub bundles: Vec<BundleSummary>,
}

/// One bundle in a [`BundleListing`].
#[derive(Debug, Clone, Serialize)]
pub struct BundleSummary {
    /// Bundle id.
    pub bundle_id: String,
    /// Bundle status.
    pub status: String,
    /// Number of recorded artifacts.
    pub artifact_count: usize,
    /// Creation time (epoch ms).
    pub created_at_epoch_ms: u64,
    /// Bundle directory.
    pub bundle_dir: String,
}

/// List the preview bundles for `recipe` (newest first).
pub fn list(project_path: Option<&Path>, recipe: &str) -> Result<BundleListing> {
    let root = bundle_root(SOFTWARE, recipe, project_path, None);
    let mut bundles = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let Ok(bytes) = std::fs::read(dir.join("manifest.json")) else {
                continue;
            };
            let Ok(m) = serde_json::from_slice::<Manifest>(&bytes) else {
                continue;
            };
            if m.protocol_version != PROTOCOL_VERSION {
                continue;
            }
            bundles.push(BundleSummary {
                bundle_id: m.bundle_id,
                status: m.status,
                artifact_count: m.artifacts.len(),
                created_at_epoch_ms: m.created_at_epoch_ms,
                bundle_dir: dir.display().to_string(),
            });
        }
    }
    // bundle_id is prefixed with a zero-padded epoch-ms stamp → lexical sort works.
    bundles.sort_by(|a, b| b.bundle_id.cmp(&a.bundle_id));
    Ok(BundleListing {
        recipe: recipe.to_string(),
        root: root.display().to_string(),
        count: bundles.len(),
        bundles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A unique temp *project path* (its parent anchors the preview root). Not
    /// created on disk — preview ops derive `<parent>/.cli-anything/previews/...`.
    fn temp_project_path() -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "cli-anything-mermaid-prev-{}-{}",
                std::process::id(),
                id
            ))
            .join("diagram.mermaid.json")
    }

    #[test]
    fn fingerprint_is_deterministic_and_content_sensitive() {
        let a = Project::with_sample("flowchart", "default");
        let b = Project::with_sample("flowchart", "default");
        let c = Project::with_sample("sequence", "default");
        assert_eq!(
            source_fingerprint(&a).unwrap(),
            source_fingerprint(&b).unwrap()
        );
        assert_ne!(
            source_fingerprint(&a).unwrap(),
            source_fingerprint(&c).unwrap()
        );
        assert!(source_fingerprint(&a).unwrap().starts_with("sha256:"));
    }

    #[test]
    fn status_is_none_when_no_session_exists() {
        let p = temp_project_path();
        let s = status(Some(&p), "default", 5).unwrap();
        assert_eq!(s.status, "none");
        assert!(!s.active);
        assert_eq!(s.trajectory_summary.step_count, 0);
    }

    #[test]
    fn list_is_empty_when_no_bundles_exist() {
        let p = temp_project_path();
        let listing = list(Some(&p), "default").unwrap();
        assert_eq!(listing.count, 0);
        assert!(listing.bundles.is_empty());
    }
}
