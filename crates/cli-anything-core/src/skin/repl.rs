//! reedline wiring: prompt, history file, and the line-input loop.
//!
//! A generated CLI implements [`ReplHandler`] and calls [`run_repl`]; the loop
//! renders the accent-colored prompt, reads a line, `shlex`-splits it (mirroring
//! the Python harness's `shlex` parsing), and dispatches. The REPL never
//! auto-saves — saving is the handler/CLI's responsibility.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use reedline::{
    FileBackedHistory, Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus,
    Reedline, Signal,
};

use crate::error::Result;
use crate::skin::Skin;

const HISTORY_CAPACITY: usize = 2000;

/// Whether the REPL loop should continue or exit after handling a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplOutcome {
    /// Keep looping.
    Continue,
    /// Leave the REPL.
    Exit,
}

/// A CLI implements this to drive its REPL via [`run_repl`].
pub trait ReplHandler {
    /// Current project label for the prompt (e.g. file stem); `None` if none open.
    fn project_label(&self) -> Option<String>;
    /// Whether there are unsaved changes (drives the `*` marker).
    fn modified(&self) -> bool;
    /// Handle one parsed (shlex-split) command line.
    fn handle(&mut self, args: &[String]) -> ReplOutcome;
}

/// The default history file path for a software, under `~/.cli-anything-<sw>/`.
pub fn default_history_path(software: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(
        PathBuf::from(home)
            .join(format!(".cli-anything-{software}"))
            .join("history"),
    )
}

/// The reedline prompt: `◆ <software> [<project>*] ❯`.
pub struct ReplPrompt {
    software: String,
    project: Option<String>,
    modified: bool,
}

impl Prompt for ReplPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        let project = self.project.as_deref().unwrap_or("no-project");
        let star = if self.modified { "*" } else { "" };
        Cow::Owned(format!("◆ {} [{project}{star}]", self.software))
    }
    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(" ❯ ")
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("… ")
    }
    fn render_prompt_history_search_indicator(&self, hs: PromptHistorySearch) -> Cow<'_, str> {
        let failing = matches!(hs.status, PromptHistorySearchStatus::Failing);
        let prefix = if failing { "failing " } else { "" };
        Cow::Owned(format!("({prefix}reverse-search: {}) ", hs.term))
    }
}

/// Build a reedline editor, attaching file-backed history when possible.
fn build_editor(history_path: Option<&Path>) -> Reedline {
    if let Some(path) = history_path {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(history) = FileBackedHistory::with_file(HISTORY_CAPACITY, path.to_path_buf()) {
            return Reedline::create().with_history(Box::new(history));
        }
    }
    Reedline::create()
}

/// Run the REPL loop, dispatching each parsed line to `handler`.
pub fn run_repl(
    skin: &Skin,
    history_path: Option<&Path>,
    handler: &mut dyn ReplHandler,
) -> Result<()> {
    skin.print_banner();
    let mut editor = build_editor(history_path);

    loop {
        // Snapshot prompt state each iteration so `handler` is not borrowed
        // across `read_line`.
        let prompt = ReplPrompt {
            software: skin.software().to_string(),
            project: handler.project_label(),
            modified: handler.modified(),
        };
        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let Some(parts) = shlex::split(&line) else {
                    skin.error("could not parse input (unbalanced quotes?)");
                    continue;
                };
                if parts.is_empty() {
                    continue;
                }
                if handler.handle(&parts) == ReplOutcome::Exit {
                    break;
                }
            }
            Ok(Signal::CtrlC) => continue,
            Ok(Signal::CtrlD) => break,
            Ok(_) => continue, // Signal is #[non_exhaustive]
            Err(e) => {
                skin.error(&format!("input error: {e}"));
                break;
            }
        }
    }
    skin.print_goodbye();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_left_shows_project_and_modified_marker() {
        let p = ReplPrompt {
            software: "mermaid".into(),
            project: Some("flow".into()),
            modified: true,
        };
        let left = p.render_prompt_left();
        assert!(left.contains("mermaid"));
        assert!(left.contains("flow"));
        assert!(left.contains('*'));
    }

    #[test]
    fn prompt_left_without_project_has_no_marker() {
        let p = ReplPrompt {
            software: "inkscape".into(),
            project: None,
            modified: false,
        };
        let left = p.render_prompt_left();
        assert!(left.contains("no-project"));
        assert!(!left.contains('*'));
    }

    #[test]
    fn shlex_parses_quoted_args() {
        let parts = shlex::split(r#"diagram set --text "hello world""#).unwrap();
        assert_eq!(parts, vec!["diagram", "set", "--text", "hello world"]);
    }
}
