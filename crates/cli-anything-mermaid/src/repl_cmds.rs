//! Command dispatch shared by one-shot mode and the REPL.

use std::path::Path;
use std::process::ExitCode;

use clap::Parser;
use cli_anything_core::prelude::*;
use serde::Serialize;

use crate::backend;
use crate::cli::{Cli, Command, DiagramCmd, ExportCmd, PreviewCmd, ProjectCmd, SessionCmd};
use crate::domain::project::{self, Project};
use crate::preview_cmd;

/// SKILL metadata that can't be derived from the clap tree.
pub fn skill_meta() -> SkillMeta {
    SkillMeta {
        software: crate::SOFTWARE.to_string(),
        version: crate::VERSION.to_string(),
        description: "Author, render (mmdc or HTTP), and verify Mermaid diagrams.".to_string(),
        install_hint: Some("npm install -g @mermaid-js/mermaid-cli".to_string()),
        prereqs: vec!["mmdc (optional; HTTP fallback otherwise)".to_string()],
    }
}

fn ok<T: Serialize>(skin: &Skin, json: bool, action: &str, data: T, human: &str) {
    if json {
        Envelope::ok(action, data).print_json();
    } else {
        skin.success(human);
    }
}

fn core_err(skin: &Skin, json: bool, action: &str, e: &CoreError) -> i32 {
    if json {
        let env: Envelope<()> = Envelope::from_core_err(action, e);
        env.print_json();
    } else {
        skin.error(&e.to_string());
    }
    1
}

fn msg_err(skin: &Skin, json: bool, action: &str, kind: &str, msg: &str) -> i32 {
    if json {
        let env: Envelope<()> = Envelope::err(action, kind, msg, None);
        env.print_json();
    } else {
        skin.error(msg);
    }
    1
}

/// Emit a core error and map it to a process `ExitCode` (used by `run`).
pub fn fail(skin: &Skin, json: bool, action: &str, e: &CoreError) -> ExitCode {
    core_err(skin, json, action, e);
    ExitCode::FAILURE
}

/// Dispatch a one-shot command. Returns an exit code (0 = success).
pub fn dispatch(command: Command, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match command {
        Command::Project(cmd) => project_cmd(cmd, session, skin, json),
        Command::Diagram(cmd) => diagram_cmd(cmd, session, skin, json),
        Command::Export(cmd) => export_cmd(cmd, session, skin, json),
        Command::Preview(cmd) => preview_cmd_dispatch(cmd, session, skin, json),
        Command::Session(cmd) => session_cmd(cmd, session, skin, json),
        Command::EmitSkill { .. } => 0, // handled in run()
    }
}

fn project_cmd(cmd: ProjectCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        ProjectCmd::New {
            sample,
            theme,
            output,
        } => {
            session.set_state(Project::with_sample(&sample, &theme));
            if let Some(path) = output {
                let state = session.state().expect("state just set");
                match save_project(&path, state) {
                    Ok(saved) => {
                        session.set_project_path(&saved);
                        session.mark_saved();
                    }
                    Err(e) => return core_err(skin, json, "project.new", &e),
                }
            }
            ok(
                skin,
                json,
                "project.new",
                session.status(),
                "created a new project",
            );
            0
        }
        ProjectCmd::Open { path } => match open_project::<Project>(&path) {
            Ok(state) => {
                session.open(state, &path);
                ok(
                    skin,
                    json,
                    "project.open",
                    session.status(),
                    "opened project",
                );
                0
            }
            Err(e) => core_err(skin, json, "project.open", &e),
        },
        ProjectCmd::Save { path } => {
            let path = path.or_else(|| session.project_path().map(Path::to_path_buf));
            let Some(path) = path else {
                skin.error("no output path; pass a path or open a project first");
                return 1;
            };
            let Some(state) = session.state() else {
                skin.error("no project is open");
                return 1;
            };
            match save_project(&path, state) {
                Ok(saved) => {
                    session.mark_saved();
                    ok(
                        skin,
                        json,
                        "project.save",
                        serde_json::json!({ "path": saved.display().to_string() }),
                        "saved project",
                    );
                    0
                }
                Err(e) => core_err(skin, json, "project.save", &e),
            }
        }
        ProjectCmd::Info => {
            let data = match session.state() {
                Some(p) => serde_json::json!({
                    "status": session.status(),
                    "theme": p.theme(),
                    "code_lines": p.code.lines().count(),
                }),
                None => serde_json::json!({ "status": session.status() }),
            };
            ok(skin, json, "project.info", data, "project info");
            0
        }
        ProjectCmd::Samples => {
            ok(
                skin,
                json,
                "project.samples",
                serde_json::json!({ "samples": project::SAMPLES }),
                &format!("samples: {}", project::SAMPLES.join(", ")),
            );
            0
        }
    }
}

fn diagram_cmd(cmd: DiagramCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        DiagramCmd::Set { text, file } => {
            let source = match (text, file) {
                (Some(t), _) => t,
                (None, Some(f)) => match std::fs::read_to_string(&f) {
                    Ok(s) => s,
                    Err(e) => return msg_err(skin, json, "diagram.set", "io", &e.to_string()),
                },
                (None, None) => {
                    return msg_err(
                        skin,
                        json,
                        "diagram.set",
                        "usage",
                        "provide --text or --file",
                    )
                }
            };
            if !session.is_open() {
                session.set_state(Project::new());
            }
            session.snapshot(Some("diagram set"));
            session.state_mut().expect("open").code = source;
            ok(
                skin,
                json,
                "diagram.set",
                session.status(),
                "updated diagram source",
            );
            0
        }
        DiagramCmd::Show => match session.state() {
            Some(p) => {
                if json {
                    Envelope::ok("diagram.show", serde_json::json!({ "code": p.code }))
                        .print_json();
                } else {
                    println!("{}", p.code);
                }
                0
            }
            None => msg_err(
                skin,
                json,
                "diagram.show",
                "no_project",
                "no project is open",
            ),
        },
    }
}

fn export_cmd(cmd: ExportCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    let Some(project) = session.state() else {
        return msg_err(skin, json, "export", "no_project", "no project is open");
    };
    match cmd {
        ExportCmd::Render {
            output,
            format,
            overwrite,
        } => match backend::render(project, &output, format, overwrite) {
            Ok(result) => {
                let human = format!(
                    "rendered {} ({}, {} bytes) via {}",
                    result.output, result.format, result.file_size, result.method
                );
                ok(skin, json, "export.render", result, &human);
                0
            }
            Err(e) => msg_err(skin, json, "export.render", "render_error", &e.to_string()),
        },
        ExportCmd::Share { mode } => match backend::share_url(project, mode) {
            Ok(url) => {
                ok(
                    skin,
                    json,
                    "export.share",
                    serde_json::json!({ "url": url }),
                    &url,
                );
                0
            }
            Err(e) => msg_err(skin, json, "export.share", "encode_error", &e.to_string()),
        },
    }
}

fn preview_cmd_dispatch(
    cmd: PreviewCmd,
    session: &mut Session<Project>,
    skin: &Skin,
    json: bool,
) -> i32 {
    // The preview root is anchored to the open project's path (else $HOME). Clone
    // it first so the immutable borrow ends before we borrow `state()`.
    let project_path = session.project_path().map(Path::to_path_buf);
    match cmd {
        PreviewCmd::Capture {
            format,
            recipe,
            force,
        } => {
            let Some(project) = session.state() else {
                return msg_err(
                    skin,
                    json,
                    "preview.capture",
                    "no_project",
                    "no project is open",
                );
            };
            match preview_cmd::capture(project, project_path.as_deref(), &recipe, format, force) {
                Ok(result) => {
                    let human = if result.cached {
                        format!("preview cache hit: bundle {}", result.bundle_id)
                    } else {
                        format!(
                            "captured {} preview: bundle {} ({})",
                            result.format,
                            result.bundle_id,
                            result.render_method.as_deref().unwrap_or("?"),
                        )
                    };
                    ok(skin, json, "preview.capture", result, &human);
                    0
                }
                Err(e) => msg_err(
                    skin,
                    json,
                    "preview.capture",
                    "preview_error",
                    &e.to_string(),
                ),
            }
        }
        PreviewCmd::Status { recipe, recent } => {
            match preview_cmd::status(project_path.as_deref(), &recipe, recent) {
                Ok(status) => {
                    let human = format!(
                        "preview {} ({} steps)",
                        status.status, status.trajectory_summary.step_count
                    );
                    ok(skin, json, "preview.status", status, &human);
                    0
                }
                Err(e) => msg_err(
                    skin,
                    json,
                    "preview.status",
                    "preview_error",
                    &e.to_string(),
                ),
            }
        }
        PreviewCmd::List { recipe } => match preview_cmd::list(project_path.as_deref(), &recipe) {
            Ok(listing) => {
                let human = format!("{} bundle(s) in {}", listing.count, listing.root);
                ok(skin, json, "preview.list", listing, &human);
                0
            }
            Err(e) => msg_err(skin, json, "preview.list", "preview_error", &e.to_string()),
        },
    }
}

fn session_cmd(cmd: SessionCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        SessionCmd::Status => {
            ok(
                skin,
                json,
                "session.status",
                session.status(),
                "session status",
            );
            0
        }
        SessionCmd::Undo => {
            if session.undo() {
                ok(
                    skin,
                    json,
                    "session.undo",
                    session.status(),
                    "undid last change",
                );
            } else {
                skin.warning("nothing to undo");
            }
            0
        }
        SessionCmd::Redo => {
            if session.redo() {
                ok(skin, json, "session.redo", session.status(), "redid change");
            } else {
                skin.warning("nothing to redo");
            }
            0
        }
    }
}

/// REPL handler: re-parses each line through clap and dispatches. Never auto-saves.
pub struct Repl {
    skin: Skin,
    session: Session<Project>,
    json: bool,
}

impl Repl {
    /// Create a REPL handler.
    pub fn new(skin: Skin, session: Session<Project>, json: bool) -> Self {
        Self {
            skin,
            session,
            json,
        }
    }
}

impl ReplHandler for Repl {
    fn project_label(&self) -> Option<String> {
        self.session
            .project_path()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
    }

    fn modified(&self) -> bool {
        self.session.modified()
    }

    fn handle(&mut self, args: &[String]) -> ReplOutcome {
        match args.first().map(String::as_str) {
            Some("quit") | Some("exit") => return ReplOutcome::Exit,
            Some("help") => {
                self.skin.help(&[
                    ("project", "new / open / save / info / samples"),
                    ("diagram", "set --text|--file / show"),
                    ("export", "render <out> [-f svg|png] / share"),
                    ("preview", "capture [-f svg|png] / status / list"),
                    ("session", "status / undo / redo"),
                    ("quit", "exit the REPL"),
                ]);
                return ReplOutcome::Continue;
            }
            _ => {}
        }
        let argv = std::iter::once(crate::SOFTWARE.to_string()).chain(args.iter().cloned());
        match Cli::try_parse_from(argv) {
            Ok(parsed) => {
                if let Some(command) = parsed.command {
                    let json = self.json || parsed.json;
                    let _ = dispatch(command, &mut self.session, &self.skin, json);
                }
            }
            Err(e) => self.skin.error(&e.to_string()),
        }
        ReplOutcome::Continue
    }
}
