//! Command dispatch shared by one-shot mode and the REPL.

use std::path::Path;
use std::process::ExitCode;

use clap::Parser;
use cli_anything_core::prelude::*;
use serde::Serialize;

use crate::backend;
use crate::cli::{
    Cli, Command, DocumentCmd, ExportCmd, GradientCmd, LayerCmd, PathCmd, SessionCmd, ShapeCmd,
    StyleCmd, TextCmd, TransformCmd,
};
use crate::domain::project::{Gradient, GradientStop, Project, Shape};

/// SKILL metadata that can't be derived from the clap tree.
pub fn skill_meta() -> SkillMeta {
    SkillMeta {
        software: crate::SOFTWARE.to_string(),
        version: crate::VERSION.to_string(),
        description:
            "Build SVG documents, export via real inkscape, and safely import untrusted SVG."
                .to_string(),
        install_hint: Some("install Inkscape from https://inkscape.org".to_string()),
        prereqs: vec!["inkscape (for PNG/PDF export)".to_string()],
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
        Command::Document(c) => document(c, session, skin, json),
        Command::Shape(c) => shape(c, session, skin, json),
        Command::Text(c) => text(c, session, skin, json),
        Command::Style(c) => style(c, session, skin, json),
        Command::Transform(c) => transform(c, session, skin, json),
        Command::Layer(c) => layer(c, session, skin, json),
        Command::Gradient(c) => gradient(c, session, skin, json),
        Command::Path(c) => path(c, session, skin, json),
        Command::Export(c) => export(c, session, skin, json),
        Command::Session(c) => session_cmd(c, session, skin, json),
        Command::EmitSkill { .. } => 0,
    }
}

// ----- helpers shared by mutating commands -------------------------------------

fn need_open(session: &Session<Project>, skin: &Skin, json: bool, action: &str) -> bool {
    if session.is_open() {
        true
    } else {
        msg_err(
            skin,
            json,
            action,
            "no_document",
            "no document is open (run `document new`)",
        );
        false
    }
}

fn check_index(
    session: &Session<Project>,
    skin: &Skin,
    json: bool,
    action: &str,
    index: usize,
) -> bool {
    if session.state().is_some_and(|p| index < p.objects.len()) {
        true
    } else {
        msg_err(
            skin,
            json,
            action,
            "bad_index",
            &format!("no object at index {index}"),
        );
        false
    }
}

// ----- document ----------------------------------------------------------------

fn document(cmd: DocumentCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        DocumentCmd::New {
            width,
            height,
            units,
            background,
            name,
            output,
        } => {
            session.set_state(Project::with_canvas(
                width,
                height,
                &units,
                &background,
                &name,
            ));
            if let Some(path) = output {
                let state = session.state().expect("just set");
                match save_project(&path, state) {
                    Ok(saved) => {
                        session.set_project_path(&saved);
                        session.mark_saved();
                    }
                    Err(e) => return core_err(skin, json, "document.new", &e),
                }
            }
            ok(
                skin,
                json,
                "document.new",
                session.status(),
                "created a new document",
            );
            0
        }
        DocumentCmd::Open { path } => match open_project::<Project>(&path) {
            Ok(state) => {
                session.open(state, &path);
                ok(
                    skin,
                    json,
                    "document.open",
                    session.status(),
                    "opened document",
                );
                0
            }
            Err(e) => core_err(skin, json, "document.open", &e),
        },
        DocumentCmd::Save { path } => {
            let path = path.or_else(|| session.project_path().map(Path::to_path_buf));
            let Some(path) = path else {
                return msg_err(
                    skin,
                    json,
                    "document.save",
                    "no_path",
                    "pass a path or open a document",
                );
            };
            let Some(state) = session.state() else {
                return msg_err(
                    skin,
                    json,
                    "document.save",
                    "no_document",
                    "no document is open",
                );
            };
            match save_project(&path, state) {
                Ok(saved) => {
                    session.mark_saved();
                    ok(
                        skin,
                        json,
                        "document.save",
                        serde_json::json!({"path": saved.display().to_string()}),
                        "saved document",
                    );
                    0
                }
                Err(e) => core_err(skin, json, "document.save", &e),
            }
        }
        DocumentCmd::Info => {
            let data = match session.state() {
                Some(p) => serde_json::json!({
                    "status": session.status(),
                    "name": p.name,
                    "canvas": p.canvas,
                    "objects": p.objects.len(),
                    "layers": p.layers.len(),
                    "gradients": p.gradients.len(),
                }),
                None => serde_json::json!({ "status": session.status() }),
            };
            ok(skin, json, "document.info", data, "document info");
            0
        }
        DocumentCmd::Json => match session.state() {
            Some(p) => {
                println!("{}", serde_json::to_string_pretty(p).unwrap_or_default());
                0
            }
            None => msg_err(
                skin,
                json,
                "document.json",
                "no_document",
                "no document is open",
            ),
        },
        DocumentCmd::CanvasSize { width, height } => {
            if !need_open(session, skin, json, "document.canvas-size") {
                return 1;
            }
            session.snapshot(Some("canvas-size"));
            let p = session.state_mut().unwrap();
            p.canvas.width = width;
            p.canvas.height = height;
            ok(
                skin,
                json,
                "document.canvas-size",
                session.status(),
                "resized canvas",
            );
            0
        }
        DocumentCmd::Units { units } => {
            if !need_open(session, skin, json, "document.units") {
                return 1;
            }
            session.snapshot(Some("units"));
            session.state_mut().unwrap().canvas.units = units;
            ok(skin, json, "document.units", session.status(), "set units");
            0
        }
        DocumentCmd::Import { path } => match backend::import_svg_safely(&path) {
            Ok(len) => {
                ok(
                    skin,
                    json,
                    "document.import",
                    serde_json::json!({ "safe": true, "bytes": len, "path": path.display().to_string() }),
                    &format!("imported safely: {len} bytes passed the SVG safety checks"),
                );
                0
            }
            // Malicious/untrusted SVG rejected here — the security showcase.
            Err(e) => msg_err(skin, json, "document.import", "unsafe_svg", &e.to_string()),
        },
    }
}

// ----- shape -------------------------------------------------------------------

fn add_shape(session: &mut Session<Project>, skin: &Skin, json: bool, shape: Shape) -> i32 {
    if !need_open(session, skin, json, "shape.add") {
        return 1;
    }
    session.snapshot(Some("add shape"));
    let kind = shape.kind();
    let id = session.state_mut().unwrap().add_shape(shape);
    let index = session.state().unwrap().objects.len() - 1;
    ok(
        skin,
        json,
        "shape.add",
        serde_json::json!({ "id": id, "index": index, "kind": kind }),
        &format!("added {kind} as #{index} ({id})"),
    );
    0
}

fn shape(cmd: ShapeCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        ShapeCmd::AddRect {
            x,
            y,
            width,
            height,
            rx,
        } => add_shape(
            session,
            skin,
            json,
            Shape::Rect {
                x,
                y,
                width,
                height,
                rx,
            },
        ),
        ShapeCmd::AddCircle { cx, cy, r } => {
            add_shape(session, skin, json, Shape::Circle { cx, cy, r })
        }
        ShapeCmd::AddEllipse { cx, cy, rx, ry } => {
            add_shape(session, skin, json, Shape::Ellipse { cx, cy, rx, ry })
        }
        ShapeCmd::AddLine { x1, y1, x2, y2 } => {
            add_shape(session, skin, json, Shape::Line { x1, y1, x2, y2 })
        }
        ShapeCmd::AddPolygon { points } => {
            add_shape(session, skin, json, Shape::Polygon { points })
        }
        ShapeCmd::AddPath { d } => add_shape(session, skin, json, Shape::Path { d }),
        ShapeCmd::AddStar { cx, cy, r, points } => {
            add_shape(session, skin, json, Shape::Star { cx, cy, r, points })
        }
        ShapeCmd::Remove { index } => {
            if !check_index(session, skin, json, "shape.remove", index) {
                return 1;
            }
            session.snapshot(Some("remove"));
            session.state_mut().unwrap().remove(index);
            ok(
                skin,
                json,
                "shape.remove",
                session.status(),
                &format!("removed #{index}"),
            );
            0
        }
        ShapeCmd::Duplicate { index } => {
            if !check_index(session, skin, json, "shape.duplicate", index) {
                return 1;
            }
            session.snapshot(Some("duplicate"));
            let id = session.state_mut().unwrap().duplicate(index);
            ok(
                skin,
                json,
                "shape.duplicate",
                serde_json::json!({ "id": id }),
                "duplicated object",
            );
            0
        }
        ShapeCmd::List => {
            let Some(p) = session.state() else {
                return msg_err(
                    skin,
                    json,
                    "shape.list",
                    "no_document",
                    "no document is open",
                );
            };
            let items: Vec<_> = p
                .objects
                .iter()
                .enumerate()
                .map(|(i, o)| serde_json::json!({ "index": i, "id": o.id, "kind": o.shape.kind(), "layer": o.layer }))
                .collect();
            ok(
                skin,
                json,
                "shape.list",
                serde_json::json!({ "objects": items }),
                &format!("{} objects", p.objects.len()),
            );
            0
        }
        ShapeCmd::Get { index } => {
            if !check_index(session, skin, json, "shape.get", index) {
                return 1;
            }
            let obj = session.state().unwrap().object(index).unwrap();
            ok(skin, json, "shape.get", obj, &format!("object #{index}"));
            0
        }
    }
}

// ----- text --------------------------------------------------------------------

fn text(cmd: TextCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        TextCmd::Add {
            x,
            y,
            content,
            font_size,
        } => add_shape(
            session,
            skin,
            json,
            Shape::Text {
                x,
                y,
                content,
                font_size,
            },
        ),
        TextCmd::List => {
            let Some(p) = session.state() else {
                return msg_err(
                    skin,
                    json,
                    "text.list",
                    "no_document",
                    "no document is open",
                );
            };
            let items: Vec<_> = p
                .objects
                .iter()
                .enumerate()
                .filter(|(_, o)| matches!(o.shape, Shape::Text { .. }))
                .map(|(i, o)| serde_json::json!({ "index": i, "id": o.id }))
                .collect();
            ok(
                skin,
                json,
                "text.list",
                serde_json::json!({ "text": items }),
                "text objects",
            );
            0
        }
    }
}

// ----- style -------------------------------------------------------------------

fn style(cmd: StyleCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        StyleCmd::SetFill { index, color } => {
            mutate_object(session, skin, json, "style.set-fill", index, |o| {
                o.style.fill = Some(color);
            })
        }
        StyleCmd::SetStroke {
            index,
            color,
            width,
        } => mutate_object(session, skin, json, "style.set-stroke", index, |o| {
            o.style.stroke = Some(color);
            o.style.stroke_width = Some(width);
        }),
        StyleCmd::SetOpacity { index, opacity } => {
            mutate_object(session, skin, json, "style.set-opacity", index, |o| {
                o.style.opacity = Some(opacity.clamp(0.0, 1.0));
            })
        }
        StyleCmd::Get { index } => {
            if !check_index(session, skin, json, "style.get", index) {
                return 1;
            }
            let style = &session.state().unwrap().object(index).unwrap().style;
            ok(
                skin,
                json,
                "style.get",
                style,
                &format!("style of #{index}"),
            );
            0
        }
    }
}

// ----- transform ---------------------------------------------------------------

fn compose(existing: &Option<String>, new: String) -> String {
    match existing {
        Some(e) if !e.is_empty() => format!("{e} {new}"),
        _ => new,
    }
}

fn transform(cmd: TransformCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        TransformCmd::Translate { index, dx, dy } => {
            mutate_object(session, skin, json, "transform.translate", index, |o| {
                o.transform = Some(compose(&o.transform, format!("translate({dx} {dy})")));
            })
        }
        TransformCmd::Rotate {
            index,
            degrees,
            cx,
            cy,
        } => mutate_object(session, skin, json, "transform.rotate", index, |o| {
            let t = match (cx, cy) {
                (Some(x), Some(y)) => format!("rotate({degrees} {x} {y})"),
                _ => format!("rotate({degrees})"),
            };
            o.transform = Some(compose(&o.transform, t));
        }),
        TransformCmd::Scale { index, sx, sy } => {
            mutate_object(session, skin, json, "transform.scale", index, |o| {
                o.transform = Some(compose(&o.transform, format!("scale({sx} {sy})")));
            })
        }
        TransformCmd::Get { index } => {
            if !check_index(session, skin, json, "transform.get", index) {
                return 1;
            }
            let t = session
                .state()
                .unwrap()
                .object(index)
                .unwrap()
                .transform
                .clone();
            ok(
                skin,
                json,
                "transform.get",
                serde_json::json!({ "transform": t }),
                "transform",
            );
            0
        }
        TransformCmd::Clear { index } => {
            mutate_object(session, skin, json, "transform.clear", index, |o| {
                o.transform = None;
            })
        }
    }
}

fn mutate_object<F: FnOnce(&mut crate::domain::project::Object)>(
    session: &mut Session<Project>,
    skin: &Skin,
    json: bool,
    action: &str,
    index: usize,
    f: F,
) -> i32 {
    if !check_index(session, skin, json, action, index) {
        return 1;
    }
    session.snapshot(Some(action));
    f(session.state_mut().unwrap().object_mut(index).unwrap());
    ok(
        skin,
        json,
        action,
        session.status(),
        &format!("{action} on #{index}"),
    );
    0
}

// ----- layer -------------------------------------------------------------------

fn layer(cmd: LayerCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        LayerCmd::Add { label } => {
            if !need_open(session, skin, json, "layer.add") {
                return 1;
            }
            session.snapshot(Some("add layer"));
            let id = session.state_mut().unwrap().add_layer(&label);
            ok(
                skin,
                json,
                "layer.add",
                serde_json::json!({ "id": id }),
                &format!("added layer {label}"),
            );
            0
        }
        LayerCmd::List => {
            let Some(p) = session.state() else {
                return msg_err(
                    skin,
                    json,
                    "layer.list",
                    "no_document",
                    "no document is open",
                );
            };
            ok(
                skin,
                json,
                "layer.list",
                serde_json::json!({ "layers": p.layers }),
                &format!("{} layers", p.layers.len()),
            );
            0
        }
        LayerCmd::MoveObject { index, layer } => {
            if !check_index(session, skin, json, "layer.move-object", index) {
                return 1;
            }
            session.snapshot(Some("move-object"));
            session
                .state_mut()
                .unwrap()
                .object_mut(index)
                .unwrap()
                .layer = layer;
            ok(
                skin,
                json,
                "layer.move-object",
                session.status(),
                &format!("moved #{index} to layer {layer}"),
            );
            0
        }
    }
}

// ----- gradient ----------------------------------------------------------------

fn parse_stops(specs: &[String]) -> std::result::Result<Vec<GradientStop>, String> {
    specs
        .iter()
        .map(|s| {
            let (off, color) = s
                .split_once(':')
                .ok_or_else(|| format!("bad stop '{s}' (use offset:color)"))?;
            let offset: f64 = off.parse().map_err(|_| format!("bad offset in '{s}'"))?;
            Ok(GradientStop {
                offset,
                color: color.to_string(),
                opacity: 1.0,
            })
        })
        .collect()
}

fn gradient(cmd: GradientCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        GradientCmd::AddLinear {
            x1,
            y1,
            x2,
            y2,
            stops,
        } => {
            if !need_open(session, skin, json, "gradient.add-linear") {
                return 1;
            }
            let stops = match parse_stops(&stops) {
                Ok(s) => s,
                Err(e) => return msg_err(skin, json, "gradient.add-linear", "bad_stop", &e),
            };
            session.snapshot(Some("add-linear"));
            let id = session.state_mut().unwrap().next_gradient_id();
            let gid = session.state_mut().unwrap().add_gradient(Gradient::Linear {
                id,
                x1,
                y1,
                x2,
                y2,
                stops,
            });
            ok(
                skin,
                json,
                "gradient.add-linear",
                serde_json::json!({ "id": gid }),
                &format!("added gradient {gid}"),
            );
            0
        }
        GradientCmd::AddRadial { cx, cy, r, stops } => {
            if !need_open(session, skin, json, "gradient.add-radial") {
                return 1;
            }
            let stops = match parse_stops(&stops) {
                Ok(s) => s,
                Err(e) => return msg_err(skin, json, "gradient.add-radial", "bad_stop", &e),
            };
            session.snapshot(Some("add-radial"));
            let id = session.state_mut().unwrap().next_gradient_id();
            let gid = session.state_mut().unwrap().add_gradient(Gradient::Radial {
                id,
                cx,
                cy,
                r,
                stops,
            });
            ok(
                skin,
                json,
                "gradient.add-radial",
                serde_json::json!({ "id": gid }),
                &format!("added gradient {gid}"),
            );
            0
        }
        GradientCmd::Apply {
            gradient,
            index,
            target,
        } => {
            if !check_index(session, skin, json, "gradient.apply", index) {
                return 1;
            }
            let url = format!("url(#{gradient})");
            mutate_object(session, skin, json, "gradient.apply", index, |o| {
                if target == "stroke" {
                    o.style.stroke = Some(url);
                } else {
                    o.style.fill = Some(url);
                }
            })
        }
        GradientCmd::List => {
            let Some(p) = session.state() else {
                return msg_err(
                    skin,
                    json,
                    "gradient.list",
                    "no_document",
                    "no document is open",
                );
            };
            ok(
                skin,
                json,
                "gradient.list",
                serde_json::json!({ "gradients": p.gradients }),
                &format!("{} gradients", p.gradients.len()),
            );
            0
        }
    }
}

// ----- path (boolean ops recorded; true geometry needs a geometry backend) -----

fn path(cmd: PathCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    match cmd {
        PathCmd::ListOperations => {
            ok(
                skin,
                json,
                "path.list-operations",
                serde_json::json!({ "operations": ["union", "difference", "intersection", "exclusion", "convert"] }),
                "path operations: union, difference, intersection, exclusion, convert",
            );
            0
        }
        PathCmd::Convert { index } => {
            if !check_index(session, skin, json, "path.convert", index) {
                return 1;
            }
            ok(
                skin,
                json,
                "path.convert",
                serde_json::json!({ "index": index, "status": "recorded" }),
                "conversion recorded (geometry backend not wired)",
            );
            0
        }
        PathCmd::Union { a, b } | PathCmd::Difference { a, b } => {
            ok(
                skin,
                json,
                "path.boolean",
                serde_json::json!({ "a": a, "b": b, "status": "recorded" }),
                "boolean op recorded — true 2D geometry needs a geometry backend (out of scope for the proof)",
            );
            0
        }
    }
}

// ----- export ------------------------------------------------------------------

fn export(cmd: ExportCmd, session: &mut Session<Project>, skin: &Skin, json: bool) -> i32 {
    let Some(project) = session.state() else {
        return msg_err(skin, json, "export", "no_document", "no document is open");
    };
    match cmd {
        ExportCmd::Svg { output, overwrite } => {
            match backend::export_svg(project, &output, overwrite) {
                Ok(size) => {
                    ok(
                        skin,
                        json,
                        "export.svg",
                        serde_json::json!({ "output": output.display().to_string(), "format": "svg", "file_size": size }),
                        &format!("wrote {} ({size} bytes)", output.display()),
                    );
                    0
                }
                Err(e) => msg_err(skin, json, "export.svg", "export_error", &e.to_string()),
            }
        }
        ExportCmd::Png {
            output,
            dpi,
            width,
            height,
            overwrite,
        } => {
            match backend::export_via_inkscape(
                project, &output, "png", dpi, width, height, overwrite,
            ) {
                Ok(size) => {
                    ok(
                        skin,
                        json,
                        "export.png",
                        serde_json::json!({ "output": output.display().to_string(), "format": "png", "file_size": size }),
                        &format!("wrote {} ({size} bytes)", output.display()),
                    );
                    0
                }
                Err(e) => msg_err(skin, json, "export.png", "export_error", &e.to_string()),
            }
        }
        ExportCmd::Pdf { output, overwrite } => {
            match backend::export_via_inkscape(project, &output, "pdf", 96, None, None, overwrite) {
                Ok(size) => {
                    ok(
                        skin,
                        json,
                        "export.pdf",
                        serde_json::json!({ "output": output.display().to_string(), "format": "pdf", "file_size": size }),
                        &format!("wrote {} ({size} bytes)", output.display()),
                    );
                    0
                }
                Err(e) => msg_err(skin, json, "export.pdf", "export_error", &e.to_string()),
            }
        }
        ExportCmd::Presets => {
            ok(
                skin,
                json,
                "export.presets",
                serde_json::json!({ "presets": ["svg", "png@96", "png@300", "pdf"] }),
                "presets: svg, png@96, png@300, pdf",
            );
            0
        }
    }
}

// ----- session -----------------------------------------------------------------

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
        SessionCmd::History => {
            let history = session.history().to_vec();
            ok(
                skin,
                json,
                "session.history",
                serde_json::json!({ "history": history }),
                &format!("{} operations", history.len()),
            );
            0
        }
    }
}

// ----- REPL --------------------------------------------------------------------

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
                    (
                        "document",
                        "new/open/save/info/json/canvas-size/units/import",
                    ),
                    ("shape", "add-rect/add-circle/.../remove/list/get"),
                    ("text", "add/list"),
                    ("style", "set-fill/set-stroke/set-opacity/get"),
                    ("transform", "translate/rotate/scale/get/clear"),
                    ("layer", "add/list/move-object"),
                    ("gradient", "add-linear/add-radial/apply/list"),
                    ("export", "svg/png/pdf/presets"),
                    ("session", "status/undo/redo/history"),
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
