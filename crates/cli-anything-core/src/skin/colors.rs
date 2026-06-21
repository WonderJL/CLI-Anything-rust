//! Accent + status color map (port of `repl_skin.py`'s color logic).
//!
//! Colors are `nu_ansi_term::Color` values (the same crate `reedline` uses, so
//! they interoperate with the REPL prompt). Per-software accents mirror the
//! Python harness; everything else uses a consistent status palette.

use nu_ansi_term::Color;

/// Default accent when a software has no specific color (sky blue).
pub const DEFAULT_ACCENT: Color = Color::Fixed(39);
/// The cli-anything brand color (cyan).
pub const BRAND: Color = Color::Fixed(44);

/// Success messages (green).
pub const SUCCESS: Color = Color::Fixed(42);
/// Warnings (amber).
pub const WARNING: Color = Color::Fixed(214);
/// Errors (red).
pub const ERROR: Color = Color::Fixed(196);
/// Informational messages (blue).
pub const INFO: Color = Color::Fixed(39);
/// Hints / secondary text (gray).
pub const HINT: Color = Color::Fixed(244);

/// The accent color for a given software name (case-insensitive).
pub fn accent(software: &str) -> Color {
    match software.to_ascii_lowercase().as_str() {
        "gimp" => Color::Fixed(208),              // orange
        "blender" => Color::Fixed(214),           // deep orange
        "inkscape" => Color::Fixed(33),           // blue
        "audacity" => Color::Fixed(63),           // indigo
        "libreoffice" => Color::Fixed(28),        // green
        "obs" | "obs-studio" => Color::Fixed(99), // violet
        "kdenlive" => Color::Fixed(35),           // teal
        "shotcut" => Color::Fixed(38),            // cyan-blue
        "mermaid" => Color::Fixed(212),           // pink
        _ => DEFAULT_ACCENT,
    }
}
