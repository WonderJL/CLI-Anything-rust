//! sha256 data/file fingerprints + the content-addressed cache key.
//!
//! Canonical form mirrors the Python `json.dumps(sort_keys=True,
//! separators=(",",":"))`: we serialize to a `serde_json::Value` (whose object
//! maps are `BTreeMap`, hence key-sorted) and `to_string` it (compact, no
//! spaces), then sha256. This guarantees *intra-Rust* fingerprint stability;
//! cross-language equality with the Python harness is explicitly not a goal.

use std::io::Read;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::Result;

/// Lowercase-hex sha256 of the canonical JSON form of `data`.
pub fn hash_data<T: Serialize>(data: &T) -> Result<String> {
    let value = serde_json::to_value(data)?;
    let canonical = serde_json::to_string(&value)?; // sorted keys (BTreeMap) + compact
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    Ok(to_hex(&hasher.finalize()))
}

/// `"sha256:<hex>"` fingerprint of `data`.
pub fn fingerprint_data<T: Serialize>(data: &T) -> Result<String> {
    Ok(format!("sha256:{}", hash_data(data)?))
}

/// Fingerprint a file by its resolved path, size, and a sha256 of its contents.
///
/// Content-addressed (not stat-based): two different byte streams never collide
/// to the same fingerprint, so a cached preview bundle is reused only when the
/// source bytes are genuinely unchanged — even if mtime is preserved or size is
/// identical.
pub fn fingerprint_file(path: &std::path::Path) -> Result<String> {
    let resolved = std::fs::canonicalize(path)?;
    let mut file = std::fs::File::open(&resolved)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let content_sha256 = to_hex(&hasher.finalize());
    let size = file.metadata()?.len();
    fingerprint_data(&serde_json::json!({
        "path": resolved.to_string_lossy(),
        "size": size,
        "content_sha256": content_sha256,
    }))
}

/// Inputs that define a bundle's cache identity.
#[derive(Debug, Clone)]
pub struct CacheKeyInputs<'a> {
    /// Protocol version (`preview-bundle/v1`).
    pub protocol_version: &'a str,
    /// Software name.
    pub software: &'a str,
    /// Recipe name.
    pub recipe: &'a str,
    /// Bundle kind (`static` / `diff` / `live-step`).
    pub bundle_kind: &'a str,
    /// Fingerprint of the source state.
    pub source_fingerprint: &'a str,
    /// Recipe options (any JSON).
    pub options: serde_json::Value,
    /// Harness version string.
    pub harness_version: &'a str,
}

/// Compute the content-addressed cache key for a bundle.
pub fn build_cache_key(inputs: &CacheKeyInputs) -> Result<String> {
    fingerprint_data(&serde_json::json!({
        "protocol_version": inputs.protocol_version,
        "software": inputs.software,
        "recipe": inputs.recipe,
        "bundle_kind": inputs.bundle_kind,
        "source_fingerprint": inputs.source_fingerprint,
        "options": inputs.options,
        "harness_version": inputs.harness_version,
    }))
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let a = serde_json::json!({"b": 1, "a": 2});
        let h1 = hash_data(&a).unwrap();
        let h2 = hash_data(&a).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // sha256 hex
    }

    #[test]
    fn canonical_form_is_key_order_independent() {
        // Same logical object, different source key order → same fingerprint.
        let a = serde_json::json!({"a": 1, "b": 2});
        let b = serde_json::json!({"b": 2, "a": 1});
        assert_eq!(hash_data(&a).unwrap(), hash_data(&b).unwrap());
    }

    #[test]
    fn fingerprint_data_is_prefixed() {
        let fp = fingerprint_data(&serde_json::json!({"x": 1})).unwrap();
        assert!(fp.starts_with("sha256:"));
    }

    #[test]
    fn different_data_differs() {
        let a = fingerprint_data(&serde_json::json!({"x": 1})).unwrap();
        let b = fingerprint_data(&serde_json::json!({"x": 2})).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn cache_key_changes_with_inputs() {
        let base = CacheKeyInputs {
            protocol_version: "preview-bundle/v1",
            software: "mermaid",
            recipe: "quick",
            bundle_kind: "static",
            source_fingerprint: "sha256:aaa",
            options: serde_json::json!({}),
            harness_version: "0.1.0",
        };
        let k1 = build_cache_key(&base).unwrap();
        let mut changed = base.clone();
        changed.source_fingerprint = "sha256:bbb";
        let k2 = build_cache_key(&changed).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn file_fingerprint_is_content_sensitive() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static C: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "cli-anything-fp-{}-{}",
            std::process::id(),
            C.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("src.txt");

        std::fs::write(&p, b"AAAA").unwrap();
        let f1 = fingerprint_file(&p).unwrap();
        // Same length, different bytes (mtime may even match on coarse FS) → must differ.
        std::fs::write(&p, b"BBBB").unwrap();
        let f2 = fingerprint_file(&p).unwrap();
        assert_ne!(
            f1, f2,
            "equal-length different content must change the fingerprint"
        );

        // Identical content → identical fingerprint.
        std::fs::write(&p, b"AAAA").unwrap();
        let f3 = fingerprint_file(&p).unwrap();
        assert_eq!(f1, f3);
    }
}
