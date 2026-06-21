//! Project lifecycle: open/save, auto-save-on-close, `--dry-run`, file locking.
//!
//! Uses [`crate::security::path_guard`] on every load/save and a crash-atomic
//! write-temp-then-`rename` save (port of `guides/session-locking.md`). File
//! locking uses the standard library's `File::lock` (stabilized in Rust 1.89) —
//! so no extra locking crate is needed.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{CoreError, Result};
use crate::security::path_guard::guard_project_path;
use crate::session::Session;

/// Open a JSON project file into state `S` (path-guarded).
pub fn open_project<S: DeserializeOwned>(path: impl AsRef<Path>) -> Result<S> {
    let path = guard_project_path(None, path.as_ref())?;
    let bytes = std::fs::read(&path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Save state `S` to a JSON project file (path-guarded, atomic, locked).
/// Returns the normalized path written.
pub fn save_project<S: Serialize>(path: impl AsRef<Path>, state: &S) -> Result<PathBuf> {
    let path = guard_project_path(None, path.as_ref())?;
    locked_save_json(&path, state)?;
    Ok(path)
}

/// Crash-atomic, locked JSON save.
///
/// Writes to a sibling temp file in the same directory, `fsync`s it, then
/// atomically `rename`s it over the destination. A crash/panic/power-loss mid-
/// write can therefore never leave a truncated or partial project file — the
/// original stays intact until the rename commits. An exclusive advisory lock on
/// the destination serializes concurrent writers; if the OS/FS rejects locks it
/// proceeds unlocked (matching the Python fallback) and is still crash-atomic
/// thanks to the rename.
pub fn locked_save_json<T: Serialize>(path: &Path, data: &T) -> Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let json = serde_json::to_string_pretty(data)?;

    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent)?;
    }
    let dir = parent
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    // Best-effort exclusive lock on the destination to serialize concurrent savers.
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| CoreError::Lock {
            path: path.to_path_buf(),
            source: e,
        })?;
    let locked = lock_file.lock().is_ok();

    let result = atomic_write_via_rename(
        &dir,
        path,
        json.as_bytes(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    );

    if locked {
        let _ = lock_file.unlock();
    }
    result?;
    Ok(())
}

/// Write `bytes` to a temp file in `dir`, fsync, then rename over `path`.
fn atomic_write_via_rename(dir: &Path, path: &Path, bytes: &[u8], seq: u64) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    let tmp = dir.join(format!(".{file_name}.tmp.{}.{seq}", std::process::id()));

    {
        let mut tmp_file = File::create(&tmp)?;
        tmp_file.write_all(bytes)?;
        tmp_file.sync_all()?; // real fsync — durability before the rename commits
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    // Best-effort durability of the directory entry for the rename.
    if let Ok(dir_file) = File::open(dir) {
        let _ = dir_file.sync_all();
    }
    Ok(())
}

/// RAII guard that auto-saves a modified session on drop, unless suppressed.
///
/// Port of `guides/auto-save-dry-run.md`: one-shot commands hold the guard for
/// the command's lifetime so mutations are flushed before the process exits.
/// The REPL never installs the guard (it saves explicitly), and `--dry-run`
/// disables the save. The guard saves iff: armed, not dry-run, the session is
/// modified, and a project path is set.
pub struct AutoSaveGuard<'a, S: Serialize> {
    session: &'a mut Session<S>,
    dry_run: bool,
    armed: bool,
}

impl<'a, S: Serialize> AutoSaveGuard<'a, S> {
    /// Arm a guard over `session`. `dry_run = true` suppresses the save.
    pub fn new(session: &'a mut Session<S>, dry_run: bool) -> Self {
        Self {
            session,
            dry_run,
            armed: true,
        }
    }

    /// Borrow the guarded session.
    pub fn session(&mut self) -> &mut Session<S> {
        self.session
    }

    /// Disarm the guard (e.g. after an explicit save). No-op save on drop.
    pub fn disarm(&mut self) {
        self.armed = false;
    }

    /// Explicitly commit the session, propagating any save error.
    ///
    /// One-shot command handlers should call this (instead of relying solely on
    /// `Drop`) so a failed save reaches a non-zero exit / error envelope rather
    /// than being silently swallowed. Consumes the guard; on success the session
    /// is marked saved and `Drop` becomes a no-op. Returns the written path, or
    /// `None` if there was nothing to save (dry-run, unmodified, or no path).
    pub fn commit(mut self) -> Result<Option<PathBuf>> {
        self.armed = false; // prevent Drop from double-saving
        if self.dry_run || !self.session.modified() {
            return Ok(None);
        }
        let Some(path) = self.session.project_path().map(Path::to_path_buf) else {
            return Ok(None);
        };
        let Some(state) = self.session.state() else {
            return Ok(None);
        };
        locked_save_json(&path, state)?;
        self.session.mark_saved();
        Ok(Some(path))
    }
}

impl<S: Serialize> Drop for AutoSaveGuard<'_, S> {
    fn drop(&mut self) {
        if !self.armed || self.dry_run || !self.session.modified() {
            return;
        }
        let Some(path) = self.session.project_path().map(Path::to_path_buf) else {
            return;
        };
        let Some(state) = self.session.state() else {
            return;
        };
        // Best-effort safety net. Prefer `commit()` for fallible saves; if we
        // reach here and the save fails, fail LOUDLY (never silently lose data).
        match locked_save_json(&path, state) {
            Ok(()) => self.session.mark_saved(),
            Err(e) => eprintln!("✗ auto-save failed for {}: {e}", path.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Doc {
        title: String,
        n: i32,
    }

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_path(name: &str) -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("cli-anything-test-{}-{}", std::process::id(), id));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn save_then_open_round_trips() {
        let p = temp_path("doc.json");
        let doc = Doc {
            title: "hi".into(),
            n: 7,
        };
        let written = save_project(&p, &doc).unwrap();
        let back: Doc = open_project(&written).unwrap();
        assert_eq!(back, doc);
    }

    #[test]
    fn locked_save_truncates_previous_contents() {
        let p = temp_path("trunc.json");
        save_project(
            &p,
            &Doc {
                title: "longer-original".into(),
                n: 1,
            },
        )
        .unwrap();
        save_project(
            &p,
            &Doc {
                title: "x".into(),
                n: 2,
            },
        )
        .unwrap();
        let back: Doc = open_project(&p).unwrap();
        // If truncation failed, trailing bytes of the longer doc would corrupt JSON.
        assert_eq!(back.n, 2);
        assert_eq!(back.title, "x");
    }

    #[test]
    fn autosave_guard_saves_on_drop() {
        let p = temp_path("auto.json");
        let mut session: Session<Doc> = Session::new();
        session.open(
            Doc {
                title: "a".into(),
                n: 0,
            },
            &p,
        );
        // Seed the file so `open` semantics match (path set, not modified yet).
        save_project(&p, session.state().unwrap()).unwrap();
        session.mark_saved();
        {
            let mut guard = AutoSaveGuard::new(&mut session, false);
            guard.session().snapshot(None);
            guard.session().state_mut().unwrap().n = 42;
        } // drop -> autosave
        let back: Doc = open_project(&p).unwrap();
        assert_eq!(back.n, 42);
    }

    #[test]
    fn dry_run_guard_does_not_save() {
        let p = temp_path("dry.json");
        let mut session: Session<Doc> = Session::new();
        session.open(
            Doc {
                title: "a".into(),
                n: 0,
            },
            &p,
        );
        save_project(&p, session.state().unwrap()).unwrap();
        session.mark_saved();
        {
            let mut guard = AutoSaveGuard::new(&mut session, true); // dry-run
            guard.session().snapshot(None);
            guard.session().state_mut().unwrap().n = 99;
        }
        let back: Doc = open_project(&p).unwrap();
        assert_eq!(back.n, 0); // unchanged
    }

    #[test]
    fn commit_saves_and_returns_path() {
        let p = temp_path("commit.json");
        let mut session: Session<Doc> = Session::new();
        session.open(
            Doc {
                title: "a".into(),
                n: 0,
            },
            &p,
        );
        save_project(&p, session.state().unwrap()).unwrap();
        session.mark_saved();

        let written = {
            let mut guard = AutoSaveGuard::new(&mut session, false);
            guard.session().snapshot(None);
            guard.session().state_mut().unwrap().n = 7;
            guard.commit().unwrap()
        };
        assert_eq!(written.as_deref(), Some(p.as_path()));
        assert!(!session.modified()); // commit marked it saved
        let back: Doc = open_project(&p).unwrap();
        assert_eq!(back.n, 7);
    }

    #[test]
    fn commit_is_noop_when_unmodified() {
        let p = temp_path("commit-noop.json");
        let mut session: Session<Doc> = Session::new();
        session.open(
            Doc {
                title: "a".into(),
                n: 0,
            },
            &p,
        );
        let guard = AutoSaveGuard::new(&mut session, false);
        assert_eq!(guard.commit().unwrap(), None);
    }
}
