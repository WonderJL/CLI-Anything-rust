//! End-to-end export tests requiring the **real `inkscape`** binary.
//!
//! `#[ignore]`d by default (and the local inkscape may be broken). Run with
//! `cargo test -p cli-anything-inkscape -- --ignored`. They export via real
//! inkscape and VERIFY the output by magic bytes.

use cli_anything_inkscape::backend::{
    export_via_inkscape, export_via_rsvg, has_rsvg, verify_pdf, verify_png,
};
use cli_anything_inkscape::domain::project::{Project, Shape};

fn demo() -> Project {
    let mut p = Project::with_canvas(120.0, 90.0, "px", "#ffffff", "e2e");
    p.add_shape(Shape::Rect {
        x: 10.0,
        y: 10.0,
        width: 100.0,
        height: 70.0,
        rx: 0.0,
    });
    p
}

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "cli-anything-inkscape-e2e-{}-{name}",
        std::process::id()
    ))
}

#[test]
#[ignore = "requires a working real `inkscape` binary"]
fn exports_png_and_verifies() {
    let out = tmp("out.png");
    let size = export_via_inkscape(&demo(), &out, "png", 96, None, None, true).expect("png export");
    assert!(size > 0);
    assert!(verify_png(&std::fs::read(&out).unwrap()));
    let _ = std::fs::remove_file(&out);
}

#[test]
#[ignore = "requires a working real `inkscape` binary"]
fn exports_pdf_and_verifies() {
    let out = tmp("out.pdf");
    let size = export_via_inkscape(&demo(), &out, "pdf", 96, None, None, true).expect("pdf export");
    assert!(size > 0);
    assert!(verify_pdf(&std::fs::read(&out).unwrap()));
    let _ = std::fs::remove_file(&out);
}

#[test]
#[ignore = "requires a working real `rsvg-convert` (librsvg) binary"]
fn exports_png_via_rsvg_and_verifies() {
    if !has_rsvg() {
        eprintln!("skipping: rsvg-convert not installed");
        return;
    }
    let out = tmp("rsvg-out.png");
    let size =
        export_via_rsvg(&demo(), &out, "png", 96, None, None, true).expect("rsvg png export");
    assert!(size > 0);
    assert!(verify_png(&std::fs::read(&out).unwrap()));
    let _ = std::fs::remove_file(&out);
}

#[test]
#[ignore = "requires a working real `rsvg-convert` (librsvg) binary"]
fn exports_pdf_via_rsvg_and_verifies() {
    if !has_rsvg() {
        eprintln!("skipping: rsvg-convert not installed");
        return;
    }
    let out = tmp("rsvg-out.pdf");
    let size =
        export_via_rsvg(&demo(), &out, "pdf", 96, None, None, true).expect("rsvg pdf export");
    assert!(size > 0);
    assert!(verify_pdf(&std::fs::read(&out).unwrap()));
    let _ = std::fs::remove_file(&out);
}
