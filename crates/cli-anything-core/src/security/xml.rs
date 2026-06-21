//! Bounded, entity-safe XML reading (the Rust analog of `defusedxml`).
//!
//! `quick-xml` does not perform DTD/external-entity expansion, which already
//! neutralizes the XXE / "billion laughs" class. This validator makes the
//! protection explicit, defense-in-depth, AND — unlike a pure gate — actively
//! rejects the constructs that a *downstream* renderer (e.g. inkscape) would
//! otherwise act on, so a doc that passes is genuinely safer to hand on:
//! 1. **Reject any `DOCTYPE`/DTD** — internal entity definitions can't be declared.
//! 2. **Reject non-builtin entity references** — only `&amp;/&lt;/&gt;/&apos;/&quot;`
//!    and numeric `&#…;` survive; the billion-laughs defense is then enforced,
//!    not merely incidental.
//! 3. **Reject processing instructions** (e.g. `<?xml-stylesheet href=…?>`) — no
//!    external stylesheet/XSLT fetch.
//! 4. **Reject external `href`/`xlink:href`/`src`** (`http(s):`/`file:`/`ftp:`/
//!    `jar:`/protocol-relative) — blocks SSRF/local-file fetch. `data:`,
//!    fragments, and relative paths are allowed.
//! 5. **Require a single, well-formed root** (no multi-root, no junk text outside
//!    the root, no unclosed-at-EOF).
//! 6. **Caps**: input size (25 MiB), nesting depth (256), element count (1,000,000).
//!
//! Used wherever untrusted SVG/XML enters the system (e.g. inkscape import).

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::error::{CoreError, Result};

/// Default maximum input size in bytes (25 MiB).
pub const DEFAULT_MAX_BYTES: usize = 25 * 1024 * 1024;
/// Default maximum element nesting depth.
pub const DEFAULT_MAX_DEPTH: usize = 256;
/// Default maximum total element count.
pub const DEFAULT_MAX_ELEMENTS: usize = 1_000_000;

/// Tunable safety limits for the XML reader.
#[derive(Debug, Clone)]
pub struct XmlLimits {
    /// Maximum input size in bytes.
    pub max_bytes: usize,
    /// Maximum nesting depth.
    pub max_depth: usize,
    /// Maximum total element count.
    pub max_elements: usize,
}

impl Default for XmlLimits {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            max_depth: DEFAULT_MAX_DEPTH,
            max_elements: DEFAULT_MAX_ELEMENTS,
        }
    }
}

/// Validate untrusted SVG/XML with the default limits, returning the text if it
/// passes every safety check (see the module docs for the full list).
pub fn read_svg_safely(bytes: &[u8]) -> Result<String> {
    read_xml_safely_with(bytes, &XmlLimits::default())
}

/// As [`read_svg_safely`] but with explicit [`XmlLimits`].
pub fn read_xml_safely_with(bytes: &[u8], limits: &XmlLimits) -> Result<String> {
    if bytes.len() > limits.max_bytes {
        return Err(CoreError::XmlTooLarge {
            limit: limits.max_bytes,
            actual: bytes.len(),
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|e| CoreError::XmlMalformed(e.to_string()))?;

    let mut reader = Reader::from_str(text);
    let mut depth: usize = 0;
    let mut elements: usize = 0;
    let mut root_count: usize = 0;

    loop {
        let event = reader
            .read_event()
            .map_err(|e| CoreError::XmlMalformed(e.to_string()))?;
        match event {
            Event::Eof => break,
            Event::DocType(_) => return Err(forbidden("DOCTYPE/DTD declaration")),
            Event::PI(_) => return Err(forbidden("processing instruction")),
            Event::GeneralRef(r) => {
                let name = String::from_utf8_lossy(r.as_ref());
                if !is_allowed_entity(&name) {
                    return Err(forbidden(&format!("entity reference &{name};")));
                }
            }
            Event::Start(e) => {
                if depth == 0 {
                    root_count += 1;
                }
                depth += 1;
                elements += 1;
                check_caps(depth, elements, limits)?;
                reject_external_refs(&e)?;
            }
            Event::Empty(e) => {
                if depth == 0 {
                    root_count += 1;
                }
                elements += 1;
                check_caps(depth, elements, limits)?;
                reject_external_refs(&e)?;
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
            }
            Event::Text(t) => {
                if depth == 0 && !t.iter().all(u8::is_ascii_whitespace) {
                    return Err(CoreError::XmlMalformed(
                        "non-whitespace text outside the root element".into(),
                    ));
                }
            }
            _ => {} // Comment, CData, Decl
        }
    }

    if root_count != 1 {
        return Err(CoreError::XmlMalformed(format!(
            "expected exactly one root element, found {root_count}"
        )));
    }
    if depth != 0 {
        return Err(CoreError::XmlMalformed(
            "unclosed element at end of document".into(),
        ));
    }

    Ok(text.to_string())
}

fn forbidden(what: &str) -> CoreError {
    CoreError::XmlForbiddenEntity {
        what: what.to_string(),
    }
}

fn check_caps(depth: usize, elements: usize, limits: &XmlLimits) -> Result<()> {
    if depth > limits.max_depth {
        return Err(CoreError::XmlTooDeep {
            max: limits.max_depth,
        });
    }
    if elements > limits.max_elements {
        return Err(CoreError::XmlTooManyElements {
            max: limits.max_elements,
        });
    }
    Ok(())
}

/// Only the five XML built-ins and numeric character references survive.
fn is_allowed_entity(name: &str) -> bool {
    matches!(name, "amp" | "lt" | "gt" | "apos" | "quot") || name.starts_with('#')
}

/// Reject `href`/`xlink:href`/`src` attributes pointing at external resources.
/// Namespace declarations (e.g. `xmlns="http://..."`) and `data:`/relative/
/// fragment values are left alone.
fn reject_external_refs(e: &BytesStart) -> Result<()> {
    for attr in e.attributes() {
        let attr = attr.map_err(|err| CoreError::XmlMalformed(err.to_string()))?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_ascii_lowercase();
        let local = key.rsplit(':').next().unwrap_or(&key);
        if local == "href" || local == "src" {
            let value = String::from_utf8_lossy(&attr.value);
            if is_external_ref(&value) {
                return Err(forbidden(&format!(
                    "external reference in '{key}': {}",
                    value.trim()
                )));
            }
        }
    }
    Ok(())
}

fn is_external_ref(value: &str) -> bool {
    let v = value.trim().to_ascii_lowercase();
    const BAD_SCHEMES: &[&str] = &["http://", "https://", "ftp://", "ftps://", "file:", "jar:"];
    v.starts_with("//") || BAD_SCHEMES.iter().any(|p| v.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_formed_svg_passes() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1"/></svg>"#;
        assert!(read_svg_safely(svg.as_bytes()).is_ok());
    }

    #[test]
    fn doctype_is_rejected() {
        let svg = "<!DOCTYPE svg><svg/>";
        let err = read_svg_safely(svg.as_bytes()).unwrap_err();
        assert_eq!(err.kind(), "xml_forbidden_entity");
    }

    #[test]
    fn billion_laughs_is_rejected_at_doctype() {
        let payload = r#"<?xml version="1.0"?>
<!DOCTYPE lolz [
  <!ENTITY lol "lol">
  <!ENTITY lol2 "&lol;&lol;&lol;&lol;&lol;">
  <!ENTITY lol3 "&lol2;&lol2;&lol2;&lol2;&lol2;">
]>
<lolz>&lol3;</lolz>"#;
        let err = read_svg_safely(payload.as_bytes()).unwrap_err();
        // Rejected before any entity could be defined or expanded.
        assert_eq!(err.kind(), "xml_forbidden_entity");
    }

    #[test]
    fn oversize_is_rejected() {
        let limits = XmlLimits {
            max_bytes: 8,
            ..Default::default()
        };
        let err = read_xml_safely_with(b"<svg></svg>", &limits).unwrap_err();
        assert_eq!(err.kind(), "xml_too_large");
    }

    #[test]
    fn over_deep_is_rejected() {
        let mut s = String::new();
        for _ in 0..20 {
            s.push_str("<a>");
        }
        for _ in 0..20 {
            s.push_str("</a>");
        }
        let limits = XmlLimits {
            max_depth: 5,
            ..Default::default()
        };
        let err = read_xml_safely_with(s.as_bytes(), &limits).unwrap_err();
        assert_eq!(err.kind(), "xml_too_deep");
    }

    #[test]
    fn malformed_is_rejected() {
        // Unterminated start tag.
        let err = read_svg_safely(b"<svg").unwrap_err();
        assert_eq!(err.kind(), "xml_malformed");
    }

    #[test]
    fn xmlns_http_namespace_is_allowed() {
        // Regression: xmlns="http://..." must NOT be mistaken for an external ref.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#;
        assert!(read_svg_safely(svg.as_bytes()).is_ok());
    }

    #[test]
    fn xml_stylesheet_pi_is_rejected() {
        let svg = r#"<?xml-stylesheet type="text/xsl" href="http://evil/x.xsl"?><svg/>"#;
        let err = read_svg_safely(svg.as_bytes()).unwrap_err();
        assert_eq!(err.kind(), "xml_forbidden_entity");
    }

    #[test]
    fn external_http_href_is_rejected() {
        let svg =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="http://evil/x.png"/></svg>"#;
        let err = read_svg_safely(svg.as_bytes()).unwrap_err();
        assert_eq!(err.kind(), "xml_forbidden_entity");
    }

    #[test]
    fn external_file_xlink_href_is_rejected() {
        let svg = r#"<svg xmlns:xlink="http://www.w3.org/1999/xlink"><image xlink:href="file:///etc/passwd"/></svg>"#;
        let err = read_svg_safely(svg.as_bytes()).unwrap_err();
        assert_eq!(err.kind(), "xml_forbidden_entity");
    }

    #[test]
    fn data_uri_href_is_allowed() {
        let svg = r#"<svg><image href="data:image/png;base64,iVBORw0KAAAA"/></svg>"#;
        assert!(read_svg_safely(svg.as_bytes()).is_ok());
    }

    #[test]
    fn undefined_entity_reference_is_rejected() {
        let err = read_svg_safely(b"<svg>&xxe;</svg>").unwrap_err();
        assert_eq!(err.kind(), "xml_forbidden_entity");
    }

    #[test]
    fn builtin_and_numeric_entities_are_allowed() {
        assert!(read_svg_safely(b"<svg>a &amp; b &lt; c &#65;</svg>").is_ok());
    }

    #[test]
    fn multiple_roots_are_rejected() {
        let err = read_svg_safely(b"<a/><b/>").unwrap_err();
        assert_eq!(err.kind(), "xml_malformed");
    }

    #[test]
    fn unclosed_at_eof_is_rejected() {
        let err = read_svg_safely(b"<a><b></b>").unwrap_err();
        assert_eq!(err.kind(), "xml_malformed");
    }
}
