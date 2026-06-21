//! Unit tests for Mermaid (synthetic; no external backend, no network).

use cli_anything_mermaid::backend::{serialize_state, share_url, verify_png, verify_svg};
use cli_anything_mermaid::cli::ShareMode;
use cli_anything_mermaid::domain::project::{Project, SAMPLES};

#[test]
fn pako_is_deterministic_and_prefixed() {
    let p = Project::with_sample("flowchart", "default");
    let a = serialize_state(&p).unwrap();
    let b = serialize_state(&p).unwrap();
    assert_eq!(a, b);
    assert!(a.starts_with("pako:"));
}

#[test]
fn pako_changes_with_code() {
    let p1 = Project::with_sample("flowchart", "default");
    let mut p2 = p1.clone();
    p2.code = "flowchart LR\n  X-->Y".to_string();
    assert_ne!(serialize_state(&p1).unwrap(), serialize_state(&p2).unwrap());
}

#[test]
fn share_url_is_well_formed() {
    let p = Project::new();
    let edit = share_url(&p, ShareMode::Edit).unwrap();
    assert!(edit.starts_with("https://mermaid.live/edit#pako:"));
    let view = share_url(&p, ShareMode::View).unwrap();
    assert!(view.contains("/view#pako:"));
}

#[test]
fn magic_byte_verification() {
    assert!(verify_png(&[0x89, 0x50, 0x4E, 0x47, 0, 0, 0, 0]));
    assert!(!verify_png(b"not a png"));
    assert!(verify_svg(
        br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#
    ));
    assert!(!verify_svg(b"nope, not svg content here"));
    assert!(!verify_svg(b""));
}

#[test]
fn samples_and_json_round_trip() {
    assert_eq!(SAMPLES, &["flowchart", "sequence", "er"]);
    let p = Project::with_sample("sequence", "dark");
    assert!(p.code.starts_with("sequenceDiagram"));
    assert_eq!(p.theme(), "dark");

    let json = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.code, p.code);
    assert_eq!(back.theme(), "dark");
}
