//! Shared error types for the core crate.
//!
//! `CoreError` is a `thiserror` enum; binaries map it into the `--json`
//! [`crate::json_envelope::ErrInfo`] (kind / message / hint) so agents always
//! receive structured, machine-readable failures (per HARNESS "fail loudly and
//! clearly").

use std::path::PathBuf;
use thiserror::Error;

/// Convenience result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, CoreError>;

/// All fallible operations in `cli-anything-core` surface this error.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Filesystem / IO failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A project path escaped its allowed base directory.
    #[error("path '{path}' escapes the allowed directory")]
    PathTraversal {
        /// The offending (pre-canonicalization) path.
        path: PathBuf,
    },

    /// Untrusted XML/SVG exceeded the byte safety limit.
    #[error("xml input is {actual} bytes, over the {limit}-byte safety limit")]
    XmlTooLarge {
        /// Configured maximum.
        limit: usize,
        /// Observed size.
        actual: usize,
    },

    /// Untrusted XML/SVG contained a forbidden construct (DOCTYPE/DTD/entity).
    #[error("xml contains a forbidden construct: {what}")]
    XmlForbiddenEntity {
        /// Human description of the rejected construct.
        what: String,
    },

    /// Untrusted XML/SVG nested deeper than the safety cap (billion-laughs guard).
    #[error("xml nesting exceeds the maximum depth of {max}")]
    XmlTooDeep {
        /// Configured maximum depth.
        max: usize,
    },

    /// Untrusted XML/SVG contained more elements than the safety cap.
    #[error("xml element count exceeds the maximum of {max}")]
    XmlTooManyElements {
        /// Configured maximum element count.
        max: usize,
    },

    /// Untrusted XML/SVG was not well-formed.
    #[error("malformed xml: {0}")]
    XmlMalformed(String),

    /// A required external program was not found on `PATH`.
    #[error("required program '{program}' was not found on PATH")]
    SubprocessNotFound {
        /// The program name we looked for.
        program: String,
        /// Optional install instructions, surfaced as the envelope `hint`.
        install_hint: Option<String>,
    },

    /// An external program exited with a non-zero status.
    #[error("'{program}' exited with code {code}")]
    SubprocessFailed {
        /// The program name.
        program: String,
        /// Exit code (or -1 if terminated by signal).
        code: i32,
        /// Tail of captured stderr.
        stderr: String,
    },

    /// An external program exceeded its timeout.
    #[error("'{program}' timed out after {seconds}s")]
    SubprocessTimeout {
        /// The program name.
        program: String,
        /// Timeout in seconds.
        seconds: u64,
    },

    /// An external program produced more output than the safety cap allows.
    #[error("'{program}' produced more than the {limit}-byte output limit")]
    SubprocessOutputTooLarge {
        /// The program name.
        program: String,
        /// Configured byte limit.
        limit: usize,
    },

    /// Failed to acquire a file lock for an atomic save.
    #[error("failed to lock '{path}'")]
    Lock {
        /// The file we tried to lock.
        path: PathBuf,
        /// The underlying IO error.
        #[source]
        source: std::io::Error,
    },

    /// No cached preview bundle matched the requested cache key.
    #[error("no cached preview bundle for cache key {cache_key}")]
    BundleCacheMiss {
        /// The cache key that missed.
        cache_key: String,
    },
}

impl CoreError {
    /// A stable, snake_case machine identifier for this error (the envelope `kind`).
    pub fn kind(&self) -> &'static str {
        match self {
            CoreError::Io(_) => "io",
            CoreError::Json(_) => "json",
            CoreError::PathTraversal { .. } => "path_traversal",
            CoreError::XmlTooLarge { .. } => "xml_too_large",
            CoreError::XmlForbiddenEntity { .. } => "xml_forbidden_entity",
            CoreError::XmlTooDeep { .. } => "xml_too_deep",
            CoreError::XmlTooManyElements { .. } => "xml_too_many_elements",
            CoreError::XmlMalformed(_) => "xml_malformed",
            CoreError::SubprocessNotFound { .. } => "subprocess_not_found",
            CoreError::SubprocessFailed { .. } => "subprocess_failed",
            CoreError::SubprocessTimeout { .. } => "subprocess_timeout",
            CoreError::SubprocessOutputTooLarge { .. } => "subprocess_output_too_large",
            CoreError::Lock { .. } => "lock",
            CoreError::BundleCacheMiss { .. } => "bundle_cache_miss",
        }
    }

    /// An optional remediation hint for agents (the envelope `hint`).
    pub fn hint(&self) -> Option<String> {
        match self {
            CoreError::SubprocessNotFound { install_hint, .. } => install_hint.clone(),
            CoreError::PathTraversal { .. } => {
                Some("use a path inside the project directory".into())
            }
            CoreError::XmlTooLarge { .. }
            | CoreError::XmlForbiddenEntity { .. }
            | CoreError::XmlTooDeep { .. }
            | CoreError::XmlTooManyElements { .. }
            | CoreError::XmlMalformed(_) => {
                Some("the input was rejected by the safe XML reader".into())
            }
            CoreError::SubprocessTimeout { .. } => {
                Some("increase the timeout or check that the tool is responsive".into())
            }
            _ => None,
        }
    }
}
