//! Safe subprocess execution: explicit args, timeout, no shell.
//!
//! Every external tool is invoked via `Command::new(exe).args(args)` — the
//! arguments are passed as a vector, never concatenated into a shell string, so
//! shell metacharacters in agent-supplied values cannot inject commands. The
//! resolved executable path is spawned (not the bare name) so the file that
//! passed the executability check is the file that runs. Each call is:
//! - **time-bounded** (`wait-timeout`) — and the child is spawned in its own
//!   process group (unix) for signal isolation;
//! - **memory-bounded** — captured stdout/stderr are byte-capped, and output is
//!   collected with a grace window so a grandchild holding the pipe open can
//!   never hang `run` past the timeout.
//!
//! Limitation: `kill` targets the direct child only. A descendant the child
//! forked (e.g. a headless renderer daemon) may outlive the timeout — full
//! process-group reaping needs `killpg`, which would require `unsafe`/an extra
//! crate and is deferred to keep `#![forbid(unsafe_code)]`.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::error::{CoreError, Result};

/// Captured result of a successful (exit-zero) subprocess run.
#[derive(Debug, Clone)]
pub struct RunOutput {
    /// Raw stdout bytes (e.g. rendered image data when a tool writes to stdout).
    pub stdout: Vec<u8>,
    /// Captured stderr, decoded lossily.
    pub stderr: String,
    /// The process exit code (0 on success).
    pub status_code: i32,
}

/// `which`-style lookup: resolve `program` to an executable path.
///
/// If `program` contains a path separator it is treated as an explicit path;
/// otherwise each `PATH` entry is searched for an executable file.
pub fn find_binary(program: &str) -> Option<PathBuf> {
    let p = Path::new(program);
    if p.is_absolute() || p.components().count() > 1 {
        return if is_executable(p) {
            Some(p.to_path_buf())
        } else {
            None
        };
    }
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(program);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Require that `program` exists, returning a typed error with an optional
/// install hint if it does not. Backends call this to fail loudly and clearly.
pub fn require_binary(program: &str, install_hint: Option<&str>) -> Result<PathBuf> {
    find_binary(program).ok_or_else(|| CoreError::SubprocessNotFound {
        program: program.to_string(),
        install_hint: install_hint.map(str::to_string),
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111 != 0))
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Hard cap on captured stdout (256 MiB). Exceeding it is a typed error.
pub const DEFAULT_MAX_STDOUT_BYTES: usize = 256 * 1024 * 1024;
/// Cap on captured stderr (256 KiB) — only the tail is ever surfaced.
const MAX_STDERR_BYTES: usize = 256 * 1024;
/// Grace window to collect reader output after the child finishes/dies, so a
/// grandchild holding the pipe open cannot hang `run` past the timeout.
const READER_GRACE: Duration = Duration::from_secs(2);

/// Run `program` with explicit `args`, bounded by `timeout`.
///
/// - Never uses a shell, so no injection is possible.
/// - On a missing binary: [`CoreError::SubprocessNotFound`].
/// - On timeout: the child is killed and [`CoreError::SubprocessTimeout`] is returned.
/// - On excessive output: [`CoreError::SubprocessOutputTooLarge`].
/// - On non-zero exit: [`CoreError::SubprocessFailed`] with the tail of stderr.
pub fn run(program: &str, args: &[&str], timeout: Duration) -> Result<RunOutput> {
    let exe = require_binary(program, None)?;

    let mut cmd = Command::new(&exe); // spawn the resolved path, not the bare name
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0); // own process group: signal isolation
    }
    let mut child = cmd.spawn()?;

    // Drain pipes on threads (bounded), delivering results over channels so we
    // can collect them with a grace window instead of an unbounded join.
    let mut out_pipe = child.stdout.take().expect("stdout piped");
    let mut err_pipe = child.stderr.take().expect("stderr piped");
    let (otx, orx) = mpsc::channel();
    let (etx, erx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = otx.send(read_capped(&mut out_pipe, DEFAULT_MAX_STDOUT_BYTES));
    });
    std::thread::spawn(move || {
        let _ = etx.send(read_capped(&mut err_pipe, MAX_STDERR_BYTES));
    });

    let status = match child.wait_timeout(timeout)? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            // Do NOT block joining the readers — a surviving grandchild could
            // hold the pipe open forever. Detach them and return promptly.
            return Err(CoreError::SubprocessTimeout {
                program: program.to_string(),
                seconds: timeout.as_secs(),
            });
        }
    };

    // Collect with a bounded grace so a success-path grandchild can't hang us.
    let (stdout, out_truncated) = orx
        .recv_timeout(READER_GRACE)
        .unwrap_or((Vec::new(), false));
    let (err_bytes, _) = erx
        .recv_timeout(READER_GRACE)
        .unwrap_or((Vec::new(), false));

    if out_truncated {
        return Err(CoreError::SubprocessOutputTooLarge {
            program: program.to_string(),
            limit: DEFAULT_MAX_STDOUT_BYTES,
        });
    }

    let stderr = String::from_utf8_lossy(&err_bytes).into_owned();
    let code = status.code().unwrap_or(-1);
    if code != 0 {
        return Err(CoreError::SubprocessFailed {
            program: program.to_string(),
            code,
            stderr: tail_chars(&stderr, 2000),
        });
    }

    Ok(RunOutput {
        stdout,
        stderr,
        status_code: code,
    })
}

/// Read up to `cap` bytes from `reader`. Returns `(bytes, truncated)` where
/// `truncated` is true if the cap was hit (reading then stops).
fn read_capped(reader: &mut impl Read, cap: usize) -> (Vec<u8>, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => return (buf, false),
            Ok(n) => {
                if buf.len() + n > cap {
                    let take = cap.saturating_sub(buf.len());
                    buf.extend_from_slice(&chunk[..take]);
                    return (buf, true);
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return (buf, false),
        }
    }
}

/// Keep the last `n` characters of `s` (for bounded stderr in error messages).
fn tail_chars(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        chars[chars.len() - n..].iter().collect()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

    const T: Duration = Duration::from_secs(10);

    #[test]
    fn runs_and_captures_stdout() {
        let out = run("echo", &["hello"], T).unwrap();
        assert_eq!(out.status_code, 0);
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
    }

    #[test]
    fn args_are_never_shell_interpreted() {
        // If a shell were involved, "$(whoami)" would be substituted. It must not be.
        let out = run("echo", &["$(whoami)"], T).unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "$(whoami)");
    }

    #[test]
    fn nonzero_exit_is_a_typed_error() {
        let err = run("false", &[], T).unwrap_err();
        assert_eq!(err.kind(), "subprocess_failed");
    }

    #[test]
    fn missing_binary_is_a_typed_error_with_hint() {
        let err = require_binary(
            "definitely-not-a-real-binary-xyz",
            Some("brew install nope"),
        )
        .unwrap_err();
        assert_eq!(err.kind(), "subprocess_not_found");
        assert_eq!(err.hint().as_deref(), Some("brew install nope"));
    }

    #[test]
    fn slow_child_times_out() {
        let err = run("sleep", &["5"], Duration::from_millis(150)).unwrap_err();
        assert_eq!(err.kind(), "subprocess_timeout");
    }

    #[test]
    fn read_capped_truncates_at_limit() {
        let data = vec![b'x'; 10_000];
        let (out, truncated) = read_capped(&mut std::io::Cursor::new(&data), 4096);
        assert!(truncated);
        assert_eq!(out.len(), 4096);

        let (out2, truncated2) = read_capped(&mut std::io::Cursor::new(&data), 1_000_000);
        assert!(!truncated2);
        assert_eq!(out2.len(), 10_000);
    }
}
