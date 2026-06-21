//! The Inkscape document model: canvas, objects, layers, gradients.
//!
//! A plain serde-serializable tree (saved as `.inkscape-cli.json`). SVG is
//! generated from this model by [`crate::domain::svg`]; the real `inkscape`
//! binary rasterizes that SVG to PNG/PDF.

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}
fn one() -> f64 {
    1.0
}

/// Visual style applied to an object (serialized to a CSS `style` attribute).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Style {
    /// Fill color (e.g. `#3b82f6`, `none`, or `url(#grad1)`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// Stroke color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    /// Stroke width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<f64>,
    /// Opacity 0.0–1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
}

impl Style {
    /// Render to a CSS declaration string for the SVG `style` attribute.
    pub fn to_css(&self) -> String {
        let mut parts = Vec::new();
        if let Some(f) = &self.fill {
            parts.push(format!("fill:{f}"));
        }
        if let Some(s) = &self.stroke {
            parts.push(format!("stroke:{s}"));
        }
        if let Some(w) = self.stroke_width {
            parts.push(format!("stroke-width:{w}"));
        }
        if let Some(o) = self.opacity {
            parts.push(format!("opacity:{o}"));
        }
        parts.join(";")
    }
}

/// A drawable shape. Tagged by `type` in JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Shape {
    /// Rectangle.
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        #[serde(default)]
        rx: f64,
    },
    /// Circle.
    Circle { cx: f64, cy: f64, r: f64 },
    /// Ellipse.
    Ellipse { cx: f64, cy: f64, rx: f64, ry: f64 },
    /// Line segment.
    Line { x1: f64, y1: f64, x2: f64, y2: f64 },
    /// Polygon (SVG points string).
    Polygon { points: String },
    /// Arbitrary path (SVG `d`).
    Path { d: String },
    /// N-pointed star (rendered as a computed polygon).
    Star {
        cx: f64,
        cy: f64,
        r: f64,
        points: u32,
    },
    /// Text.
    Text {
        x: f64,
        y: f64,
        content: String,
        #[serde(default = "default_font_size")]
        font_size: f64,
    },
}

fn default_font_size() -> f64 {
    16.0
}

impl Shape {
    /// A short kind label.
    pub fn kind(&self) -> &'static str {
        match self {
            Shape::Rect { .. } => "rect",
            Shape::Circle { .. } => "circle",
            Shape::Ellipse { .. } => "ellipse",
            Shape::Line { .. } => "line",
            Shape::Polygon { .. } => "polygon",
            Shape::Path { .. } => "path",
            Shape::Star { .. } => "star",
            Shape::Text { .. } => "text",
        }
    }
}

/// An object: a shape plus id, style, transform, and layer index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    /// Unique id (e.g. `obj1`).
    pub id: String,
    /// The shape geometry.
    pub shape: Shape,
    /// Visual style.
    #[serde(default)]
    pub style: Style,
    /// SVG transform string (e.g. `rotate(45 10 10)`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<String>,
    /// Index into `layers` (0 = first/default layer).
    #[serde(default)]
    pub layer: usize,
}

/// A layer (an SVG group with Inkscape layer metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    /// Layer id.
    pub id: String,
    /// Human label.
    pub label: String,
    /// Whether the layer is visible.
    #[serde(default = "default_true")]
    pub visible: bool,
}

/// A gradient stop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientStop {
    /// Offset 0.0–1.0.
    pub offset: f64,
    /// Stop color.
    pub color: String,
    /// Stop opacity.
    #[serde(default = "one")]
    pub opacity: f64,
}

/// A gradient definition. Tagged by `type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Gradient {
    /// Linear gradient.
    Linear {
        id: String,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stops: Vec<GradientStop>,
    },
    /// Radial gradient.
    Radial {
        id: String,
        cx: f64,
        cy: f64,
        r: f64,
        stops: Vec<GradientStop>,
    },
}

impl Gradient {
    /// The gradient id.
    pub fn id(&self) -> &str {
        match self {
            Gradient::Linear { id, .. } | Gradient::Radial { id, .. } => id,
        }
    }
}

/// The canvas (document) settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    /// Width in `units`.
    pub width: f64,
    /// Height in `units`.
    pub height: f64,
    /// Units: px | mm | cm | in | pt | pc.
    pub units: String,
    /// Background color.
    pub background: String,
}

/// The full Inkscape project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Schema version.
    pub version: u32,
    /// Document name.
    pub name: String,
    /// Canvas settings.
    pub canvas: Canvas,
    /// Objects in draw order.
    #[serde(default)]
    pub objects: Vec<Object>,
    /// Layers (index 0 = default).
    #[serde(default)]
    pub layers: Vec<Layer>,
    /// Gradient definitions.
    #[serde(default)]
    pub gradients: Vec<Gradient>,
    /// Monotonic id counter.
    #[serde(default)]
    pub next_id: u32,
}

impl Default for Project {
    fn default() -> Self {
        Self::with_canvas(1920.0, 1080.0, "px", "#ffffff", "untitled")
    }
}

impl Project {
    /// Create a new document.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new document with explicit canvas settings.
    pub fn with_canvas(width: f64, height: f64, units: &str, background: &str, name: &str) -> Self {
        Self {
            version: 1,
            name: name.to_string(),
            canvas: Canvas {
                width,
                height,
                units: units.to_string(),
                background: background.to_string(),
            },
            objects: Vec::new(),
            layers: vec![Layer {
                id: "layer1".to_string(),
                label: "Layer 1".to_string(),
                visible: true,
            }],
            gradients: Vec::new(),
            next_id: 1,
        }
    }

    fn mint_id(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}{}", self.next_id);
        self.next_id += 1;
        id
    }

    /// Add a shape, returning its new object id.
    pub fn add_shape(&mut self, shape: Shape) -> String {
        let id = self.mint_id("obj");
        self.objects.push(Object {
            id: id.clone(),
            shape,
            style: Style::default(),
            transform: None,
            layer: 0,
        });
        id
    }

    /// Borrow an object by index.
    pub fn object(&self, index: usize) -> Option<&Object> {
        self.objects.get(index)
    }

    /// Mutably borrow an object by index.
    pub fn object_mut(&mut self, index: usize) -> Option<&mut Object> {
        self.objects.get_mut(index)
    }

    /// Remove an object by index, returning it.
    pub fn remove(&mut self, index: usize) -> Option<Object> {
        if index < self.objects.len() {
            Some(self.objects.remove(index))
        } else {
            None
        }
    }

    /// Duplicate an object by index, returning the new id.
    pub fn duplicate(&mut self, index: usize) -> Option<String> {
        let mut copy = self.objects.get(index)?.clone();
        let id = self.mint_id("obj");
        copy.id = id.clone();
        self.objects.push(copy);
        Some(id)
    }

    /// Add a layer, returning its id.
    pub fn add_layer(&mut self, label: &str) -> String {
        let id = self.mint_id("layer");
        self.layers.push(Layer {
            id: id.clone(),
            label: label.to_string(),
            visible: true,
        });
        id
    }

    /// Add a gradient.
    pub fn add_gradient(&mut self, gradient: Gradient) -> String {
        let id = gradient.id().to_string();
        self.gradients.push(gradient);
        id
    }

    /// Allocate a gradient id (e.g. `grad1`).
    pub fn next_gradient_id(&mut self) -> String {
        self.mint_id("grad")
    }
}
