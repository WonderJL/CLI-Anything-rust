//! End-to-end render tests.
//!
//! These need a working renderer — either local `mmdc` with a browser, or
//! network access to mermaid.ink (the HTTP fallback) — so they are `#[ignore]`d
//! by default. Run with `cargo test -p cli-anything-mermaid -- --ignored`.
//! They render a real diagram and VERIFY the output by magic bytes.

use cli_anything_mermaid::backend::{render, verify_png, verify_svg};
use cli_anything_mermaid::cli::Format;
use cli_anything_mermaid::domain::project::Project;

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
