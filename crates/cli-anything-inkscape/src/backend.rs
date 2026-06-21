//! Inkscape rendering backend + safe SVG import.
//!
//! - SVG export is generated locally (no inkscape needed) from the model.
//! - PNG / PDF / EPS export shells out to the **real `inkscape`** (no fallback —
//!   the real software is the renderer, per the HARNESS rule). A missing/broken
//!   binary yields a typed error.
//! - SVG *import* routes untrusted input through
//!   [`cli_anything_core::security::xml::read_svg_safely`] — the security showcase.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use cli_anything_core::prelude::{read_svg_safely, require_binary, run};

use crate::domain::project::Project;
use crate::domain::svg::to_svg;

/// The real renderer binary.
pub const INKSCAPE_BIN: &str = "inkscape";
const EXPORT_TIMEOUT: Duration = Duration::from_secs(60);

fn guard_overwrite(out: &Path, overwrite: bool) -> Result<()> {
    if out.exists() && !overwrite {
        bail!("{} already exists (use --overwrite)", out.display());
    }
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// Export the model to SVG (generated locally; no inkscape). Returns byte size.
pub fn export_svg(project: &Project, out: &Path, overwrite: bool) -> Result<u64> {
    guard_overwrite(out, overwrite)?;
    let svg = to_svg(project)?;
    if !verify_svg(svg.as_bytes()) {
        bail!("generated SVG failed verification");
    }
    std::fs::write(out, &svg).with_context(|| format!("writing {}", out.display()))?;
    Ok(svg.len() as u64)
}

/// Raster/vector export via the real `inkscape`. `format` is `png`/`pdf`/`eps`.
pub fn export_via_inkscape(
    project: &Project,
    out: &Path,
    format: &str,
    dpi: u32,
    width: Option<u32>,
    height: Option<u32>,
    overwrite: bool,
) -> Result<u64> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    guard_overwrite(out, overwrite)?;
    require_binary(
        INKSCAPE_BIN,
        Some("install Inkscape from https://inkscape.org"),
    )?;

    let svg = to_svg(project)?;
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("inkscape-{}-{seq}.svg", std::process::id()));
    std::fs::write(&tmp, &svg)?;

    // Build the exact arg list (owned, then borrowed).
    let mut args: Vec<String> = vec![
        tmp.to_string_lossy().into_owned(),
        format!("--export-filename={}", out.display()),
    ];
    if format == "png" {
        args.push(format!("--export-dpi={dpi}"));
        if let Some(w) = width {
            args.push(format!("--export-width={w}"));
        }
        if let Some(h) = height {
            args.push(format!("--export-height={h}"));
        }
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let result = run(INKSCAPE_BIN, &arg_refs, EXPORT_TIMEOUT);
    let _ = std::fs::remove_file(&tmp);
    result?; // CoreError → anyhow (missing/failed inkscape surfaces here)

    let bytes = std::fs::read(out).with_context(|| format!("reading {}", out.display()))?;
    if !verify_format(&bytes, format) {
        bail!("exported {format} failed magic-byte verification");
    }
    Ok(bytes.len() as u64)
}

/// Safely read an untrusted SVG file (the security showcase). Returns the
/// validated text length. Rejects DOCTYPE/entities/SSRF/oversize via core.
pub fn import_svg_safely(path: &Path) -> Result<usize> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let safe = read_svg_safely(&bytes)?; // CoreError on a malicious doc → anyhow
    Ok(safe.len())
}

/// Probe the inkscape version (demonstrates the safe-subprocess path).
#[allow(dead_code)]
pub fn version() -> Result<String> {
    require_binary(
        INKSCAPE_BIN,
        Some("install Inkscape from https://inkscape.org"),
    )?;
    let out = run(INKSCAPE_BIN, &["--version"], EXPORT_TIMEOUT)?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Verify by format magic bytes.
fn verify_format(bytes: &[u8], format: &str) -> bool {
    match format {
        "png" => verify_png(bytes),
        "pdf" => verify_pdf(bytes),
        "eps" => bytes.starts_with(b"%!PS") || bytes.starts_with(&[0xC5, 0xD0, 0xD3, 0xC6]),
        _ => !bytes.is_empty(),
    }
}

/// PNG 8-byte signature.
pub fn verify_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
}

/// PDF `%PDF-` signature.
pub fn verify_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

/// SVG: non-empty and `<svg` appears within the first 256 bytes.
pub fn verify_svg(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let head = &bytes[..bytes.len().min(256)];
    String::from_utf8_lossy(head).contains("<svg")
}
