//! The clap command tree — the single source of truth for the CLI surface and
//! the generated `SKILL.md` (via the hidden `emit-skill`).

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Agent-native CLI for Inkscape SVG documents.
#[derive(Parser)]
#[command(
    name = "cli-anything-inkscape",
    version,
    about = "Build SVG documents, export via real inkscape, and safely import untrusted SVG."
)]
pub struct Cli {
    /// Emit machine-readable JSON instead of human output.
    #[arg(long, global = true)]
    pub json: bool,
    /// Project file to open before running the command.
    #[arg(long, global = true)]
    pub project: Option<PathBuf>,
    /// Apply changes in memory but do not write them to disk.
    #[arg(long, global = true)]
    pub dry_run: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Top-level command groups.
#[derive(Subcommand)]
pub enum Command {
    /// Document lifecycle, canvas, and safe SVG import.
    #[command(subcommand)]
    Document(DocumentCmd),
    /// Add and manage shapes.
    #[command(subcommand)]
    Shape(ShapeCmd),
    /// Add and manage text.
    #[command(subcommand)]
    Text(TextCmd),
    /// Fill / stroke / opacity.
    #[command(subcommand)]
    Style(StyleCmd),
    /// Translate / rotate / scale objects.
    #[command(subcommand)]
    Transform(TransformCmd),
    /// Layer management.
    #[command(subcommand)]
    Layer(LayerCmd),
    /// Gradients.
    #[command(subcommand)]
    Gradient(GradientCmd),
    /// Path operations.
    #[command(subcommand)]
    Path(PathCmd),
    /// Export to SVG / PNG / PDF.
    #[command(subcommand)]
    Export(ExportCmd),
    /// Undo/redo session control.
    #[command(subcommand)]
    Session(SessionCmd),
    /// Regenerate SKILL.md from the command tree (hidden).
    #[command(hide = true)]
    EmitSkill {
        #[arg(short, long, default_value = "SKILL.md")]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum DocumentCmd {
    /// Create a new document.
    New {
        #[arg(short = 'W', long, default_value_t = 1920.0)]
        width: f64,
        #[arg(short = 'H', long, default_value_t = 1080.0)]
        height: f64,
        #[arg(short, long, default_value = "px")]
        units: String,
        #[arg(short = 'b', long = "background", default_value = "#ffffff")]
        background: String,
        #[arg(short, long, default_value = "untitled")]
        name: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Open an existing `.inkscape-cli.json` document.
    Open { path: PathBuf },
    /// Save the current document.
    Save { path: Option<PathBuf> },
    /// Show document + session status.
    Info,
    /// Print the document model as JSON.
    Json,
    /// Resize the canvas.
    CanvasSize {
        #[arg(short = 'W', long)]
        width: f64,
        #[arg(short = 'H', long)]
        height: f64,
    },
    /// Set the canvas units.
    Units { units: String },
    /// Safely import (validate) an untrusted SVG file — the security showcase.
    Import { path: PathBuf },
}

#[derive(Subcommand)]
pub enum ShapeCmd {
    /// Add a rectangle.
    AddRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        #[arg(long, default_value_t = 0.0)]
        rx: f64,
    },
    /// Add a circle.
    AddCircle { cx: f64, cy: f64, r: f64 },
    /// Add an ellipse.
    AddEllipse { cx: f64, cy: f64, rx: f64, ry: f64 },
    /// Add a line.
    AddLine { x1: f64, y1: f64, x2: f64, y2: f64 },
    /// Add a polygon (SVG points string).
    AddPolygon { points: String },
    /// Add a path (SVG `d`).
    AddPath { d: String },
    /// Add an N-pointed star.
    AddStar {
        cx: f64,
        cy: f64,
        r: f64,
        #[arg(default_value_t = 5)]
        points: u32,
    },
    /// Remove an object by index.
    Remove { index: usize },
    /// Duplicate an object by index.
    Duplicate { index: usize },
    /// List all objects.
    List,
    /// Show one object by index.
    Get { index: usize },
}

#[derive(Subcommand)]
pub enum TextCmd {
    /// Add a text object.
    Add {
        x: f64,
        y: f64,
        content: String,
        #[arg(long, default_value_t = 16.0)]
        font_size: f64,
    },
    /// List text objects.
    List,
}

#[derive(Subcommand)]
pub enum StyleCmd {
    /// Set the fill color of an object.
    SetFill { index: usize, color: String },
    /// Set the stroke color and width.
    SetStroke {
        index: usize,
        color: String,
        #[arg(long, default_value_t = 1.0)]
        width: f64,
    },
    /// Set the opacity (0.0–1.0).
    SetOpacity { index: usize, opacity: f64 },
    /// Show an object's style.
    Get { index: usize },
}

#[derive(Subcommand)]
pub enum TransformCmd {
    /// Translate an object.
    Translate { index: usize, dx: f64, dy: f64 },
    /// Rotate an object (degrees, about its origin or a point).
    Rotate {
        index: usize,
        degrees: f64,
        #[arg(long)]
        cx: Option<f64>,
        #[arg(long)]
        cy: Option<f64>,
    },
    /// Scale an object.
    Scale { index: usize, sx: f64, sy: f64 },
    /// Show an object's transform.
    Get { index: usize },
    /// Clear an object's transform.
    Clear { index: usize },
}

#[derive(Subcommand)]
pub enum LayerCmd {
    /// Add a layer.
    Add { label: String },
    /// List layers.
    List,
    /// Move an object to a layer index.
    MoveObject { index: usize, layer: usize },
}

#[derive(Subcommand)]
pub enum GradientCmd {
    /// Add a linear gradient (stops as `offset:color` pairs).
    AddLinear {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        #[arg(long, value_delimiter = ',', num_args = 1..)]
        stops: Vec<String>,
    },
    /// Add a radial gradient.
    AddRadial {
        cx: f64,
        cy: f64,
        r: f64,
        #[arg(long, value_delimiter = ',', num_args = 1..)]
        stops: Vec<String>,
    },
    /// Apply a gradient id to an object's fill or stroke.
    Apply {
        gradient: String,
        index: usize,
        #[arg(short, long, default_value = "fill")]
        target: String,
    },
    /// List gradients.
    List,
}

#[derive(Subcommand)]
pub enum PathCmd {
    /// List supported path operations.
    ListOperations,
    /// Convert an object to a path (records intent).
    Convert { index: usize },
    /// Boolean union (recorded; true geometry needs a geometry backend).
    Union { a: usize, b: usize },
    /// Boolean difference (recorded).
    Difference { a: usize, b: usize },
}

#[derive(Subcommand)]
pub enum ExportCmd {
    /// Export to SVG (generated locally; no renderer needed).
    Svg {
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
    },
    /// Export to PNG via a real renderer (default inkscape; rsvg also supported).
    Png {
        output: PathBuf,
        #[arg(long, default_value_t = 96)]
        dpi: u32,
        #[arg(short = 'W', long)]
        width: Option<u32>,
        #[arg(short = 'H', long)]
        height: Option<u32>,
        /// Which real renderer to drive.
        #[arg(long, value_enum, default_value_t = Renderer::Inkscape)]
        renderer: Renderer,
        #[arg(long)]
        overwrite: bool,
    },
    /// Export to PDF via a real renderer (default inkscape; rsvg also supported).
    Pdf {
        output: PathBuf,
        /// Which real renderer to drive.
        #[arg(long, value_enum, default_value_t = Renderer::Inkscape)]
        renderer: Renderer,
        #[arg(long)]
        overwrite: bool,
    },
    /// List export presets.
    Presets,
}

/// Which real external renderer drives a raster/vector export.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Renderer {
    /// The real Inkscape binary (full SVG feature fidelity).
    Inkscape,
    /// librsvg's `rsvg-convert` (no Inkscape install required).
    Rsvg,
}

impl Renderer {
    /// Lowercase name (for envelopes / logs).
    pub fn as_str(self) -> &'static str {
        match self {
            Renderer::Inkscape => "inkscape",
            Renderer::Rsvg => "rsvg",
        }
    }
}

#[derive(Subcommand)]
pub enum SessionCmd {
    /// Show undo/redo status.
    Status,
    /// Undo the last change.
    Undo,
    /// Redo the last undone change.
    Redo,
    /// Show the operation history.
    History,
}
