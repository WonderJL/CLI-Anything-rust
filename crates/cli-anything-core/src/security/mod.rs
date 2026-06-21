//! Safe-by-default utilities — the concrete form of the security motivation.
//!
//! - [`subprocess`] — run external tools via explicit args (never a shell string),
//!   with a timeout and PATH resolution → no shell injection.
//! - [`xml`]        — bounded, entity-safe `quick-xml` reader (the Rust analog of
//!   Python's `defusedxml`): reject DOCTYPE/entities and cap input size.
//! - [`path_guard`] — reject path traversal on project load/save.
//!
//! Phase B: full implementations; each maps to an item in the HARNESS-rs.md
//! security checklist.

pub mod path_guard;
pub mod subprocess;
pub mod xml;
