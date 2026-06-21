//! SVG generation from the document model, via the `quick-xml` writer.
//!
//! No string concatenation: every element/attribute goes through quick-xml, so
//! values are properly escaped (untrusted text in a `text` object cannot break
//! out of the markup).

use anyhow::Result;
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::writer::Writer;

use super::project::{Gradient, Object, Project, Shape};

type W = Writer<Vec<u8>>;

/// Render the project to an SVG document string.
pub fn to_svg(project: &Project) -> Result<String> {
    let mut w: W = Writer::new(Vec::new());
    let c = &project.canvas;

    let mut svg = BytesStart::new("svg");
    svg.push_attribute(("xmlns", "http://www.w3.org/2000/svg"));
    svg.push_attribute((
        "xmlns:inkscape",
        "http://www.inkscape.org/namespaces/inkscape",
    ));
    svg.push_attribute((
        "xmlns:sodipodi",
        "http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd",
    ));
    svg.push_attribute(("xmlns:xlink", "http://www.w3.org/1999/xlink"));
    svg.push_attribute(("width", format!("{}{}", c.width, c.units).as_str()));
    svg.push_attribute(("height", format!("{}{}", c.height, c.units).as_str()));
    svg.push_attribute(("viewBox", format!("0 0 {} {}", c.width, c.height).as_str()));
    w.write_event(Event::Start(svg))?;

    // Definitions (gradients).
    if !project.gradients.is_empty() {
        w.write_event(Event::Start(BytesStart::new("defs")))?;
        for g in &project.gradients {
            write_gradient(&mut w, g)?;
        }
        w.write_event(Event::End(quick_xml::events::BytesEnd::new("defs")))?;
    }

    // Background.
    if !c.background.is_empty() && c.background != "none" {
        let mut bg = BytesStart::new("rect");
        bg.push_attribute(("x", "0"));
        bg.push_attribute(("y", "0"));
        bg.push_attribute(("width", c.width.to_string().as_str()));
        bg.push_attribute(("height", c.height.to_string().as_str()));
        bg.push_attribute(("fill", c.background.as_str()));
        w.write_event(Event::Empty(bg))?;
    }

    // One group per layer; objects whose `layer` index matches go inside.
    for (li, layer) in project.layers.iter().enumerate() {
        let mut g = BytesStart::new("g");
        g.push_attribute(("id", layer.id.as_str()));
        g.push_attribute(("inkscape:groupmode", "layer"));
        g.push_attribute(("inkscape:label", layer.label.as_str()));
        if !layer.visible {
            g.push_attribute(("style", "display:none"));
        }
        w.write_event(Event::Start(g))?;
        for obj in project.objects.iter().filter(|o| o.layer == li) {
            write_object(&mut w, obj)?;
        }
        w.write_event(Event::End(quick_xml::events::BytesEnd::new("g")))?;
    }

    // Objects on a non-existent layer index → emit at the root so nothing is lost.
    let layer_count = project.layers.len();
    for obj in project.objects.iter().filter(|o| o.layer >= layer_count) {
        write_object(&mut w, obj)?;
    }

    w.write_event(Event::End(quick_xml::events::BytesEnd::new("svg")))?;
    Ok(String::from_utf8(w.into_inner())?)
}

fn common_attrs(e: &mut BytesStart, obj: &Object) {
    e.push_attribute(("id", obj.id.as_str()));
    let css = obj.style.to_css();
    if !css.is_empty() {
        e.push_attribute(("style", css.as_str()));
    }
    if let Some(t) = &obj.transform {
        e.push_attribute(("transform", t.as_str()));
    }
}

fn write_object(w: &mut W, obj: &Object) -> Result<()> {
    match &obj.shape {
        Shape::Rect {
            x,
            y,
            width,
            height,
            rx,
        } => {
            let mut e = BytesStart::new("rect");
            e.push_attribute(("x", x.to_string().as_str()));
            e.push_attribute(("y", y.to_string().as_str()));
            e.push_attribute(("width", width.to_string().as_str()));
            e.push_attribute(("height", height.to_string().as_str()));
            if *rx > 0.0 {
                e.push_attribute(("rx", rx.to_string().as_str()));
            }
            common_attrs(&mut e, obj);
            w.write_event(Event::Empty(e))?;
        }
        Shape::Circle { cx, cy, r } => {
            let mut e = BytesStart::new("circle");
            e.push_attribute(("cx", cx.to_string().as_str()));
            e.push_attribute(("cy", cy.to_string().as_str()));
            e.push_attribute(("r", r.to_string().as_str()));
            common_attrs(&mut e, obj);
            w.write_event(Event::Empty(e))?;
        }
        Shape::Ellipse { cx, cy, rx, ry } => {
            let mut e = BytesStart::new("ellipse");
            e.push_attribute(("cx", cx.to_string().as_str()));
            e.push_attribute(("cy", cy.to_string().as_str()));
            e.push_attribute(("rx", rx.to_string().as_str()));
            e.push_attribute(("ry", ry.to_string().as_str()));
            common_attrs(&mut e, obj);
            w.write_event(Event::Empty(e))?;
        }
        Shape::Line { x1, y1, x2, y2 } => {
            let mut e = BytesStart::new("line");
            e.push_attribute(("x1", x1.to_string().as_str()));
            e.push_attribute(("y1", y1.to_string().as_str()));
            e.push_attribute(("x2", x2.to_string().as_str()));
            e.push_attribute(("y2", y2.to_string().as_str()));
            common_attrs(&mut e, obj);
            w.write_event(Event::Empty(e))?;
        }
        Shape::Polygon { points } => {
            let mut e = BytesStart::new("polygon");
            e.push_attribute(("points", points.as_str()));
            common_attrs(&mut e, obj);
            w.write_event(Event::Empty(e))?;
        }
        Shape::Path { d } => {
            let mut e = BytesStart::new("path");
            e.push_attribute(("d", d.as_str()));
            common_attrs(&mut e, obj);
            w.write_event(Event::Empty(e))?;
        }
        Shape::Star { cx, cy, r, points } => {
            let mut e = BytesStart::new("polygon");
            e.push_attribute(("points", star_points(*cx, *cy, *r, *points).as_str()));
            common_attrs(&mut e, obj);
            w.write_event(Event::Empty(e))?;
        }
        Shape::Text {
            x,
            y,
            content,
            font_size,
        } => {
            let mut e = BytesStart::new("text");
            e.push_attribute(("x", x.to_string().as_str()));
            e.push_attribute(("y", y.to_string().as_str()));
            e.push_attribute(("font-size", font_size.to_string().as_str()));
            common_attrs(&mut e, obj);
            w.write_event(Event::Start(e))?;
            // Content is escaped by BytesText — untrusted text cannot inject markup.
            w.write_event(Event::Text(BytesText::new(content)))?;
            w.write_event(Event::End(quick_xml::events::BytesEnd::new("text")))?;
        }
    }
    Ok(())
}

fn write_gradient(w: &mut W, g: &Gradient) -> Result<()> {
    let (tag, attrs, stops): (&str, Vec<(&str, String)>, _) = match g {
        Gradient::Linear {
            id,
            x1,
            y1,
            x2,
            y2,
            stops,
        } => (
            "linearGradient",
            vec![
                ("id", id.clone()),
                ("x1", x1.to_string()),
                ("y1", y1.to_string()),
                ("x2", x2.to_string()),
                ("y2", y2.to_string()),
            ],
            stops,
        ),
        Gradient::Radial {
            id,
            cx,
            cy,
            r,
            stops,
        } => (
            "radialGradient",
            vec![
                ("id", id.clone()),
                ("cx", cx.to_string()),
                ("cy", cy.to_string()),
                ("r", r.to_string()),
            ],
            stops,
        ),
    };
    let mut start = BytesStart::new(tag);
    for (k, v) in &attrs {
        start.push_attribute((*k, v.as_str()));
    }
    w.write_event(Event::Start(start))?;
    for s in stops {
        let mut stop = BytesStart::new("stop");
        stop.push_attribute(("offset", s.offset.to_string().as_str()));
        stop.push_attribute((
            "style",
            format!("stop-color:{};stop-opacity:{}", s.color, s.opacity).as_str(),
        ));
        w.write_event(Event::Empty(stop))?;
    }
    w.write_event(Event::End(quick_xml::events::BytesEnd::new(tag)))?;
    Ok(())
}

/// Compute the points string for an `n`-pointed star.
fn star_points(cx: f64, cy: f64, r: f64, points: u32) -> String {
    let n = points.max(2);
    let inner = r * 0.5;
    let mut out = Vec::with_capacity((n * 2) as usize);
    // Use integer-stepped angles to avoid Math.random/Date concerns; pure math.
    let step = std::f64::consts::PI / n as f64;
    for i in 0..(n * 2) {
        let radius = if i % 2 == 0 { r } else { inner };
        let angle = step * i as f64 - std::f64::consts::FRAC_PI_2;
        let x = cx + radius * angle.cos();
        let y = cy + radius * angle.sin();
        out.push(format!("{x:.3},{y:.3}"));
    }
    out.join(" ")
}
