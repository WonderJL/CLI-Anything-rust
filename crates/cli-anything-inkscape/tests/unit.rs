//! Unit tests for Inkscape (offline — no real inkscape, no network).

use std::sync::atomic::{AtomicUsize, Ordering};

use cli_anything_inkscape::backend::{
    export_svg, import_svg_safely, verify_pdf, verify_png, verify_svg,
};
use cli_anything_inkscape::domain::project::{Project, Shape};
use cli_anything_inkscape::domain::svg::to_svg;

static C: AtomicUsize = AtomicUsize::new(0);

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cli-anything-inkscape-test-{}-{}",
        std::process::id(),
        C.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn doc_with_shapes() -> Project {
    let mut p = Project::with_canvas(200.0, 150.0, "px", "#ffffff", "demo");
    p.add_shape(Shape::Rect {
        x: 10.0,
        y: 10.0,
        width: 80.0,
        height: 60.0,
        rx: 4.0,
    });
    p.add_shape(Shape::Circle {
        cx: 150.0,
        cy: 75.0,
        r: 30.0,
    });
    p
}

#[test]
fn model_json_round_trips() {
    let p = doc_with_shapes();
    let json = serde_json::to_string_pretty(&p).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.objects.len(), 2);
    assert_eq!(back.canvas.width, 200.0);
    assert_eq!(back.objects[0].shape.kind(), "rect");
}

#[test]
fn generated_svg_is_well_formed_and_safe() {
    let svg = to_svg(&doc_with_shapes()).unwrap();
    // Our own SVG must pass the same safety reader untrusted input goes through.
    assert!(cli_anything_core::security::xml::read_svg_safely(svg.as_bytes()).is_ok());
    assert!(svg.contains("<rect"));
    assert!(svg.contains("<circle"));
    assert!(svg.contains("inkscape:groupmode=\"layer\""));
}

#[test]
fn untrusted_text_is_escaped_in_svg() {
    let mut p = Project::with_canvas(100.0, 100.0, "px", "none", "x");
    p.add_shape(Shape::Text {
        x: 5.0,
        y: 20.0,
        content: "<script>alert(1)</script>".to_string(),
        font_size: 16.0,
    });
    let svg = to_svg(&p).unwrap();
    assert!(!svg.contains("<script>"), "raw script tag must not appear");
    assert!(svg.contains("&lt;script&gt;"));
    assert!(cli_anything_core::security::xml::read_svg_safely(svg.as_bytes()).is_ok());
}

#[test]
fn export_svg_writes_verifiable_file() {
    let out = tmp("out.svg");
    let size = export_svg(&doc_with_shapes(), &out, true).unwrap();
    assert!(size > 0);
    let bytes = std::fs::read(&out).unwrap();
    assert!(verify_svg(&bytes));
}

#[test]
fn security_import_rejects_malicious_and_accepts_safe() {
    // billion-laughs / DOCTYPE → rejected.
    let evil = tmp("evil.svg");
    std::fs::write(
        &evil,
        b"<?xml version=\"1.0\"?>\n<!DOCTYPE lolz [<!ENTITY lol \"lol\">]>\n<svg>&lol;</svg>",
    )
    .unwrap();
    assert!(import_svg_safely(&evil).is_err());

    // external href (SSRF) → rejected.
    let ssrf = tmp("ssrf.svg");
    std::fs::write(
        &ssrf,
        br#"<svg xmlns="http://www.w3.org/2000/svg"><image href="http://evil/x.png"/></svg>"#,
    )
    .unwrap();
    assert!(import_svg_safely(&ssrf).is_err());

    // well-formed, safe SVG → accepted.
    let good = tmp("good.svg");
    std::fs::write(
        &good,
        br#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1"/></svg>"#,
    )
    .unwrap();
    assert!(import_svg_safely(&good).is_ok());
}

#[test]
fn magic_byte_verification() {
    assert!(verify_png(&[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A
    ]));
    assert!(!verify_png(b"not png"));
    assert!(verify_pdf(b"%PDF-1.7\n..."));
    assert!(!verify_pdf(b"%NOPE"));
    assert!(verify_svg(br#"<svg xmlns="..."></svg>"#));
    assert!(!verify_svg(b""));
}
