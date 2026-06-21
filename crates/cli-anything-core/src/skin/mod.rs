//! reedline-based REPL skin (port of `repl_skin.py`).
//!
//! A [`Skin`] provides the branded banner, message helpers, and table/progress
//! rendering shared by every generated CLI. Human/REPL output only — the
//! `--json` path bypasses the skin entirely.
//!
//! Color is enabled only when stdout is a TTY and `NO_COLOR` is unset, so piped
//! or agent-captured output stays clean.

pub mod colors;
pub mod repl;

use std::io::IsTerminal;
use std::path::PathBuf;

use nu_ansi_term::Color;

/// Branded terminal renderer for a single CLI.
#[derive(Debug, Clone)]
pub struct Skin {
    software: String,
    version: String,
    accent: Color,
    skill_path: Option<PathBuf>,
    color: bool,
}

impl Skin {
    /// Create a skin for `software` at `version`, choosing the accent color.
    pub fn new(software: impl Into<String>, version: impl Into<String>) -> Self {
        let software = software.into();
        let accent = colors::accent(&software);
        Self {
            software,
            version: version.into(),
            accent,
            skill_path: None,
            color: color_enabled(),
        }
    }

    /// Attach the absolute `SKILL.md` path shown in the banner (for agents).
    pub fn with_skill_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.skill_path = Some(path.into());
        self
    }

    /// Force color on/off (overrides TTY detection; useful for tests/demos).
    pub fn with_color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }

    /// The software name.
    pub fn software(&self) -> &str {
        &self.software
    }

    /// The accent color.
    pub fn accent(&self) -> Color {
        self.accent
    }

    fn paint(&self, color: Color, text: &str) -> String {
        if self.color {
            color.paint(text).to_string()
        } else {
            text.to_string()
        }
    }

    /// Render the startup banner (box-drawn). Returns the string; see
    /// [`Skin::print_banner`] to emit it.
    pub fn banner(&self) -> String {
        let title = format!(
            "◆ cli-anything · {} v{}",
            title_case(&self.software),
            self.version
        );
        let mut lines = vec![title];
        if let Some(skill) = &self.skill_path {
            lines.push(format!("skill: {}", skill.display()));
        }
        lines.push("tip: type 'help' for commands, 'quit' to exit".to_string());

        let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let mut out = String::new();
        out.push_str(&format!("╭─{}─╮\n", "─".repeat(width)));
        for line in &lines {
            let pad = width - line.chars().count();
            out.push_str(&format!("│ {}{} │\n", line, " ".repeat(pad)));
        }
        out.push_str(&format!("╰─{}─╯", "─".repeat(width)));
        self.paint(self.accent, &out)
    }

    /// Print the banner to stdout.
    pub fn print_banner(&self) {
        println!("{}", self.banner());
    }

    fn line(&self, symbol: &str, color: Color, msg: &str) -> String {
        format!("{} {}", self.paint(color, symbol), msg)
    }

    /// `✓` success line (stdout).
    pub fn success(&self, msg: &str) {
        println!("{}", self.line("✓", colors::SUCCESS, msg));
    }

    /// `✗` error line (stderr — agents read errors there).
    pub fn error(&self, msg: &str) {
        eprintln!("{}", self.line("✗", colors::ERROR, msg));
    }

    /// `⚠` warning line (stderr).
    pub fn warning(&self, msg: &str) {
        eprintln!("{}", self.line("⚠", colors::WARNING, msg));
    }

    /// `●` info line (stdout).
    pub fn info(&self, msg: &str) {
        println!("{}", self.line("●", colors::INFO, msg));
    }

    /// Dimmed hint line (stdout).
    pub fn hint(&self, msg: &str) {
        println!("{}", self.paint(colors::HINT, &format!("  {msg}")));
    }

    /// A section header (stdout).
    pub fn section(&self, title: &str) {
        println!("\n{}", self.paint(self.accent, &format!("▸ {title}")));
    }

    /// A single `label: value` status line (stdout).
    pub fn status(&self, label: &str, value: &str) {
        println!(
            "{} {}",
            self.paint(colors::HINT, &format!("{label}:")),
            value
        );
    }

    /// A titled block of `label: value` rows (stdout).
    pub fn status_block(&self, title: &str, items: &[(&str, &str)]) {
        self.section(title);
        for (label, value) in items {
            self.status(label, value);
        }
    }

    /// Render a `[####----] 50% (5/10) label` progress string.
    pub fn progress(&self, current: usize, total: usize, label: &str) -> String {
        let total = total.max(1);
        let frac = (current as f64 / total as f64).clamp(0.0, 1.0);
        let width = 20usize;
        let filled = (frac * width as f64).round() as usize;
        let bar = format!("{}{}", "#".repeat(filled), "-".repeat(width - filled));
        let pct = (frac * 100.0).round() as u32;
        format!(
            "{} {pct}% ({current}/{total}) {label}",
            self.paint(self.accent, &format!("[{bar}]"))
        )
    }

    /// Render a box-drawn table. Returns the string.
    pub fn table(&self, headers: &[&str], rows: &[Vec<String>]) -> String {
        let cols = headers.len();
        let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
        for row in rows {
            for (i, cell) in row.iter().enumerate().take(cols) {
                widths[i] = widths[i].max(cell.chars().count());
            }
        }
        let sep = |l: &str, m: &str, r: &str| {
            let mut s = String::from(l);
            for (i, w) in widths.iter().enumerate() {
                s.push_str(&"─".repeat(w + 2));
                s.push_str(if i + 1 == cols { r } else { m });
            }
            s
        };
        let render_row = |cells: &[String]| {
            let mut s = String::from("│");
            for (i, w) in widths.iter().enumerate() {
                let cell = cells.get(i).map(String::as_str).unwrap_or("");
                let pad = w - cell.chars().count();
                s.push_str(&format!(" {}{} │", cell, " ".repeat(pad)));
            }
            s
        };
        let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
        let mut out = String::new();
        out.push_str(&sep("┌", "┬", "┐"));
        out.push('\n');
        out.push_str(&self.paint(self.accent, &render_row(&header_cells)));
        out.push('\n');
        out.push_str(&sep("├", "┼", "┤"));
        out.push('\n');
        for row in rows {
            out.push_str(&render_row(row));
            out.push('\n');
        }
        out.push_str(&sep("└", "┴", "┘"));
        out
    }

    /// Print a command reference list (stdout).
    pub fn help(&self, commands: &[(&str, &str)]) {
        self.section("commands");
        let width = commands
            .iter()
            .map(|(c, _)| c.chars().count())
            .max()
            .unwrap_or(0);
        for (cmd, desc) in commands {
            let pad = width - cmd.chars().count();
            println!(
                "  {}{}  {}",
                self.paint(self.accent, cmd),
                " ".repeat(pad),
                desc
            );
        }
    }

    /// Print the styled exit message.
    pub fn print_goodbye(&self) {
        println!("{}", self.paint(colors::HINT, "bye — artifacts saved."));
    }
}

fn color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_software_and_version() {
        let skin = Skin::new("mermaid", "0.1.0")
            .with_color(false)
            .with_skill_path("/tmp/SKILL.md");
        let b = skin.banner();
        assert!(b.contains("Mermaid"));
        assert!(b.contains("0.1.0"));
        assert!(b.contains("/tmp/SKILL.md"));
        assert!(b.contains("╭"));
    }

    #[test]
    fn table_renders_headers_and_rows() {
        let skin = Skin::new("inkscape", "0.1.0").with_color(false);
        let t = skin.table(
            &["name", "kind"],
            &[
                vec!["rect1".into(), "rect".into()],
                vec!["c1".into(), "circle".into()],
            ],
        );
        assert!(t.contains("name"));
        assert!(t.contains("rect1"));
        assert!(t.contains("circle"));
        assert!(t.contains('┌') && t.contains('┘'));
    }

    #[test]
    fn progress_bar_reports_percentage() {
        let skin = Skin::new("mermaid", "0.1.0").with_color(false);
        let p = skin.progress(5, 10, "rendering");
        assert!(p.contains("50%"));
        assert!(p.contains("(5/10)"));
        assert!(p.contains("rendering"));
    }

    #[test]
    fn accent_is_software_specific() {
        assert_eq!(
            Skin::new("inkscape", "1").accent(),
            colors::accent("inkscape")
        );
        assert_eq!(
            Skin::new("unknown-xyz", "1").accent(),
            colors::DEFAULT_ACCENT
        );
    }
}
