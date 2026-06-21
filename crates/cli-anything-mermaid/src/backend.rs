//! Mermaid rendering backend.
//!
//! Two render paths (selection logged in the result `method`):
//! - **local `mmdc`** (preferred when present) — writes the source to a temp
//!   `.mmd` and runs `mmdc` via the safe subprocess helper;
//! - **HTTP fallback** (`ureq`) — `pako`-encodes the state and GETs
//!   `mermaid.ink`.
//!
//! Output is always verified by magic bytes (PNG signature / `<svg`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use cli_anything_core::security::subprocess::{find_binary, run};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::Serialize;

use crate::cli::{Format, ShareMode};
use crate::domain::project::Project;

const RENDER_TIMEOUT: Duration = Duration::from_secs(120);

/// Outcome of a render, surfaced in the `--json` envelope.
#[derive(Debug, Clone, Serialize)]
pub struct RenderResult {
    /// The written output path.
    pub output: String,
    /// `svg` or `png`.
    pub format: String,
    /// `mmdc` or `http`.
    pub method: String,
    /// Output size in bytes.
    pub file_size: u64,
    /// The HTTP URL used (if the HTTP path rendered it).
    pub url: Option<String>,
}

/// `pako:`-encode the project state the way mermaid.ink / mermaid.live expect:
/// `pako:` + urlsafe-base64(no pad) of zlib-deflated `{code, mermaid: "<json>"}`.
pub fn serialize_state(project: &Project) -> Result<String> {
    let mermaid_str = serde_json::to_string(&project.mermaid)?;
    let state = serde_json::json!({ "code": project.code, "mermaid": mermaid_str });
    let json = serde_json::to_string(&state)?;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(json.as_bytes())?;
    let compressed = encoder.finish()?;
    Ok(format!("pako:{}", URL_SAFE_NO_PAD.encode(&compressed)))
}

/// Build a mermaid.live share URL (no network).
pub fn share_url(project: &Project, mode: ShareMode) -> Result<String> {
    let pako = serialize_state(project)?;
    let base =
        std::env::var("MERMAID_LIVE_URL").unwrap_or_else(|_| "https://mermaid.live".to_string());
    Ok(format!("{base}/{}#{pako}", mode.as_str()))
}

/// Render the diagram to `output`, trying `mmdc` then HTTP. Verifies the result.
pub fn render(
    project: &Project,
    output: &Path,
    format: Format,
    overwrite: bool,
) -> Result<RenderResult> {
    if output.exists() && !overwrite {
        bail!("{} already exists (use --overwrite)", output.display());
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let (method, url) = match render_mmdc(project, output, format) {
        Ok(()) => ("mmdc", None),
        Err(_) => {
            let url = render_http(project, output, format)?;
            ("http", Some(url))
        }
    };

    let bytes = std::fs::read(output)
        .with_context(|| format!("reading rendered output {}", output.display()))?;
    verify(&bytes, format)?;

    Ok(RenderResult {
        output: output.display().to_string(),
        format: format.as_str().to_string(),
        method: method.to_string(),
        file_size: bytes.len() as u64,
        url,
    })
}

fn render_mmdc(project: &Project, output: &Path, _format: Format) -> Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    if find_binary("mmdc").is_none() {
        bail!("mmdc not on PATH");
    }
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("mermaid-{}-{seq}.mmd", std::process::id()));
    std::fs::write(&tmp, &project.code)?;

    let in_path = tmp.to_string_lossy().into_owned();
    let out_path = output.to_string_lossy().into_owned();
    let result = run(
        "mmdc",
        &["-i", &in_path, "-o", &out_path, "-t", project.theme()],
        RENDER_TIMEOUT,
    );
    let _ = std::fs::remove_file(&tmp);
    result?; // CoreError → anyhow
    Ok(())
}

fn render_http(project: &Project, output: &Path, format: Format) -> Result<String> {
    let pako = serialize_state(project)?;
    let base =
        std::env::var("MERMAID_RENDERER_URL").unwrap_or_else(|_| "https://mermaid.ink".to_string());
    let url = match format {
        Format::Svg => format!("{base}/svg/{pako}"),
        Format::Png => format!("{base}/img/{pako}?type=png"),
    };

    let response = ureq::get(&url)
        .call()
        .with_context(|| format!("requesting {url}"))?;
    let status = response.status();
    if !status.is_success() {
        bail!("mermaid.ink returned HTTP {status}");
    }
    let bytes = response
        .into_body()
        .read_to_vec()
        .context("reading mermaid.ink response body")?;
    std::fs::write(output, &bytes).with_context(|| format!("writing {}", output.display()))?;
    Ok(url)
}

/// Verify rendered bytes match the requested format by magic bytes.
fn verify(bytes: &[u8], format: Format) -> Result<()> {
    let ok = match format {
        Format::Png => verify_png(bytes),
        Format::Svg => verify_svg(bytes),
    };
    if !ok {
        bail!("rendered output failed {} verification", format.as_str());
    }
    Ok(())
}

/// PNG: starts with the `\x89PNG` signature.
pub fn verify_png(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[..4] == [0x89, 0x50, 0x4E, 0x47]
}

/// SVG: non-empty and `<svg` appears within the first 200 bytes.
pub fn verify_svg(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let head = &bytes[..bytes.len().min(200)];
    String::from_utf8_lossy(head).contains("<svg")
}

/// Whether the local `mmdc` binary is available.
pub fn has_mmdc() -> bool {
    find_binary("mmdc").is_some()
}

/// The output path's bytes (helper for callers that need them).
#[allow(dead_code)]
pub fn read_output(path: &Path) -> Result<Vec<u8>> {
    Ok(std::fs::read(path)?)
}

/// Return a `PathBuf` for the default output of `format` (helper).
#[allow(dead_code)]
pub fn default_output(format: Format) -> PathBuf {
    PathBuf::from(format!("diagram.{}", format.as_str()))
}
