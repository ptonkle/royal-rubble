//! The tab icon, made here as an SVG data URL: a stitched castle tower. The page sets it on its `<link rel="icon">`; the CSP's
//! `img-src data:` lets it through. A data URL means no icon file to request
//! and no second copy of the art. After Crabble's and Honey Hole's tab icons.

/// The icon as SVG text, on a 32-unit grid so it stays readable at 16
/// pixels: a crenellated tower worked in wool on linen, in the tapestry
/// palette (`data/look.json`): a tan laid fill with brick-red courses, a
/// dark blue stem-stitch outline and gate, on a linen square with a thin
/// brick rule.
pub fn svg() -> String {
    concat!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'>",
        "<rect x='1' y='1' width='30' height='30' rx='5' fill='#ddd3c8' stroke='#8e4a3b' stroke-width='1.2'/>",
        "<path d='M8 28 V10 H11 V7 H14 V10 H18 V7 H21 V10 H24 V28 Z' fill='#a88e6c'/>",
        "<path d='M8 15 H24 M8 21 H24' stroke='#8e4a3b' stroke-width='1.3' stroke-dasharray='2 1'/>",
        "<path d='M8 28 V10 H11 V7 H14 V10 H18 V7 H21 V10 H24 V28 Z' fill='none' stroke='#2a3646' stroke-width='1.6' stroke-linejoin='round'/>",
        "<path d='M13.5 28 V23 Q16 19.5 18.5 23 V28 Z' fill='#2a3646'/>",
        "</svg>"
    )
    .to_string()
}

/// Percent-encodes everything but the characters a URL keeps as they are.
pub(crate) fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3 / 2);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The tab icon as the `href` of a `<link rel="icon">`.
pub fn data_url() -> String {
    format!("data:image/svg+xml,{}", encode(&svg()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_keeps_only_the_characters_a_url_keeps() {
        // worked by hand: '<' is 3C, ' ' is 20, '#' is 23, '>' is 3E
        assert_eq!(encode("<a b#>"), "%3Ca%20b%23%3E");
        assert_eq!(encode("x-y_z.1~"), "x-y_z.1~");
    }

    #[test]
    fn the_tab_icon_is_an_svg_data_url_a_browser_can_read() {
        let url = data_url();
        let body = url.strip_prefix("data:image/svg+xml,").expect("an SVG data URL");
        assert!(body.starts_with("%3Csvg"), "{body}");
        for raw in ['<', '>', '#', '"', '\'', ' '] {
            assert!(!body.contains(raw), "{raw:?} would end or break the URL");
        }
    }
}
