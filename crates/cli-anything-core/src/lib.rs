#![forbid(unsafe_code)]
//! `cli-anything-core` — the shared substrate for CLI-Anything-rust generated CLIs.
//!
//! This crate is the Rust-idiomatic replacement for the Python project's pattern
//! of vendoring `repl_skin.py` into every package: everything reusable lives here
//! exactly once, so a generated CLI is "thin" (a clap surface + domain logic + a
//! few calls into this crate).
//!
//! # Module map
//! - [`session`]      — session state + undo/redo snapshot stack (~50 levels)
//! - [`project`]      — open/save, auto-save-on-close, `--dry-run`, file locking
//! - [`json_envelope`]— the `--json` output contract (`{ok, action, data, error, warnings}`)
//! - [`skin`]         — reedline-based REPL skin (banner, prompt, messages, tables)
//! - [`preview`]      — full preview subsystem (bundle / session head / trajectory)
//! - [`security`]     — safe-by-default utilities (subprocess, XML, path guard)
//! - [`emit_skill`]   — generate `SKILL.md` by walking a clap `Command` tree
//! - [`error`]        — shared error types
//!
//! Phase A status: skeleton only. Module bodies are implemented in Phase B.

pub mod emit_skill;
pub mod error;
pub mod json_envelope;
pub mod preview;
pub mod project;
pub mod security;
pub mod session;
pub mod skin;

/// Common re-exports for generated CLIs (`use cli_anything_core::prelude::*;`).
pub mod prelude {
    pub use crate::emit_skill::{emit_skill, SkillMeta};
    pub use crate::error::{CoreError, Result};
    pub use crate::json_envelope::{Envelope, ErrInfo};
    pub use crate::project::{open_project, save_project, AutoSaveGuard};
    pub use crate::security::path_guard::guard_project_path;
    pub use crate::security::subprocess::{find_binary, require_binary, run, RunOutput};
    pub use crate::security::xml::{read_svg_safely, read_xml_safely_with, XmlLimits};
    pub use crate::session::{Session, SessionStatus, DEFAULT_MAX_UNDO};
    pub use crate::skin::repl::{run_repl, ReplHandler, ReplOutcome};
    pub use crate::skin::Skin;
}
