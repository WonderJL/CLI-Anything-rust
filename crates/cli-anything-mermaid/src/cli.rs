//! The clap command tree — the single source of truth for the CLI surface and
//! the generated `SKILL.md` (via the hidden `emit-skill`).

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Agent-native CLI for Mermaid diagrams.
#[derive(Parser)]
#[command(
    name = "cli-anything-mermaid",
    version,
    about = "Author, render, and verify Mermaid diagrams."
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
    /// Project lifecycle.
    #[command(subcommand)]
    Project(ProjectCmd),

    /// Edit the diagram source.
    #[command(subcommand)]
    Diagram(DiagramCmd),

    /// Render and share the diagram.
    #[command(subcommand)]
    Export(ExportCmd),

    /// Capture immutable preview bundles + a live session for an agent to poll.
    #[command(subcommand)]
    Preview(PreviewCmd),

    /// Undo/redo session control.
    #[command(subcommand)]
    Session(SessionCmd),

    /// Regenerate SKILL.md from the command tree (hidden).
    #[command(hide = true)]
    EmitSkill {
        /// Output path for the generated SKILL.md.
        #[arg(short, long, default_value = "SKILL.md")]
        output: PathBuf,
    },
}

/// Project lifecycle commands.
#[derive(Subcommand)]
pub enum ProjectCmd {
    /// Create a new project from a sample.
    New {
        /// Built-in sample: flowchart | sequence | er.
        #[arg(long, default_value = "flowchart")]
        sample: String,
        /// Mermaid theme (default | dark | forest | neutral).
        #[arg(long, default_value = "default")]
        theme: String,
        /// Save the new project to this path immediately.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Open an existing `.mermaid.json` project.
    Open {
        /// Path to the project JSON.
        path: PathBuf,
    },
    /// Save the current project.
    Save {
        /// Output path (defaults to the open project path).
        path: Option<PathBuf>,
    },
    /// Show project + session status.
    Info,
    /// List the built-in samples.
    Samples,
}

/// Diagram-source commands.
#[derive(Subcommand)]
pub enum DiagramCmd {
    /// Set the diagram source (provide exactly one of --text / --file).
    Set {
        /// Inline diagram source.
        #[arg(long, conflicts_with = "file", required_unless_present = "file")]
        text: Option<String>,
        /// Read the diagram source from a file.
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Print the current diagram source.
    Show,
}

/// Export / share commands.
#[derive(Subcommand)]
pub enum ExportCmd {
    /// Render the diagram to an image (mmdc if present, else HTTP).
    Render {
        /// Output file path.
        output: PathBuf,
        /// Output format.
        #[arg(short, long, value_enum, default_value_t = Format::Svg)]
        format: Format,
        /// Overwrite the output if it exists.
        #[arg(long)]
        overwrite: bool,
    },
    /// Build a mermaid.live share URL (no network).
    Share {
        /// Editor or read-only view.
        #[arg(long, value_enum, default_value_t = ShareMode::Edit)]
        mode: ShareMode,
    },
}

/// Preview-bundle commands: render into an immutable, content-addressed bundle,
/// track a live session head, and append to a replayable trajectory.
#[derive(Subcommand)]
pub enum PreviewCmd {
    /// Render the current diagram into an immutable preview bundle and advance
    /// the live session (reuses a cached bundle when the source is unchanged).
    Capture {
        /// Format to render into the bundle.
        #[arg(short, long, value_enum, default_value_t = Format::Svg)]
        format: Format,
        /// Recipe name — groups bundles and the live session.
        #[arg(long, default_value = "default")]
        recipe: String,
        /// Force a fresh bundle even if an identical one is cached.
        #[arg(long)]
        force: bool,
    },
    /// Print the live preview status — the cheap agent poll (`--json`).
    Status {
        /// Recipe name to inspect.
        #[arg(long, default_value = "default")]
        recipe: String,
        /// How many recent trajectory steps to include.
        #[arg(long, default_value_t = 5)]
        recent: usize,
    },
    /// List the preview bundles for a recipe (newest first).
    List {
        /// Recipe name.
        #[arg(long, default_value = "default")]
        recipe: String,
    },
}

/// Undo/redo session commands.
#[derive(Subcommand)]
pub enum SessionCmd {
    /// Show undo/redo status.
    Status,
    /// Undo the last change.
    Undo,
    /// Redo the last undone change.
    Redo,
}

/// Render output format.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Format {
    /// Scalable vector graphics.
    Svg,
    /// Portable network graphics.
    Png,
}

impl Format {
    /// Lowercase name.
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Svg => "svg",
            Format::Png => "png",
        }
    }
}

/// Share-URL mode.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ShareMode {
    /// Open the editor.
    Edit,
    /// Read-only view.
    View,
}

impl ShareMode {
    /// URL fragment segment.
    pub fn as_str(self) -> &'static str {
        match self {
            ShareMode::Edit => "edit",
            ShareMode::View => "view",
        }
    }
}
