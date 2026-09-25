//! XML syntax belongs to quick-xml; allowed document shape remains our policy.
//! Recognition is NOT sanitization. Serve only as a separate sandboxed image.
use quick_xml::{Reader, events::Event};

pub(super) fn recognize(bytes: &[u8]) -> bool {
    if bytes.len() > super::super::storage::MAX_BYTES {
        return false;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut reader = Reader::from_str(text.trim_start_matches('\u{feff}'));
    reader.config_mut().enable_all_checks(true);
    reader.config_mut().expand_empty_elements = true;
    let mut depth = 0usize;
    let mut root = false;
    let mut declaration = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                if depth >= 128
                    || (depth == 0 && (root || element.name().as_ref() != b"svg"))
                    || element.attributes().any(|attr| attr.is_err())
                {
                    return false;
                }
                root = true;
                depth += 1;
            }
            Ok(Event::End(_)) if depth > 0 => depth -= 1,
            Ok(Event::Decl(decl)) if !root && !declaration => {
                if decl.version().is_err() {
                    return false;
                }
                declaration = true;
            }
            Ok(Event::Comment(_)) => {}
            Ok(Event::Text(text)) if depth > 0 || text.iter().all(u8::is_ascii_whitespace) => {}
            Ok(Event::CData(_) | Event::GeneralRef(_)) if depth > 0 => {}
            Ok(Event::Eof) => return root && depth == 0,
            // In particular: DTDs, external entity declarations and processing
            // instructions are not needed for retained image previews.
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xml_parser_rejects_broken_and_multiple_roots_and_bounds_nesting() {
        for bad in [
            "<svg>",
            "<svg><g></svg>",
            "<svg/><svg/>",
            "<svg/>tail",
            "<svg a='1' a='2'/>",
            "<!--bad--comment--><svg/>",
            "<?xml-stylesheet href='https://example.org/a'?><svg/>",
            "<svg/><!DOCTYPE svg>",
            "<svg><!ENTITY x 'expanded'></svg>",
        ] {
            assert!(!recognize(bad.as_bytes()), "accepted malformed XML");
        }
        assert!(recognize(
            b"<svg><text>A &amp; B</text><![CDATA[<g>]]></svg>"
        ));
        assert!(!recognize(
            format!("<svg>{}{}</svg>", "<g>".repeat(128), "</g>".repeat(128)).as_bytes()
        ));
        // Deliberately not a sanitizer: response CSP must still block scripts.
        assert!(recognize(
            b"<svg onload='alert(1)'><script>alert(1)</script></svg>"
        ));
    }
}
