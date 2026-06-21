//! `cli-anything-new` — the deterministic scaffolder.
//!
//! Stamps a compilable, agent-native CLI crate (`cli-anything-<software>`) that
//! depends on `cli-anything-core`. The agent then fills in only the domain
//! logic; the boilerplate (clap surface, reedline REPL, `--json` envelope,
//! auto-save, SKILL.md emitter) is generated here, identically, every time.

mod manifest;
mod render;

use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Parser;

use crate::render::Scaffold;

#[derive(Parser)]
#[command(
    name = "cli-anything-new",
    version,
    about = "Scaffold a new agent-native CLI crate"
)]
struct Args {
    /// Software name (e.g. `mermaid`); becomes crate `cli-anything-<name>`.
    #[arg(long)]
    software: String,

    /// REPL accent name (informational; core derives the accent from the name).
    #[arg(long)]
    accent: Option<String>,

    /// Workspace root to scaffold into.
    #[arg(long, default_value = ".")]
    out: PathBuf,

    /// One-line description (defaults to "Agent-native CLI for <Name>.").
    #[arg(long)]
    description: Option<String>,

    /// Omit the `preview` command group.
    #[arg(long)]
    no_preview: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let slug = slugify(&args.software);
    if slug.is_empty() {
        bail!("--software must contain alphanumeric characters");
    }
    let crate_name = format!("cli-anything-{slug}");
    let crate_ident = crate_name.replace('-', "_");
    let display = title_case(&slug);
    let description = args
        .description
        .unwrap_or_else(|| format!("Agent-native CLI for {display}."));

    let scaffold = Scaffold {
        software: slug,
        software_display: display,
        crate_name: crate_name.clone(),
        crate_ident,
        description,
        accent: args.accent.unwrap_or_else(|| "auto".to_string()),
        with_preview: !args.no_preview,
    };

    let crate_rel = format!("crates/{crate_name}");
    let crate_dir = args.out.join(&crate_rel);
    if crate_dir.exists() {
        bail!(
            "{} already exists — refusing to overwrite",
            crate_dir.display()
        );
    }

    let files = render::render(&scaffold)?;
    for (rel, content) in &files {
        let dest = crate_dir.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, content).with_context(|| format!("writing {}", dest.display()))?;
    }

    let registered = manifest::register_member(&args.out, &crate_rel)?;

    println!(
        "✓ scaffolded {crate_name} ({} files) at {}",
        files.len(),
        crate_dir.display()
    );
    println!(
        "  workspace member: {}",
        if registered {
            "registered"
        } else {
            "already present / no workspace manifest"
        }
    );
    println!("  build:  cargo build -p {crate_name}");
    println!("  skill:  cargo run -p {crate_name} -- emit-skill -o {crate_rel}/SKILL.md");
    Ok(())
}

/// Lowercase, collapse non-alphanumeric runs to single hyphens, trim hyphens.
fn slugify(s: &str) -> String {
    let lower = s.trim().to_ascii_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut prev_dash = false;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Title-case the first character of a slug for display.
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
