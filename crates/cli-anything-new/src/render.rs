//! Template rendering: embedded `templates/*.j2` stamped via minijinja.

use anyhow::{Context, Result};
use minijinja::{context, Environment};

/// `(relative output path, embedded template source)`. Templates are baked into
/// the binary so the installed scaffolder is fully self-contained.
const TEMPLATES: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../templates/Cargo.toml.j2")),
    ("src/lib.rs", include_str!("../templates/lib.rs.j2")),
    ("src/main.rs", include_str!("../templates/main.rs.j2")),
    ("src/cli.rs", include_str!("../templates/cli.rs.j2")),
    (
        "src/domain/mod.rs",
        include_str!("../templates/domain_mod.rs.j2"),
    ),
    (
        "src/domain/project.rs",
        include_str!("../templates/domain_project.rs.j2"),
    ),
    ("src/backend.rs", include_str!("../templates/backend.rs.j2")),
    (
        "src/repl_cmds.rs",
        include_str!("../templates/repl_cmds.rs.j2"),
    ),
    (
        "tests/unit.rs",
        include_str!("../templates/tests_unit.rs.j2"),
    ),
    ("tests/e2e.rs", include_str!("../templates/tests_e2e.rs.j2")),
    ("README.md", include_str!("../templates/README.md.j2")),
];

/// The variables fed to every template.
pub struct Scaffold {
    /// Slug, e.g. `mermaid`.
    pub software: String,
    /// Display name, e.g. `Mermaid`.
    pub software_display: String,
    /// Package name, e.g. `cli-anything-mermaid`.
    pub crate_name: String,
    /// Rust crate identifier, e.g. `cli_anything_mermaid`.
    pub crate_ident: String,
    /// One-line description.
    pub description: String,
    /// Accent name (informational).
    pub accent: String,
    /// Whether to stamp the `preview` command group.
    pub with_preview: bool,
}

/// Render every template, returning `(relative path, rendered content)`.
pub fn render(s: &Scaffold) -> Result<Vec<(String, String)>> {
    let mut env = Environment::new();
    for (name, src) in TEMPLATES {
        env.add_template(name, src)
            .with_context(|| format!("compiling template {name}"))?;
    }
    let ctx = context! {
        software => s.software.clone(),
        software_display => s.software_display.clone(),
        crate_name => s.crate_name.clone(),
        crate_ident => s.crate_ident.clone(),
        description => s.description.clone(),
        accent => s.accent.clone(),
        with_preview => s.with_preview,
    };

    let mut out = Vec::with_capacity(TEMPLATES.len());
    for (name, _) in TEMPLATES {
        let tmpl = env
            .get_template(name)
            .expect("template was just added above");
        let rendered = tmpl
            .render(&ctx)
            .with_context(|| format!("rendering template {name}"))?;
        out.push(((*name).to_string(), rendered));
    }
    Ok(out)
}
