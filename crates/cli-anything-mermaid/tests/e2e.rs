//! End-to-end render tests.
//!
//! These need a working renderer — either local `mmdc` with a browser, or
//! network access to mermaid.ink (the HTTP fallback) — so they are `#[ignore]`d
//! by default. Run with `cargo test -p cli-anything-mermaid -- --ignored`.
//! They render a real diagram and VERIFY the output by magic bytes.

use cli_anything_mermaid::backend::{render, verify_png, verify_svg};
use cli_anything_mermaid::cli::Format;
use cli_anything_mermaid::domain::project::Project;
use cli_anything_mermaid::preview_cmd;

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "cli-anything-mermaid-e2e-{}-{name}",
        std::process::id()
    ))
}

#[test]
#[ignore = "needs mmdc+browser or network (mermaid.ink)"]
fn renders_svg_and_verifies() {
    let project = Project::with_sample("flowchart", "default");
    let out = tmp("out.svg");
    let result = render(&project, &out, Format::Svg, true).expect("svg render");
    assert_eq!(result.format, "svg");
    assert!(result.file_size > 0);
    assert!(matches!(result.method.as_str(), "mmdc" | "http"));
    assert!(verify_svg(&std::fs::read(&out).unwrap()));
    let _ = std::fs::remove_file(&out);
}

#[test]
#[ignore = "needs mmdc+browser or network (mermaid.ink)"]
fn renders_png_and_verifies() {
    let project = Project::with_sample("sequence", "default");
    let out = tmp("out.png");
    let result = render(&project, &out, Format::Png, true).expect("png render");
    assert_eq!(result.format, "png");
    assert!(verify_png(&std::fs::read(&out).unwrap()));
    let _ = std::fs::remove_file(&out);
}

/// Full preview round-trip against the real render backend: capture writes a
/// verified artifact into an immutable bundle, a re-capture hits the cache, and
/// `status`/`list` reflect the live session.
#[test]
#[ignore = "needs mmdc+browser or network (mermaid.ink)"]
fn preview_capture_caches_and_status_reflects_it() {
    // Anchor the preview root inside a unique temp project dir.
    let dir = std::env::temp_dir().join(format!(
        "cli-anything-mermaid-preve2e-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let project_path = dir.join("diagram.mermaid.json");
    let project = Project::with_sample("flowchart", "default");

    // First capture → fresh bundle with a verified SVG artifact.
    let first =
        preview_cmd::capture(&project, Some(&project_path), "default", Format::Svg, false).unwrap();
    assert!(!first.cached, "first capture should render, not hit cache");
    let artifact_rel = first.artifact.expect("hero artifact recorded");
    let artifact_abs = std::path::Path::new(&first.bundle_dir).join(&artifact_rel);
    assert!(verify_svg(&std::fs::read(&artifact_abs).unwrap()));
    assert!(matches!(
        first.render_method.as_deref(),
        Some("http") | Some("mmdc")
    ));

    // Second capture, identical source → cache hit, same bundle id.
    let second =
        preview_cmd::capture(&project, Some(&project_path), "default", Format::Svg, false).unwrap();
    assert!(second.cached, "identical source should hit the cache");
    assert_eq!(second.bundle_id, first.bundle_id);

    // Status reflects the active live session and ≥2 trajectory steps.
    let st = preview_cmd::status(Some(&project_path), "default", 5).unwrap();
    assert_eq!(st.status, "active");
    assert!(st.active);
    assert_eq!(
        st.current_bundle_id.as_deref(),
        Some(first.bundle_id.as_str())
    );
    assert!(st.trajectory_summary.step_count >= 2);

    // List finds the bundle.
    let listing = preview_cmd::list(Some(&project_path), "default").unwrap();
    assert!(listing.count >= 1);
    assert!(listing
        .bundles
        .iter()
        .any(|b| b.bundle_id == first.bundle_id));

    let _ = std::fs::remove_dir_all(&dir);
}
