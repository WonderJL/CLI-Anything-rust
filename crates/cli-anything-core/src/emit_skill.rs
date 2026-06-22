//! Generate `SKILL.md` by walking a clap `Command` tree.
//!
//! This replaces the Python `skill_generator.py`'s regex-over-decorators
//! approach with runtime introspection of the real `clap::Command`. Because the
//! parser is the single source of truth, the generated `SKILL.md` can never
//! drift from the actual command surface. Each generated CLI exposes a hidden
//! `emit-skill` subcommand that calls this.

use clap::{Arg, Command};

/// Metadata that can't be derived from the clap tree alone.
#[derive(Debug, Clone)]
pub struct SkillMeta {
    /// Software name, e.g. `mermaid` (skill becomes `cli-anything-mermaid`).
    pub software: String,
    /// Crate version.
    pub version: String,
    /// One-line description for the YAML frontmatter.
    pub description: String,
    /// Optional install command for the backend tool.
    pub install_hint: Option<String>,
    /// Backend prerequisites (e.g. `inkscape`, `mmdc`).
    pub prereqs: Vec<String>,
}

/// Render a complete `SKILL.md` for `cmd` (the CLI root) using `meta`.
pub fn emit_skill(cmd: &Command, meta: &SkillMeta) -> String {
    let skill_name = format!("cli-anything-{}", meta.software);
    let mut out = String::new();

    // YAML frontmatter.
    out.push_str("---\n");
    out.push_str(&format!("name: {skill_name}\n"));
    out.push_str(&format!("description: {}\n", meta.description));
    out.push_str("---\n\n");

    out.push_str(&format!("# {skill_name}\n\n{}\n\n", meta.description));

    // Installation.
    out.push_str("## Installation\n\n");
    out.push_str(&format!(
        "```bash\ncargo install --path crates/{skill_name}\n```\n\n"
    ));
    if !meta.prereqs.is_empty() {
        out.push_str(&format!(
            "Requires the real backend: {}.",
            meta.prereqs.join(", ")
        ));
        if let Some(hint) = &meta.install_hint {
            out.push_str(&format!(" Install with `{hint}`."));
        }
        out.push_str("\n\n");
    }

    // Conventions.
    out.push_str("## Conventions\n\n");
    out.push_str(
        "- Every command accepts `--json`. JSON output is a uniform envelope: \
         `{ok, action, data, error, warnings}` (`ok=false` plus `error.kind`/`error.message`/`error.hint` on failure).\n\
         - Run with no subcommand to enter an interactive REPL.\n\
         - Global flags: `--project <path>`, `--dry-run`.\n\n",
    );

    // Command groups / leaves.
    let mut groups = String::new();
    let mut leaves: Vec<&Command> = Vec::new();
    let mut has_preview = false;

    for sub in cmd.get_subcommands() {
        if sub.is_hide_set() {
            continue; // e.g. the hidden `emit-skill`
        }
        if sub.get_name() == "preview" {
            has_preview = true;
        }
        if sub.get_subcommands().next().is_some() {
            groups.push_str(&render_group(sub));
        } else {
            leaves.push(sub);
        }
    }

    out.push_str("## Commands\n\n");
    if !leaves.is_empty() {
        out.push_str("### top-level\n\n");
        for cmd in &leaves {
            out.push_str(&render_command_line(cmd.get_name(), cmd));
        }
        out.push('\n');
    }
    out.push_str(&groups);

    // Agent guidance.
    out.push_str("## Agent guidance\n\n");
    out.push_str(
        "- Always pass `--json` for machine-readable output.\n\
         - Check the exit code: `0` success, `1` error.\n\
         - On error, read `error.kind` (stable) and `error.hint`.\n\
         - Use absolute paths for `--project` and outputs.\n\
         - Verify produced files (e.g. magic bytes) rather than trusting exit code alone.\n\n",
    );

    if has_preview {
        out.push_str("## Preview\n\n");
        out.push_str(
            "- `preview capture` renders an honest, content-addressed bundle from the real \
             backend and advances a live session (identical source reuses the cached bundle).\n\
             - Poll cheaply with `preview status --json` — a compact `trajectory_summary` plus the \
             current bundle, without reading every step.\n\
             - Enumerate a recipe's bundles (newest first) with `preview list --json`.\n\n",
        );
    }

    out
}

fn render_group(group: &Command) -> String {
    let mut s = format!("### {}\n\n", group.get_name());
    if let Some(about) = group.get_about() {
        s.push_str(&format!("{about}\n\n"));
    }
    for sub in group.get_subcommands() {
        if sub.is_hide_set() {
            continue;
        }
        s.push_str(&render_command_line(
            &format!("{} {}", group.get_name(), sub.get_name()),
            sub,
        ));
    }
    s.push('\n');
    s
}

fn render_command_line(full_name: &str, cmd: &Command) -> String {
    let about = cmd.get_about().map(|a| a.to_string()).unwrap_or_default();
    let mut s = format!("- `{full_name}` — {about}\n");
    for arg in cmd.get_arguments() {
        if matches!(arg.get_id().as_str(), "help" | "version") {
            continue;
        }
        let required = if arg.is_required_set() {
            " (required)"
        } else {
            ""
        };
        let help = arg.get_help().map(|h| h.to_string()).unwrap_or_default();
        let sep = if help.is_empty() { "" } else { " — " };
        s.push_str(&format!("  - `{}`{required}{sep}{help}\n", arg_label(arg)));
    }
    s
}

fn arg_label(arg: &Arg) -> String {
    let takes_value = arg.get_action().takes_values();
    let value = arg
        .get_value_names()
        .and_then(|v| v.first())
        .map(|s| s.to_string())
        .unwrap_or_else(|| arg.get_id().as_str().to_ascii_uppercase());

    if let Some(long) = arg.get_long() {
        if takes_value {
            format!("--{long} <{value}>")
        } else {
            format!("--{long}")
        }
    } else if let Some(short) = arg.get_short() {
        if takes_value {
            format!("-{short} <{value}>")
        } else {
            format!("-{short}")
        }
    } else {
        format!("<{value}>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ArgAction;

    fn synthetic() -> Command {
        Command::new("cli-anything-mermaid")
            .subcommand(
                Command::new("project")
                    .about("Project lifecycle")
                    .subcommand(
                        Command::new("new").about("Create a project").arg(
                            Arg::new("sample")
                                .long("sample")
                                .help("starter sample")
                                .action(ArgAction::Set),
                        ),
                    )
                    .subcommand(Command::new("save").about("Save the project")),
            )
            .subcommand(
                Command::new("preview")
                    .about("Render preview bundles")
                    .subcommand(Command::new("capture").about("Render a bundle"))
                    .subcommand(Command::new("status").about("Live preview status"))
                    .subcommand(Command::new("list").about("List bundles")),
            )
            .subcommand(Command::new("emit-skill").hide(true))
    }

    fn meta() -> SkillMeta {
        SkillMeta {
            software: "mermaid".into(),
            version: "0.1.0".into(),
            description: "Agent-native CLI for Mermaid diagrams.".into(),
            install_hint: Some("npm i -g @mermaid-js/mermaid-cli".into()),
            prereqs: vec!["mmdc".into()],
        }
    }

    #[test]
    fn emits_frontmatter_and_groups() {
        let md = emit_skill(&synthetic(), &meta());
        assert!(md.starts_with("---\n"));
        assert!(md.contains("name: cli-anything-mermaid"));
        assert!(md.contains("description: Agent-native CLI for Mermaid diagrams."));
        assert!(md.contains("### project"));
        assert!(md.contains("`project new`"));
        assert!(md.contains("Create a project"));
        assert!(md.contains("`--sample <SAMPLE>`"));
    }

    #[test]
    fn hidden_commands_are_omitted() {
        let md = emit_skill(&synthetic(), &meta());
        assert!(!md.contains("emit-skill"));
    }

    #[test]
    fn preview_section_present_when_group_exists() {
        let md = emit_skill(&synthetic(), &meta());
        assert!(md.contains("## Preview"));
        // The guidance must name only commands that actually exist.
        assert!(md.contains("preview status --json"));
        assert!(md.contains("preview list --json"));
        assert!(md.contains("preview capture"));
        assert!(!md.contains("preview live status"));
        assert!(!md.contains("preview latest"));
        assert!(!md.contains("preview recipes"));
    }

    #[test]
    fn prereqs_and_install_hint_render() {
        let md = emit_skill(&synthetic(), &meta());
        assert!(md.contains("mmdc"));
        assert!(md.contains("npm i -g @mermaid-js/mermaid-cli"));
    }

    #[test]
    fn no_preview_section_without_preview_group() {
        let cmd = Command::new("cli-anything-x")
            .subcommand(Command::new("project").subcommand(Command::new("new")));
        let md = emit_skill(&cmd, &meta());
        assert!(!md.contains("## Preview"));
    }
}
