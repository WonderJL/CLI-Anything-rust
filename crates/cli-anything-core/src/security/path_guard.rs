//! Path-traversal guard for project load/save.
//!
//! The core security property is: a crafted project path cannot use `..` to
//! escape its allowed directory. This is enforced *lexically* (without touching
//! the filesystem, so it works for not-yet-existing save targets) by normalizing
//! the path and verifying it stays within the allowed root.
//!
//! Threat model note: this defends against `..` traversal in supplied paths.
//! Symlink-based escapes (an attacker pre-planting a symlink) are a separate
//! threat handled by canonicalization at the call site when the target exists;
//! the lexical guard here is the always-applicable first line.

use std::path::{Component, Path, PathBuf};

use crate::error::{CoreError, Result};

/// Lexically normalize a path: drop `.`, resolve `..` by popping, keep root and
/// prefix. Never touches the filesystem.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Validate a project path against an allowed base.
///
/// - With `base = Some(root)`: the candidate (relative or absolute) must resolve
///   *within* `root`, else [`CoreError::PathTraversal`].
/// - With `base = None`: relative candidates must stay within the current working
///   directory (no climbing out via `..`); absolute candidates are allowed
///   (the user explicitly chose a location) but still normalized.
///
/// Returns the normalized absolute path to use for the actual IO.
pub fn guard_project_path(base: Option<&Path>, candidate: &Path) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;

    let root = match base {
        Some(b) => {
            let b = if b.is_absolute() {
                b.to_path_buf()
            } else {
                cwd.join(b)
            };
            lexical_normalize(&b)
        }
        None => lexical_normalize(&cwd),
    };

    let abs = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let normalized = lexical_normalize(&abs);

    let must_be_within = base.is_some() || !candidate.is_absolute();
    if must_be_within {
        // 1. Lexical containment (always applicable, works for non-existent targets).
        if !normalized.starts_with(&root) {
            return Err(CoreError::PathTraversal {
                path: candidate.to_path_buf(),
            });
        }
        // 2. Symlink backstop: resolve the existing path components on both sides
        //    and re-check containment, so a pre-planted symlink inside the base
        //    cannot redirect outside it. Both are resolved the same way so that
        //    e.g. macOS `/tmp -> /private/tmp` does not cause a false positive.
        let real_root = canonicalize_existing_ancestor(&root);
        let real_candidate = canonicalize_existing_ancestor(&normalized);
        if !real_candidate.starts_with(&real_root) {
            return Err(CoreError::PathTraversal {
                path: candidate.to_path_buf(),
            });
        }
    }
    Ok(normalized)
}

/// Resolve the longest existing prefix of `path` via `canonicalize` (following
/// symlinks) and re-attach the non-existent tail. For a fully non-existent path
/// this walks up to the first existing ancestor (ultimately `/`).
fn canonicalize_existing_ancestor(path: &Path) -> PathBuf {
    let mut ancestor: &Path = path;
    loop {
        if let Ok(real) = ancestor.canonicalize() {
            let rest = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return real.join(rest);
        }
        match ancestor.parent() {
            Some(parent) if parent != ancestor => ancestor = parent,
            _ => return path.to_path_buf(), // nothing along the path exists
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn relative_within_cwd_is_allowed() {
        let p = guard_project_path(None, Path::new("diagram.json")).unwrap();
        assert!(p.ends_with("diagram.json"));
        assert!(p.is_absolute());
    }

    #[test]
    fn relative_parent_escape_is_rejected() {
        let err = guard_project_path(None, Path::new("../../../../../../etc/passwd")).unwrap_err();
        assert_eq!(err.kind(), "path_traversal");
    }

    #[test]
    fn absolute_without_base_is_allowed() {
        let p = guard_project_path(None, Path::new("/tmp/out/foo.json")).unwrap();
        assert_eq!(p, PathBuf::from("/tmp/out/foo.json"));
    }

    #[test]
    fn absolute_outside_enforced_base_is_rejected() {
        let err = guard_project_path(Some(Path::new("/tmp/sandbox")), Path::new("/etc/passwd"))
            .unwrap_err();
        assert_eq!(err.kind(), "path_traversal");
    }

    #[test]
    fn within_enforced_base_is_allowed() {
        let p =
            guard_project_path(Some(Path::new("/tmp/sandbox")), Path::new("sub/a.json")).unwrap();
        assert_eq!(p, PathBuf::from("/tmp/sandbox/sub/a.json"));
    }

    #[test]
    fn parent_escape_from_enforced_base_is_rejected() {
        let err = guard_project_path(Some(Path::new("/tmp/sandbox")), Path::new("../secret"))
            .unwrap_err();
        assert_eq!(err.kind(), "path_traversal");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_from_enforced_base_is_rejected() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static C: AtomicUsize = AtomicUsize::new(0);
        let uniq = |tag: &str| {
            std::env::temp_dir().join(format!(
                "cli-anything-pg-{tag}-{}-{}",
                std::process::id(),
                C.fetch_add(1, Ordering::Relaxed)
            ))
        };
        let base = uniq("base");
        let outside = uniq("out");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        // Pre-plant a symlink inside the base that points outside it.
        let link = base.join("link");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        // Lexically `link/secret.json` is inside base, but it resolves outside.
        let err = guard_project_path(Some(&base), Path::new("link/secret.json")).unwrap_err();
        assert_eq!(err.kind(), "path_traversal");

        // A genuinely-inside (non-existent) path is still allowed.
        assert!(guard_project_path(Some(&base), Path::new("real/inside.json")).is_ok());
    }
}
