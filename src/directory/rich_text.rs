//! Server-side rendering for public descriptions. Storage keeps the source unchanged.
use comrak::{markdown_to_html, Options};
use sanitize_html::{
    rules::{pattern::Pattern, Element, Rules},
    sanitize_str,
};
use std::sync::LazyLock;
use url::Url;

pub(crate) struct RenderedDescription {
    pub html: String,
    pub plain: String,
}

/// Render CommonMark/GFM with raw HTML, then apply an HTML5 DOM allowlist.
pub(crate) fn render_description(source: &str) -> RenderedDescription {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    // Raw HTML is safe only after the strict DOM sanitizer below.
    options.render.r#unsafe = true;

    let rendered = markdown_to_html(source, &options);
    let html = sanitize_str(&DESCRIPTION_RULES, &rendered).unwrap_or_default();
    let plain = html2text::from_read_with_decorator(
        html.as_bytes(),
        8_000,
        html2text::render::TrivialDecorator::new(),
    )
    .unwrap_or_default()
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ");

    RenderedDescription { html, plain }
}

static DESCRIPTION_RULES: LazyLock<Rules> = LazyLock::new(|| {
    let mut rules = Rules::new();
    for tag in [
        "b",
        "blockquote",
        "br",
        "code",
        "del",
        "em",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "i",
        "li",
        "ol",
        "p",
        "pre",
        "s",
        "strong",
        "table",
        "tbody",
        "td",
        "th",
        "thead",
        "tr",
        "ul",
    ] {
        rules = rules.element(Element::new(tag));
    }
    rules = rules.element(
        Element::new("a")
            .attribute("href", absolute_http_url())
            .attribute("title", Pattern::any())
            .mandatory_attribute("rel", "noopener noreferrer ugc"),
    );
    for tag in [
        "base", "embed", "form", "iframe", "link", "math", "meta", "noscript", "object", "script",
        "style", "svg", "template",
    ] {
        rules = rules.delete(tag);
    }
    rules
});

fn absolute_http_url() -> Pattern {
    Pattern(Box::new(|value| {
        if value.is_empty()
            || value.trim() != value
            || value.chars().any(|character| character.is_ascii_control())
        {
            return false;
        }
        Url::parse(value)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.has_host())
    }))
}
