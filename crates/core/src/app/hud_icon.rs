//! The HUD's icon set, made here as SVG data URLs. Each icon is one
//! silhouette on a 24-unit grid; the page uses it as a CSS mask and the
//! stylesheet colors it by name (`.icon[data-icon=wood]`), so the palette
//! stays in the look's tokens and one set serves every HUD item. The CSP's
//! `img-src data:` lets them through, as it does the tab icon.

/// The shapes, by name: SVG body text inside a 24 by 24 view box. Strokes
/// and fills are black; only their coverage matters to a mask.
const SHAPES: &[(&str, &str)] = &[
    // materials
    (
        "wood",
        "<path fill-rule='evenodd' d='M7 6 H18 A6 6 0 0 1 18 18 H7 A6 6 0 0 1 7 6 Z M18 9 A3 3 0 0 0 18 15 A3 3 0 0 0 18 9 Z'/>",
    ),
    ("stone", "<path d='M3 19 L5 10 L10 5 L17 6 L21 12 L19 19 Z'/>"),
    ("metal", "<path d='M2 18 L6 9 H18 L22 18 Z M8 7 L9 5 H15 L16 7 Z'/>"),
    (
        "rope",
        "<path fill='none' stroke='black' stroke-width='2.5' d='M5 4 C11 4 11 10 5 10 C11 10 11 16 5 16 C11 16 11 22 5 22 M15 2 C21 2 21 8 15 8 C21 8 21 14 15 14 C21 14 21 20 15 20'/>",
    ),
    // purse
    (
        "gold",
        "<path fill-rule='evenodd' d='M12 2 A10 10 0 1 1 12 22 A10 10 0 1 1 12 2 Z M12 6 A6 6 0 1 0 12 18 A6 6 0 1 0 12 6 Z'/><path d='M11 8 H13 V16 H11 Z'/>",
    ),
    ("xp", "<path d='M12 2 L15 9 L22 9 L16 14 L18 21 L12 17 L6 21 L8 14 L2 9 L9 9 Z'/>"),
    ("level", "<path d='M12 2 L21 7 V17 L12 22 L3 17 V7 Z'/>"),
    ("royal", "<path d='M3 19 L2 7 L8 12 L12 4 L16 12 L22 7 L21 19 Z'/>"),
    ("heart", "<path d='M12 21 L3 12 C0 8 3 3 7 3 C9 3 11 4 12 6 C13 4 15 3 17 3 C21 3 24 8 21 12 Z'/>"),
    ("shield", "<path d='M12 2 L21 5 V11 C21 16 17 20 12 22 C7 20 3 16 3 11 V5 Z'/>"),
    ("skull", "<path fill-rule='evenodd' d='M12 2 C18 2 21 6 21 11 C21 14 19 16 17 17 V21 H7 V17 C5 16 3 14 3 11 C3 6 6 2 12 2 Z M8 10 A2 2 0 1 0 8 14 A2 2 0 1 0 8 10 Z M16 10 A2 2 0 1 0 16 14 A2 2 0 1 0 16 10 Z'/>"),
    ("timer", "<path d='M5 2 H19 V4 H17 V7 L13 12 L17 17 V20 H19 V22 H5 V20 H7 V17 L11 12 L7 7 V4 H5 Z'/>"),
    ("target", "<path fill-rule='evenodd' d='M12 2 A10 10 0 1 1 12 22 A10 10 0 1 1 12 2 Z M12 5 A7 7 0 1 0 12 19 A7 7 0 1 0 12 5 Z M12 8 A4 4 0 1 1 12 16 A4 4 0 1 1 12 8 Z'/>"),
    // phases
    ("upgrade", "<path d='M12 2 L21 11 H15 V22 H9 V11 H3 Z'/>"),
    ("build", "<path d='M3 5 H15 L18 8 V10 H12 V22 H8 V10 H3 Z'/>"),
    ("siege", "<path d='M3 21 L13 11 L11 9 L17 3 H21 V7 L15 13 L13 11 L5 21 Z M2 16 L8 22 L6 23 L1 18 Z'/>"),
    // producers
    ("carpenter", "<path d='M2 18 L16 4 L20 8 L6 22 Z M14 2 L22 10 L20 12 L12 4 Z'/>"),
    ("quarry", "<path d='M2 8 C8 2 16 2 22 8 L20 9 C15 5 9 5 4 9 Z M11 6 H13 V22 H11 Z'/>"),
    ("mine", "<path fill-rule='evenodd' d='M2 7 H22 L19 16 H5 Z M7 17 A2.5 2.5 0 1 1 7 22 A2.5 2.5 0 1 1 7 17 Z M17 17 A2.5 2.5 0 1 1 17 22 A2.5 2.5 0 1 1 17 17 Z'/>"),
    ("armory", "<path d='M12 3 A4 4 0 1 1 12 11 A4 4 0 1 1 12 3 Z M7 12 A4 4 0 1 1 7 20 A4 4 0 1 1 7 12 Z M17 12 A4 4 0 1 1 17 20 A4 4 0 1 1 17 12 Z'/>"),
    // shots
    ("normal", "<path d='M12 5 A7 7 0 1 1 12 19 A7 7 0 1 1 12 5 Z'/>"),
    ("heavy", "<path fill-rule='evenodd' d='M12 2 A10 10 0 1 1 12 22 A10 10 0 1 1 12 2 Z M3 11 H21 V13 H3 Z'/>"),
    ("fire", "<path d='M12 2 C14 7 19 9 19 15 A7 7 0 0 1 5 15 C5 11 8 9 9 6 C10 9 11 10 12 10 C12 7 11 5 12 2 Z'/>"),
    ("spread", "<path d='M12 3 A3 3 0 1 1 12 9 A3 3 0 1 1 12 3 Z M5 13 A3 3 0 1 1 5 19 A3 3 0 1 1 5 13 Z M19 13 A3 3 0 1 1 19 19 A3 3 0 1 1 19 13 Z'/>"),
    ("exploding", "<path d='M12 1 L14 8 L21 4 L17 10 L23 12 L17 14 L21 20 L14 16 L12 23 L10 16 L3 20 L7 14 L1 12 L7 10 L3 4 L10 8 Z'/>"),
    ("chain", "<path d='M6 8 A4 4 0 1 1 6 16 A4 4 0 1 1 6 8 Z M18 8 A4 4 0 1 1 18 16 A4 4 0 1 1 18 8 Z M9 11 H15 V13 H9 Z'/>"),
    ("meteor", "<path d='M15 9 A6 6 0 1 1 15 21 A6 6 0 1 1 15 9 Z M2 2 L12 10 L10 12 Z M8 1 L15 8 L13 9 Z M1 8 L9 14 L8 16 Z'/>"),
    // tricks
    ("drop", "<path d='M10 2 H14 V14 H19 L12 22 L5 14 H10 Z'/>"),
    ("reverse", "<path d='M20 20 V10 A6 6 0 0 0 8 10 V12 H12 L6 19 L0 12 H4 V10 A10 10 0 0 1 24 10 V20 Z'/>"),
    ("phase", "<path fill='none' stroke='black' stroke-width='2.5' stroke-dasharray='3 2.5' d='M12 3 A9 9 0 1 1 12 21 A9 9 0 1 1 12 3 Z'/><path d='M12 9 A3 3 0 1 1 12 15 A3 3 0 1 1 12 9 Z'/>"),
    ("split", "<path d='M10 22 V12 L4 6 L2 8 V2 H8 L6 4 L12 10 L18 4 L16 2 H22 V8 L20 6 L14 12 V22 Z'/>"),
    ("dive", "<path d='M3 5 L5 3 L17 15 V8 H21 V21 H8 V17 H15 Z'/>"),
    ("boost", "<path d='M2 4 L10 12 L2 20 V15 L5 12 L2 9 Z M12 4 L20 12 L12 20 V15 L15 12 L12 9 Z'/>"),
    ("hover", "<path d='M12 2 A10 10 0 1 1 12 22 A10 10 0 1 1 12 2 Z' fill-opacity='0.35'/><path d='M8 7 H11 V17 H8 Z M13 7 H16 V17 H13 Z'/>"),
    // actions
    ("buy_shot", "<path d='M10 7 A7 7 0 1 1 10 21 A7 7 0 1 1 10 7 Z M18 1 H20 V4 H23 V6 H20 V9 H18 V6 H15 V4 H18 Z'/>"),
    ("test", "<path fill='none' stroke='black' stroke-width='2.5' stroke-linejoin='round' d='M1 13 H5 L8 5 L12 20 L15 9 L17 13 H23'/>"),
    ("scout", "<path fill-rule='evenodd' d='M1 12 C5 5 19 5 23 12 C19 19 5 19 1 12 Z M12 8 A4 4 0 1 0 12 16 A4 4 0 1 0 12 8 Z'/>"),
    ("ready", "<path d='M2 13 L5 10 L9 14 L19 4 L22 7 L9 20 Z'/>"),
    ("reroll", "<path d='M12 3 A9 9 0 0 1 20 8 L23 6 V14 H15 L18 11 A6 6 0 0 0 6 12 H3 A9 9 0 0 1 12 3 Z M12 21 A9 9 0 0 1 4 16 L1 18 V10 H9 L6 13 A6 6 0 0 0 18 12 H21 A9 9 0 0 1 12 21 Z'/>"),
    ("leave", "<path d='M3 2 H14 V8 H11 V5 H6 V19 H11 V16 H14 V22 H3 Z M14 9 V6 L21 12 L14 18 V15 H9 V9 Z'/>"),
    ("skip", "<path d='M2 4 L12 12 L2 20 Z M12 4 L22 12 L12 20 Z'/>"),
    // card categories
    ("defense", "<path d='M12 2 L21 5 V11 C21 16 17 20 12 22 C7 20 3 16 3 11 V5 Z'/>"),
    ("economy", "<path fill-rule='evenodd' d='M12 2 A10 10 0 1 1 12 22 A10 10 0 1 1 12 2 Z M12 6 A6 6 0 1 0 12 18 A6 6 0 1 0 12 6 Z'/>"),
];

/// Every icon name the set holds.
pub fn names() -> impl Iterator<Item = &'static str> {
    SHAPES.iter().map(|(n, _)| *n)
}

/// The icon `name` as SVG text, or None for a name the set lacks.
pub fn svg(name: &str) -> Option<String> {
    let (_, body) = SHAPES.iter().find(|(n, _)| *n == name)?;
    Some(format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'>{body}</svg>"
    ))
}

/// The icon `name` as a data URL, or "" for a name the set lacks (the page
/// then draws no icon).
pub fn url(name: &str) -> String {
    svg(name).map_or_else(String::new, |s| {
        format!("data:image/svg+xml,{}", crate::icon::encode(&s))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_icons_are_svg_data_urls_with_unique_names() {
        let mut seen: Vec<&str> = Vec::new();
        for n in names() {
            assert!(!seen.contains(&n), "{n} is in the set twice");
            seen.push(n);
            let u = url(n);
            let body = u.strip_prefix("data:image/svg+xml,").expect("an SVG data URL");
            assert!(body.starts_with("%3Csvg"), "{n}: {body}");
            assert!(body.ends_with("%3C%2Fsvg%3E"), "{n}: {body}");
        }
        assert_eq!(url("no such icon"), "");
    }
}
