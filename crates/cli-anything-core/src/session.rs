//! Session state + undo/redo snapshot stack.
//!
//! `Session<S>` holds a serde-serializable project state `S` plus a bounded
//! undo/redo history. Deep-copy snapshots use `Clone` (serde state structs
//! derive `Clone`); the stack is capped at [`DEFAULT_MAX_UNDO`] with FIFO
//! eviction of the oldest entry — matching the Python original's 50-level cap
//! (mermaid's Python `Session` was uncapped; we standardize on 50 for both).

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Default undo depth, matching the inkscape Python harness's `_undo_stack` cap.
pub const DEFAULT_MAX_UNDO: usize = 50;

/// A mutable editing session over a project state `S`.
#[derive(Debug)]
pub struct Session<S> {
    state: Option<S>,
    project_path: Option<PathBuf>,
    modified: bool,
    undo_stack: Vec<S>,
    redo_stack: Vec<S>,
    max_undo: usize,
    /// Breadcrumb log of applied operations (independent of undo/redo), capped.
    history: Vec<String>,
}

impl<S> Default for Session<S> {
    fn default() -> Self {
        Self {
            state: None,
            project_path: None,
            modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_undo: DEFAULT_MAX_UNDO,
            history: Vec::new(),
        }
    }
}

/// Machine-readable session status (for `session status --json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionStatus {
    /// Whether a project is currently open.
    pub open: bool,
    /// The on-disk project path, if any.
    pub project_path: Option<String>,
    /// Whether there are unsaved changes.
    pub modified: bool,
    /// Number of undo steps available.
    pub undo_depth: usize,
    /// Number of redo steps available.
    pub redo_depth: usize,
    /// The configured undo cap.
    pub max_undo: usize,
}

impl<S> Session<S> {
    /// A fresh, empty session with the default undo cap.
    pub fn new() -> Self {
        Self::default()
    }

    /// A fresh session with a custom undo cap (minimum 1).
    pub fn with_max_undo(max_undo: usize) -> Self {
        Self {
            max_undo: max_undo.max(1),
            ..Self::default()
        }
    }

    /// Open `state` loaded from `path` (clears history, not modified).
    pub fn open(&mut self, state: S, path: impl Into<PathBuf>) {
        self.state = Some(state);
        self.project_path = Some(path.into());
        self.modified = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.history.clear();
    }

    /// Install fresh state (e.g. `project new`), clearing undo/redo/history so a
    /// later redo can never resurrect a previous project's snapshots.
    pub fn set_state(&mut self, state: S) {
        self.state = Some(state);
        self.modified = true;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.history.clear();
    }

    /// Whether a project is open.
    pub fn is_open(&self) -> bool {
        self.state.is_some()
    }

    /// Borrow the current state.
    pub fn state(&self) -> Option<&S> {
        self.state.as_ref()
    }

    /// Mutably borrow the current state. (Snapshot first if you want undo.)
    pub fn state_mut(&mut self) -> Option<&mut S> {
        self.state.as_mut()
    }

    /// The on-disk project path, if set.
    pub fn project_path(&self) -> Option<&Path> {
        self.project_path.as_deref()
    }

    /// Set or change the on-disk project path.
    pub fn set_project_path(&mut self, path: impl Into<PathBuf>) {
        self.project_path = Some(path.into());
    }

    /// Whether there are unsaved changes.
    pub fn modified(&self) -> bool {
        self.modified
    }

    /// Mark the session dirty.
    pub fn mark_modified(&mut self) {
        self.modified = true;
    }

    /// Mark the session clean (after a successful save).
    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    /// The recent-operation breadcrumb log.
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Snapshot the current state onto the undo stack *before* a mutation.
    ///
    /// Deep-copies via `Clone`, evicts the oldest entry past the cap (FIFO),
    /// clears the redo stack, records `description` in the breadcrumb log, and
    /// marks the session modified.
    pub fn snapshot(&mut self, description: Option<&str>)
    where
        S: Clone,
    {
        if let Some(state) = self.state.as_ref() {
            self.undo_stack.push(state.clone());
            while self.undo_stack.len() > self.max_undo {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
        }
        if let Some(desc) = description {
            self.history.push(desc.to_string());
            while self.history.len() > self.max_undo {
                self.history.remove(0);
            }
        }
        self.modified = true;
    }

    /// Restore the previous state, pushing the current onto the redo stack.
    /// Returns `false` if there was nothing to undo.
    pub fn undo(&mut self) -> bool
    where
        S: Clone,
    {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        if let Some(cur) = self.state.take() {
            self.redo_stack.push(cur);
        }
        self.state = Some(prev);
        self.modified = true;
        true
    }

    /// Re-apply a previously undone state. Returns `false` if nothing to redo.
    pub fn redo(&mut self) -> bool
    where
        S: Clone,
    {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        if let Some(cur) = self.state.take() {
            self.undo_stack.push(cur);
        }
        self.state = Some(next);
        self.modified = true;
        true
    }

    /// A snapshot of session status for `--json` output.
    pub fn status(&self) -> SessionStatus {
        SessionStatus {
            open: self.is_open(),
            project_path: self
                .project_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            modified: self.modified,
            undo_depth: self.undo_stack.len(),
            redo_depth: self.redo_stack.len(),
            max_undo: self.max_undo,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_session(start: i32) -> Session<i32> {
        let mut s = Session::new();
        s.open(start, "mem.json");
        s
    }

    #[test]
    fn undo_redo_round_trip() {
        let mut s = open_session(0);
        s.snapshot(Some("set 1"));
        *s.state_mut().unwrap() = 1;
        s.snapshot(Some("set 2"));
        *s.state_mut().unwrap() = 2;
        assert_eq!(*s.state().unwrap(), 2);

        assert!(s.undo());
        assert_eq!(*s.state().unwrap(), 1);
        assert!(s.undo());
        assert_eq!(*s.state().unwrap(), 0);
        assert!(!s.undo()); // empty

        assert!(s.redo());
        assert_eq!(*s.state().unwrap(), 1);
        assert!(s.redo());
        assert_eq!(*s.state().unwrap(), 2);
        assert!(!s.redo()); // empty
    }

    #[test]
    fn new_snapshot_clears_redo() {
        let mut s = open_session(0);
        s.snapshot(None);
        *s.state_mut().unwrap() = 1;
        assert!(s.undo());
        assert_eq!(s.status().redo_depth, 1);
        // A new edit must invalidate the redo branch.
        s.snapshot(None);
        *s.state_mut().unwrap() = 9;
        assert_eq!(s.status().redo_depth, 0);
        assert!(!s.redo());
    }

    #[test]
    fn undo_stack_is_capped_fifo() {
        let mut s = Session::with_max_undo(3);
        s.open(0, "mem.json");
        for i in 1..=10 {
            s.snapshot(Some(&format!("step {i}")));
            *s.state_mut().unwrap() = i;
        }
        assert_eq!(s.status().undo_depth, 3); // capped
                                              // Only the most recent 3 snapshots survive (states 7,8,9 then current 10).
        assert!(s.undo());
        assert_eq!(*s.state().unwrap(), 9);
        assert!(s.undo());
        assert_eq!(*s.state().unwrap(), 8);
        assert!(s.undo());
        assert_eq!(*s.state().unwrap(), 7);
        assert!(!s.undo());
    }

    #[test]
    fn modified_tracking() {
        let mut s = open_session(0);
        assert!(!s.modified());
        s.snapshot(None);
        assert!(s.modified());
        s.mark_saved();
        assert!(!s.modified());
    }

    #[test]
    fn status_shape() {
        let mut s = open_session(0);
        s.snapshot(Some("a"));
        let st = s.status();
        assert!(st.open);
        assert_eq!(st.max_undo, DEFAULT_MAX_UNDO);
        assert_eq!(st.undo_depth, 1);
        assert_eq!(st.project_path.as_deref(), Some("mem.json"));
    }

    #[test]
    fn set_state_clears_history_so_redo_cannot_cross_projects() {
        let mut s = open_session(0);
        s.snapshot(None);
        *s.state_mut().unwrap() = 1;
        assert!(s.undo()); // state back to 0, redo=[1]
        assert_eq!(s.status().redo_depth, 1);
        // A brand-new project must not be redo-able into the old project's state.
        s.set_state(99);
        assert_eq!(s.status().redo_depth, 0);
        assert_eq!(s.status().undo_depth, 0);
        assert!(!s.redo());
        assert_eq!(*s.state().unwrap(), 99);
    }
}
