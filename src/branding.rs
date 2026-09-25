use dioxus::prelude::*;

/// One canonical, self-contained brand mark for the browser chrome and site shell.
pub const BRAND_MARK: Asset = asset!("/assets/fediverse-kr-mark.svg");

/// Decorative mark; the adjacent visible wordmark supplies the accessible name.
#[component]
pub fn BrandMark() -> Element {
    rsx! {
        img {
            class: "brand-mark",
            src: BRAND_MARK,
            alt: "",
            aria_hidden: "true",
        }
    }
}

#[cfg(test)]
mod tests {
    const BRAND_MARK_SVG: &str = include_str!("../assets/fediverse-kr-mark.svg");

    #[test]
    fn brand_mark_is_a_self_contained_safe_svg() {
        assert!(BRAND_MARK_SVG.trim_start().starts_with("<svg"));
        assert!(BRAND_MARK_SVG.contains("viewBox=\"0 0 120 120\""));
        assert!(BRAND_MARK_SVG.contains("<title id=\"title\">fediverse.kr shared orbit</title>"));
        assert!(BRAND_MARK_SVG.contains("<ellipse"));
        assert!(BRAND_MARK_SVG.contains("stroke=\"#AA94E8\""));
        assert!(BRAND_MARK_SVG.contains("transform=\"rotate(-26 60 60)\""));
        assert!(BRAND_MARK_SVG.contains("fill=\"#A685FF\""));
        assert!(BRAND_MARK_SVG.contains("fill=\"#5145A6\""));
        for forbidden in [
            "<script",
            "<image",
            "url(",
            "@font-face",
            "<foreignObject",
            "<mask",
            "<clipPath",
            "태극",
            "단청",
        ] {
            assert!(
                !BRAND_MARK_SVG.contains(forbidden),
                "brand mark must not contain {forbidden}"
            );
        }
    }
}
