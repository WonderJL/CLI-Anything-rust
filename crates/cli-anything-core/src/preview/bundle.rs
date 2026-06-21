//! Immutable `preview-bundle/v1` snapshot: manifest, summary, artifacts.
//!
//! A bundle directory is written once (summary then manifest, the latter being
//! the commit marker) and treated as immutable thereafter. Caching reuses an
//! existing bundle whose `cache_key` matches the current source state.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::preview::fingerprint::{build_cache_key, CacheKeyInputs};
use crate::preview::{slug, PROTOCOL_VERSION};

/// One artifact recorded in a bundle (path is relative to the bundle dir).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    /// Stable id within the bundle.
    pub artifact_id: String,
    /// Role: `hero` / `gallery` / `clip` / `diff` / …
    pub role: String,
    /// Kind: `image` / `video` / `inspection` / …
    pub kind: String,
    /// Human label.
    pub label: String,
    /// MIME media type.
    pub media_type: String,
    /// Path relative to the bundle directory.
    pub path: String,
    /// Size in bytes.
    pub bytes: u64,
}

/// The immutable bundle manifest (`manifest.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Protocol version (`preview-bundle/v1`).
    pub protocol_version: String,
    /// Unique, sortable bundle id.
    pub bundle_id: String,
    /// Bundle kind (`static` / `diff` / `live-step`).
    pub bundle_kind: String,
    /// Software name.
    pub software: String,
    /// Recipe name.
    pub recipe: String,
    /// Fingerprint of the source state.
    pub source_fingerprint: String,
    /// Content-addressed cache key.
    pub cache_key: String,
    /// `ok` / `partial` / `error`.
    pub status: String,
    /// Creation time, epoch milliseconds.
    pub created_at_epoch_ms: u64,
    /// Generator identity (`<software> <version>`).
    pub generator: String,
    /// Recorded artifacts.
    pub artifacts: Vec<ArtifactRecord>,
    /// Non-fatal warnings (e.g. injected preview helpers).
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Free-form labels.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Optional truthfulness note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,

    // Resolved paths — attached on read, never serialized into the canonical file.
    /// Absolute bundle directory (filled on read; not serialized).
    #[serde(skip)]
    pub bundle_dir: Option<PathBuf>,
    /// Absolute manifest path (filled on read; not serialized).
    #[serde(skip)]
    pub manifest_path: Option<PathBuf>,
    /// Absolute summary path (filled on read; not serialized).
    #[serde(skip)]
    pub summary_path: Option<PathBuf>,
}

/// The bundle summary (`summary.json`) — a compact, human-facing digest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// Bundle id this summary describes.
    pub bundle_id: String,
    /// Software name.
    pub software: String,
    /// Recipe name.
    pub recipe: String,
    /// Bundle status.
    pub status: String,
    /// Number of artifacts.
    pub artifact_count: usize,
    /// One-line headline.
    pub headline: String,
    /// Arbitrary metrics blob.
    #[serde(default)]
    pub metrics: serde_json::Value,
}

/// Result of [`prepare_bundle`]: where to write (or the cached bundle's paths).
#[derive(Debug, Clone)]
pub struct PrepareResult {
    /// Whether an existing cached bundle was reused.
    pub cached: bool,
    /// The computed cache key.
    pub cache_key: String,
    /// The bundle id (existing if cached, freshly minted otherwise).
    pub bundle_id: String,
    /// The bundle directory.
    pub bundle_dir: PathBuf,
    /// The `artifacts/` directory inside the bundle.
    pub artifacts_dir: PathBuf,
    /// The manifest path.
    pub manifest_path: PathBuf,
    /// The summary path.
    pub summary_path: PathBuf,
}

/// Everything needed to finalize a freshly-rendered bundle.
#[derive(Debug, Clone)]
pub struct FinalizeInputs {
    /// `ok` / `partial` / `error`.
    pub status: String,
    /// Generator identity.
    pub generator: String,
    /// Creation time (epoch ms).
    pub created_at_epoch_ms: u64,
    /// Recorded artifacts.
    pub artifacts: Vec<ArtifactRecord>,
    /// Warnings.
    pub warnings: Vec<String>,
    /// Labels.
    pub labels: Vec<String>,
    /// Optional truthfulness note.
    pub note: Option<String>,
    /// Summary headline.
    pub summary_headline: String,
    /// Summary metrics.
    pub summary_metrics: serde_json::Value,
}

/// Compute the root directory that holds bundles for a software+recipe.
///
/// Priority: explicit `root_dir`; else `<project_dir>/.cli-anything/previews`;
/// else `~/.cli-anything/previews`.
pub fn bundle_root(
    software: &str,
    recipe: &str,
    project_path: Option<&Path>,
    root_dir: Option<&Path>,
) -> PathBuf {
    let base = if let Some(r) = root_dir {
        r.to_path_buf()
    } else if let Some(p) = project_path {
        p.parent()
            .unwrap_or(Path::new("."))
            .join(".cli-anything")
            .join("previews")
    } else {
        super::home_dir().join(".cli-anything").join("previews")
    };
    base.join(slug(software)).join(slug(recipe))
}

/// Prepare a bundle: reuse a cached one if the cache key matches, else mint a
/// fresh bundle directory (with `artifacts/`).
pub fn prepare_bundle(
    root: &Path,
    inputs: &CacheKeyInputs,
    stamp: &str,
    force: bool,
) -> Result<PrepareResult> {
    let cache_key = build_cache_key(inputs)?;

    if !force {
        if let Some(m) = find_cached_manifest(
            root,
            inputs.software,
            inputs.recipe,
            inputs.bundle_kind,
            &cache_key,
        )? {
            let dir = m
                .bundle_dir
                .clone()
                .unwrap_or_else(|| root.join(&m.bundle_id));
            return Ok(PrepareResult {
                cached: true,
                cache_key,
                bundle_id: m.bundle_id.clone(),
                artifacts_dir: dir.join("artifacts"),
                manifest_path: dir.join("manifest.json"),
                summary_path: dir.join("summary.json"),
                bundle_dir: dir,
            });
        }
    }

    let short = cache_key.strip_prefix("sha256:").unwrap_or(&cache_key);
    let short = &short[..short.len().min(8)];
    let bundle_id = format!("{stamp}_{short}_{}", slug(inputs.recipe));
    let bundle_dir = root.join(&bundle_id);
    let artifacts_dir = bundle_dir.join("artifacts");
    std::fs::create_dir_all(&artifacts_dir)?;

    Ok(PrepareResult {
        cached: false,
        cache_key,
        bundle_id,
        manifest_path: bundle_dir.join("manifest.json"),
        summary_path: bundle_dir.join("summary.json"),
        artifacts_dir,
        bundle_dir,
    })
}

/// Build an [`ArtifactRecord`] for a file already written inside `bundle_dir`.
pub fn artifact_record(
    bundle_dir: &Path,
    path: &Path,
    artifact_id: &str,
    role: &str,
    kind: &str,
    label: &str,
    media_type: &str,
) -> Result<ArtifactRecord> {
    let bytes = std::fs::metadata(path)?.len();
    let rel = path
        .strip_prefix(bundle_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned();
    Ok(ArtifactRecord {
        artifact_id: artifact_id.to_string(),
        role: role.to_string(),
        kind: kind.to_string(),
        label: label.to_string(),
        media_type: media_type.to_string(),
        path: rel,
        bytes,
    })
}

/// Write `summary.json` then `manifest.json`, returning the manifest with paths
/// attached. The manifest is written last so its presence marks completion.
pub fn finalize_bundle(
    prep: &PrepareResult,
    inputs: &CacheKeyInputs,
    fin: FinalizeInputs,
) -> Result<Manifest> {
    let summary = Summary {
        bundle_id: prep.bundle_id.clone(),
        software: inputs.software.to_string(),
        recipe: inputs.recipe.to_string(),
        status: fin.status.clone(),
        artifact_count: fin.artifacts.len(),
        headline: fin.summary_headline,
        metrics: fin.summary_metrics,
    };
    std::fs::write(&prep.summary_path, serde_json::to_string_pretty(&summary)?)?;

    let mut manifest = Manifest {
        protocol_version: PROTOCOL_VERSION.to_string(),
        bundle_id: prep.bundle_id.clone(),
        bundle_kind: inputs.bundle_kind.to_string(),
        software: inputs.software.to_string(),
        recipe: inputs.recipe.to_string(),
        source_fingerprint: inputs.source_fingerprint.to_string(),
        cache_key: prep.cache_key.clone(),
        status: fin.status,
        created_at_epoch_ms: fin.created_at_epoch_ms,
        generator: fin.generator,
        artifacts: fin.artifacts,
        warnings: fin.warnings,
        labels: fin.labels,
        note: fin.note,
        bundle_dir: None,
        manifest_path: None,
        summary_path: None,
    };
    std::fs::write(
        &prep.manifest_path,
        serde_json::to_string_pretty(&manifest)?,
    )?;
    attach_paths(&mut manifest, &prep.bundle_dir);
    Ok(manifest)
}

/// Find a cached bundle whose key matches (status `ok`/`partial`), newest first.
pub fn find_cached_manifest(
    root: &Path,
    software: &str,
    recipe: &str,
    bundle_kind: &str,
    cache_key: &str,
) -> Result<Option<Manifest>> {
    Ok(scan_manifests(root, |m| {
        m.software == software
            && m.recipe == recipe
            && m.bundle_kind == bundle_kind
            && m.cache_key == cache_key
            && matches!(m.status.as_str(), "ok" | "partial")
    }))
}

/// Find the newest bundle for a software+recipe+kind (status `ok`/`partial`).
pub fn find_latest_manifest(
    root: &Path,
    software: &str,
    recipe: &str,
    bundle_kind: &str,
) -> Result<Option<Manifest>> {
    Ok(scan_manifests(root, |m| {
        m.software == software
            && m.recipe == recipe
            && m.bundle_kind == bundle_kind
            && matches!(m.status.as_str(), "ok" | "partial")
    }))
}

fn scan_manifests(root: &Path, pred: impl Fn(&Manifest) -> bool) -> Option<Manifest> {
    let entries = std::fs::read_dir(root).ok()?;
    let mut best: Option<Manifest> = None;
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Ok(bytes) = std::fs::read(dir.join("manifest.json")) else {
            continue;
        };
        let Ok(mut m) = serde_json::from_slice::<Manifest>(&bytes) else {
            continue;
        };
        if m.protocol_version != PROTOCOL_VERSION || !pred(&m) {
            continue;
        }
        attach_paths(&mut m, &dir);
        if best.as_ref().is_none_or(|b| m.bundle_id > b.bundle_id) {
            best = Some(m);
        }
    }
    best
}

fn attach_paths(m: &mut Manifest, dir: &Path) {
    m.bundle_dir = Some(dir.to_path_buf());
    m.manifest_path = Some(dir.join("manifest.json"));
    m.summary_path = Some(dir.join("summary.json"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_root() -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("cli-anything-bundle-{}-{}", std::process::id(), id));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn inputs<'a>() -> CacheKeyInputs<'a> {
        CacheKeyInputs {
            protocol_version: PROTOCOL_VERSION,
            software: "mermaid",
            recipe: "quick",
            bundle_kind: "static",
            source_fingerprint: "sha256:deadbeef",
            options: serde_json::json!({}),
            harness_version: "0.1.0",
        }
    }

    fn finalize(prep: &PrepareResult) -> Manifest {
        finalize_bundle(
            prep,
            &inputs(),
            FinalizeInputs {
                status: "ok".into(),
                generator: "mermaid 0.1.0".into(),
                created_at_epoch_ms: 1_700_000_000_000,
                artifacts: vec![],
                warnings: vec![],
                labels: vec![],
                note: None,
                summary_headline: "rendered".into(),
                summary_metrics: serde_json::json!({}),
            },
        )
        .unwrap()
    }

    #[test]
    fn bundle_root_layout() {
        let r = bundle_root("Mermaid", "Quick Render", None, Some(Path::new("/tmp/x")));
        assert_eq!(r, PathBuf::from("/tmp/x/mermaid/quick-render"));
    }

    #[test]
    fn prepare_then_finalize_then_cache_hit() {
        let root = temp_root();
        // First prepare → miss, fresh bundle.
        let p1 = prepare_bundle(&root, &inputs(), "0000000000001", false).unwrap();
        assert!(!p1.cached);
        assert!(p1.artifacts_dir.is_dir());
        let m = finalize(&p1);
        assert_eq!(m.status, "ok");
        assert!(m.manifest_path.is_some());

        // Second prepare with same inputs → cache hit, same bundle id.
        let p2 = prepare_bundle(&root, &inputs(), "0000000000002", false).unwrap();
        assert!(p2.cached);
        assert_eq!(p2.bundle_id, p1.bundle_id);
    }

    #[test]
    fn force_bypasses_cache() {
        let root = temp_root();
        let p1 = prepare_bundle(&root, &inputs(), "0000000000001", false).unwrap();
        finalize(&p1);
        let p2 = prepare_bundle(&root, &inputs(), "0000000000002", true).unwrap();
        assert!(!p2.cached);
        assert_ne!(p2.bundle_id, p1.bundle_id);
    }

    #[test]
    fn different_source_misses_cache() {
        let root = temp_root();
        let p1 = prepare_bundle(&root, &inputs(), "0000000000001", false).unwrap();
        finalize(&p1);
        let mut changed = inputs();
        changed.source_fingerprint = "sha256:feedface";
        let p2 = prepare_bundle(&root, &changed, "0000000000002", false).unwrap();
        assert!(!p2.cached);
    }

    #[test]
    fn find_latest_returns_newest() {
        let root = temp_root();
        let a = prepare_bundle(&root, &inputs(), "0000000000001", true).unwrap();
        finalize(&a);
        let b = prepare_bundle(&root, &inputs(), "0000000000009", true).unwrap();
        finalize(&b);
        let latest = find_latest_manifest(&root, "mermaid", "quick", "static")
            .unwrap()
            .unwrap();
        assert_eq!(latest.bundle_id, b.bundle_id);
    }
}
