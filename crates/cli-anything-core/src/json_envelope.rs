//! The `--json` output contract.
//!
//! Every command emits the same [`Envelope`] shape so agents get a uniform
//! `ok` / `error` contract. The Python originals returned bare, per-command
//! dicts; this wrapper is a strict superset (documented in `HARNESS-rs.md` and
//! each `SKILL.md`) — an improvement, not a divergence in spirit.
//!
//! The shape is intentionally *uniform*: `data`, `error`, and `warnings` keys
//! are always present (null / `[]` when empty) so a consumer never has to probe
//! for missing keys.

use serde::Serialize;

use crate::error::CoreError;

/// Uniform machine-readable result wrapper printed under `--json`.
#[derive(Debug, Serialize)]
pub struct Envelope<T: Serialize> {
    /// `true` on success, `false` on error.
    pub ok: bool,
    /// Dotted action identifier, e.g. `"project.new"`, `"export.render"`.
    pub action: String,
    /// Command-specific payload; `null` on error.
    pub data: Option<T>,
    /// Populated on failure; `null` on success.
    pub error: Option<ErrInfo>,
    /// Non-fatal notes (always present, `[]` when empty).
    pub warnings: Vec<String>,
}

/// Structured error detail inside a failed [`Envelope`].
#[derive(Debug, Serialize)]
pub struct ErrInfo {
    /// Stable snake_case machine identifier (see [`CoreError::kind`]).
    pub kind: String,
    /// Human-readable message.
    pub message: String,
    /// Optional remediation hint.
    pub hint: Option<String>,
}

impl<T: Serialize> Envelope<T> {
    /// Build a success envelope carrying `data`.
    pub fn ok(action: impl Into<String>, data: T) -> Self {
        Self {
            ok: true,
            action: action.into(),
            data: Some(data),
            error: None,
            warnings: Vec::new(),
        }
    }

    /// Build an error envelope (no data) from explicit parts.
    pub fn err(
        action: impl Into<String>,
        kind: impl Into<String>,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        Self {
            ok: false,
            action: action.into(),
            data: None,
            error: Some(ErrInfo {
                kind: kind.into(),
                message: message.into(),
                hint,
            }),
            warnings: Vec::new(),
        }
    }

    /// Build an error envelope from a [`CoreError`], mapping its `kind`/`hint`.
    pub fn from_core_err(action: impl Into<String>, err: &CoreError) -> Self {
        Self::err(action, err.kind(), err.to_string(), err.hint())
    }

    /// Attach non-fatal warnings (builder style).
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }

    /// The process exit code this envelope implies: `0` on success, `1` on error.
    pub fn exit_code(&self) -> i32 {
        i32::from(!self.ok)
    }

    /// Serialize to a compact JSON string. Serialization is infallible for any
    /// `T: Serialize` whose `Serialize` impl does not itself error.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("envelope is always serializable")
    }

    /// Print the JSON form to stdout (the `--json` path).
    pub fn print_json(&self) {
        println!("{}", self.to_json());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn ok_envelope_has_uniform_shape() {
        let env = Envelope::ok("project.new", serde_json::json!({ "path": "a.json" }));
        assert_eq!(env.exit_code(), 0);
        let v: Value = serde_json::from_str(&env.to_json()).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["action"], "project.new");
        assert_eq!(v["data"]["path"], "a.json");
        assert!(v["error"].is_null());
        assert!(v["warnings"].is_array());
        assert_eq!(v["warnings"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn err_envelope_has_uniform_shape() {
        let env: Envelope<()> = Envelope::err(
            "export.render",
            "subprocess_failed",
            "boom",
            Some("install mmdc".into()),
        );
        assert_eq!(env.exit_code(), 1);
        let v: Value = serde_json::from_str(&env.to_json()).unwrap();
        assert_eq!(v["ok"], false);
        assert!(v["data"].is_null());
        assert_eq!(v["error"]["kind"], "subprocess_failed");
        assert_eq!(v["error"]["message"], "boom");
        assert_eq!(v["error"]["hint"], "install mmdc");
    }

    #[test]
    fn from_core_err_maps_kind_and_hint() {
        let e = CoreError::PathTraversal {
            path: "../etc/passwd".into(),
        };
        let env: Envelope<()> = Envelope::from_core_err("project.open", &e);
        let v: Value = serde_json::from_str(&env.to_json()).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["kind"], "path_traversal");
        assert!(v["error"]["hint"].is_string());
    }

    #[test]
    fn warnings_round_trip() {
        let env = Envelope::ok("session.status", serde_json::json!({}))
            .with_warnings(vec!["unsaved changes".into()]);
        let v: Value = serde_json::from_str(&env.to_json()).unwrap();
        assert_eq!(v["warnings"][0], "unsaved changes");
    }
}
