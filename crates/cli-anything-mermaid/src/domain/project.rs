//! Project state for Mermaid: the diagram source plus render configuration.

use serde::{Deserialize, Serialize};

/// A Mermaid project. Saved as `.mermaid.json` (serde JSON, indent 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// The Mermaid diagram source.
    pub code: String,
    /// Mermaid render config (e.g. `{"theme":"default"}`).
    pub mermaid: serde_json::Value,
    /// Auto-update the diagram on edit (mermaid.live parity).
    #[serde(default = "default_true")]
    pub update_diagram: bool,
    /// Hand-drawn "rough" look.
    #[serde(default)]
    pub rough: bool,
    /// Pan/zoom in the live viewer.
    #[serde(default)]
    pub pan_zoom: bool,
    /// Background grid in the live viewer.
    #[serde(default)]
    pub grid: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Project {
    fn default() -> Self {
        Self {
            code: sample("flowchart"),
            mermaid: serde_json::json!({ "theme": "default" }),
            update_diagram: true,
            rough: false,
            pan_zoom: false,
            grid: false,
        }
    }
}

impl Project {
    /// A new project seeded with the flowchart sample and the default theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// A new project from a named sample and theme.
    pub fn with_sample(sample_name: &str, theme: &str) -> Self {
        Self {
            code: sample(sample_name),
            mermaid: serde_json::json!({ "theme": theme }),
            ..Self::default()
        }
    }

    /// The configured theme name (defaults to `"default"`).
    pub fn theme(&self) -> &str {
        self.mermaid
            .get("theme")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("default")
    }
}

/// The built-in sample names.
pub const SAMPLES: &[&str] = &["flowchart", "sequence", "er"];

/// The diagram source for a built-in sample (unknown → flowchart).
pub fn sample(name: &str) -> String {
    match name {
        "sequence" => {
            "sequenceDiagram\n    Alice->>Bob: Hello Bob\n    Bob-->>Alice: Hi Alice".to_string()
        }
        "er" => {
            "erDiagram\n    CUSTOMER ||--o{ ORDER : places\n    ORDER ||--|{ LINE-ITEM : contains"
                .to_string()
        }
        _ => {
            "flowchart TD\n    A[Start] --> B{Decision}\n    B -->|Yes| C[OK]\n    B -->|No| D[End]"
                .to_string()
        }
    }
}
