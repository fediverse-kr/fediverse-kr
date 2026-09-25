//! Shared fictional app UI. Playback and explainer scroll state live in their consumers.
use crate::demo_icons::{DemoIcon, Glyph, IconFamily};
use crate::typing::TypingPreview;
use dioxus::prelude::*;
use lucide_dioxus::{Ellipsis, Globe, MessageCircle};

// Server and character identities from the Phoenix component lab, not its UI.
pub struct DemoSite {
    pub name: &'static str,
    pub domain: &'static str,
}
pub const SITES: [DemoSite; 4] = [
    DemoSite {
        name: "멍멍.개집",
        domain: "dog.town",
    },
    DemoSite {
        name: "냥냥.타워",
        domain: "cat.tower",
    },
    DemoSite {
        name: "추억.사진",
        domain: "photo.town",
    },
    DemoSite {
        name: "곰국.블로그",
        domain: "blog.town",
    },
];
pub struct DemoUser {
    pub name: &'static str,
    pub username: &'static str,
    pub site: usize,
}
impl DemoUser {
    pub fn address(&self) -> String {
        format!("@{}@{}", self.username, SITES[self.site].domain)
    }
}
pub const USERS: [DemoUser; 5] = [
    DemoUser {
        name: "슈나",
        username: "shuna",
        site: 0,
    },
    DemoUser {
        name: "골댕",
        username: "golden",
        site: 0,
    },
    DemoUser {
        name: "페트리샤",
        username: "patricia",
        site: 1,
    },
    DemoUser {
        name: "필름",
        username: "film",
        site: 2,
    },
    DemoUser {
        name: "곰국",
        username: "soup",
        site: 3,
    },
];

#[derive(Clone, Copy, PartialEq)]
pub enum Site {
    Dog,
    Cat,
    Photo,
    Blog,
}
impl Site {
    pub const ALL: [Self; 4] = [Self::Dog, Self::Cat, Self::Photo, Self::Blog];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        SITES[self.index()].name
    }
    pub fn domain(self) -> &'static str {
        SITES[self.index()].domain
    }
    fn software(self) -> &'static str {
        ["moa", "sogon", "bitdam", "yeobaek"][self.index()]
    }
    fn family(self) -> IconFamily {
        [
            IconFamily::Lucide,
            IconFamily::Heroicons,
            IconFamily::Ionicons,
            IconFamily::Bootstrap,
        ][self.index()]
    }
}
#[derive(Clone, Copy, PartialEq)]
pub enum Person {
    Shuna,
    Golden,
    Patricia,
    Film,
    Soup,
}
impl Person {
    pub const ALL: [Self; 5] = [
        Self::Shuna,
        Self::Golden,
        Self::Patricia,
        Self::Film,
        Self::Soup,
    ];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        USERS[self.index()].name
    }
    pub fn username(self) -> &'static str {
        USERS[self.index()].username
    }
    pub fn site(self) -> Site {
        Site::ALL[USERS[self.index()].site]
    }
}

/// The Cat demo needs a site mark, not a cat profile portrait.
/// Do not substitute a Unicode glyph: emoji fallback turns it into a raster-like app icon.
fn brand_glyph(site: Site) -> Option<Glyph> {
    (site == Site::Cat).then_some(Glyph::CatTower)
}

/// Product chrome only. Consumers own the feed and any composer/toast overlay.
/// Decorative controls use the identical markup styles without pretending to work.
#[component]
pub fn SiteWindow(
    site: Site,
    viewer: Person,
    #[props(default = "홈".into())] heading: String,
    #[props(default = "글 / 발행됨".into())] journal_status: String,
    #[props(default = true)] visible: bool,
    #[props(default = "comfortable".into())] density: String,
    #[props(default)] interactive: bool,
    #[props(default)] compose_hint: bool,
    #[props(default)] on_compose: EventHandler,
    #[props(default = rsx! {})] overlays: Element,
    children: Element,
) -> Element {
    let account = viewer.index();
    let name = site.name();
    let photo_site = site == Site::Photo;
    let blog_site = site == Site::Blog;
    rsx! { ProductIcons { key: "{site.software()}", family: site.family(),
        section { class: "social-window", "data-software": site.software(), "data-remote": site != Site::Dog, "data-visible": visible, "data-photo-site": photo_site, "data-density": density, aria_label: "{name} 예시 화면",
            div { class: "social-browser", aria_hidden: "true", span { class: "window-dots", i {} i {} i {} } span { Globe { size: 11 } "{site.domain()}" } Ellipsis { size: 16 } }
            header { class: "social-header",
                if blog_site { div { class: "journal-appbar", strong { "{name}" } } }
                else if photo_site { div { class: "photo-wordmark", DemoIcon { kind: Glyph::Image, size: 23 } strong { "{name}" } } }
                else { div { class: "social-site-title", span { class: "social-brand-mark", aria_hidden: "true", if let Some(kind) = brand_glyph(site) { DemoIcon { kind, size: 27 } } else { MessageCircle { size: 24 } } } strong { "{name}" } } }
                if site == Site::Dog { div { class: "social-header-icons", aria_hidden: "true", DemoIcon { kind: Glyph::Search, size: 18 } Avatar { author: account } } }
                else if photo_site { div { class: "social-header-icons", aria_hidden: "true", DemoIcon { kind: Glyph::Search, size: 18 } DemoIcon { kind: Glyph::Bell, size: 18 } } }
            }
            if blog_site { div { class: "journal-nav", aria_hidden: "true", span { "{journal_status}" } span { "{viewer.name()}" } } }
            else if photo_site { div { class: "photo-nav", aria_hidden: "true", strong { if heading == "홈" { "팔로잉" } else { "{heading}" } } span { "둘러보기" } DemoIcon { kind: Glyph::Image, size: 17 } } }
            else if site == Site::Cat { div { class: "sogon-nav", aria_hidden: "true", span { class: "sogon-selected", "{heading}" } span { "로컬" } span { "소셜" } } }
            else { div { class: "social-nav", aria_hidden: "true", span { class: "social-nav-active", DemoIcon { kind: Glyph::Home, size: 18 } "{heading}" } span { DemoIcon { kind: Glyph::Bell, size: 18 } "알림" } span { DemoIcon { kind: Glyph::Public, size: 18 } "탐색" } } }
            div { class: "social-app-body",
                div { class: "social-feed", tabindex: if interactive { "0" } else { "-1" }, role: "feed", aria_label: "{name} {heading} 타임라인", {children} }
                if site == Site::Cat {
                    aside { class: "misskey-rail", aria_label: "{name} 메뉴",
                        span { class: "rail-active", title: "홈", DemoIcon { kind: Glyph::Home, size: 20 } small { "홈" } }
                        span { title: "알림", DemoIcon { kind: Glyph::Bell, size: 20 } small { "알림" } }
                        span { title: "탐색", DemoIcon { kind: Glyph::Search, size: 20 } small { "탐색" } }
                        DemoControl { class: "rail-compose", label: "{name} 글쓰기", interactive, onclick: on_compose, DemoIcon { kind: Glyph::Write, size: 22 } small { "글쓰기" } }
                        span { class: "rail-more", title: "더 보기", DemoIcon { kind: Glyph::More, size: 21 } }
                        span { class: "rail-profile", title: "{USERS[account].address()}", Avatar { author: account } small { "{viewer.name()}" } }
                    }
                }
                if site == Site::Dog { DemoControl { class: "mastodon-compose-fab", label: "{name} 글쓰기", interactive, onclick: on_compose, DemoIcon { kind: Glyph::Write, size: 22 } if compose_hint { ClickRipple {} } } }
                {overlays}
            }
            if photo_site { div { class: "photo-bottom", DemoIcon { kind: Glyph::Home, size: 19 }
                DemoControl { class: "photo-compose", label: "{name} 사진 올리기", interactive, onclick: on_compose, DemoIcon { kind: Glyph::Image, size: 21 } if compose_hint { ClickRipple {} } }
                DemoIcon { kind: Glyph::Heart, size: 19 } Avatar { author: account }
            } }
        }
    } }
}

/// Replace the icon context together with a changed product, without adding DOM.
#[component]
fn ProductIcons(family: IconFamily, children: Element) -> Element {
    use_context_provider(move || family);
    children
}

#[component]
fn DemoControl(
    class: String,
    label: String,
    interactive: bool,
    onclick: EventHandler,
    children: Element,
) -> Element {
    if interactive {
        rsx! { button { class, aria_label: label, onclick: move |_| onclick.call(()), {children} } }
    } else {
        rsx! { span { class, title: label, aria_hidden: "true", {children} } }
    }
}

#[component]
pub fn JournalArticle(
    title: String,
    body: String,
    #[props(default)] highlighted: bool,
    #[props(default)] replies: usize,
    #[props(default = rsx! {})] children: Element,
) -> Element {
    rsx! { article { class: "journal-article", "data-published": true, "data-highlighted": highlighted,
        div { class: "journal-meta", span { "일상 · 3분 읽기" } span { "✓ 발행됨" } }
        h3 { "{title}" }
        div { class: "journal-byline", Avatar { author: Person::Soup.index() } span { "{Person::Soup.name()}" } }
        img { src: PHOTO, alt: "산책길에서 만난 고양이", width: "960", height: "640" }
        p { "{body}" }
        div { class: "journal-discussion",
            div { class: "journal-discussion-title", DemoIcon { kind: Glyph::Public, size: 15 } "소셜 웹 답글" span { "{replies}" } }
            if replies == 0 { p { class: "journal-no-comments", "아직 답글이 없어요." } } else { {children} }
        }
    } }
}

#[component]
pub fn JournalComment(author: usize, body: String, #[props(default)] highlighted: bool) -> Element {
    rsx! { div { class: "journal-comment", "data-incoming": true, "data-highlighted": highlighted,
        Avatar { author }
        div { strong { "{USERS[author].name}" } small { "{USERS[author].address()}" } p { "{body}" } }
    } }
}

const SHUNA: Asset = asset!(
    "/assets/images/shuna.png",
    AssetOptions::image()
        .with_jpg()
        .with_size(ImageSize::Manual {
            width: 128,
            height: 128
        })
);
const PATRICIA: Asset = asset!(
    "/assets/images/patricia.png",
    AssetOptions::image()
        .with_jpg()
        .with_size(ImageSize::Manual {
            width: 128,
            height: 128
        })
);
const GOLDEN: Asset = asset!(
    "/assets/images/golden.png",
    AssetOptions::image()
        .with_jpg()
        .with_size(ImageSize::Manual {
            width: 128,
            height: 128
        })
);
const FILM: Asset = asset!(
    "/assets/images/film.png",
    AssetOptions::image()
        .with_jpg()
        .with_size(ImageSize::Manual {
            width: 128,
            height: 128
        })
);
const SOUP: Asset = asset!(
    "/assets/images/soup.png",
    AssetOptions::image()
        .with_jpg()
        .with_size(ImageSize::Manual {
            width: 128,
            height: 128
        })
);
pub const PHOTO: Asset = asset!(
    "/assets/images/neighborhood.png",
    AssetOptions::image()
        .with_jpg()
        .with_size(ImageSize::Manual {
            width: 960,
            height: 640
        })
);

#[component]
pub fn Arrival(children: Element) -> Element {
    rsx! { div { class: "social-arrival", div { class: "social-arrival-content", {children} } } }
}

#[component]
pub fn ClickRipple() -> Element {
    rsx! { span { class: "demo-click-ripple", aria_hidden: "true" } }
}

#[component]
pub fn SocialPost(
    author: usize,
    body: String,
    #[props(default)] reply_to: String,
    #[props(default)] photo: bool,
    #[props(default)] photo_site: bool,
    #[props(default)] article_preview: bool,
    #[props(default)] old: bool,
    #[props(default)] remote_arrival: bool,
    #[props(default)] fresh: bool,
    #[props(default)] compact: bool,
    #[props(default)] reply_count: u8,
    #[props(default)] reply_selected: bool,
    #[props(default)] highlighted: bool,
    #[props(default = true)] reveal_address: bool,
    #[props(default)] scope: String,
) -> Element {
    let identity = &USERS[author];
    let (name, user, domain) = (
        identity.name,
        identity.username,
        SITES[identity.site].domain,
    );
    let mut liked = use_signal(|| false);
    rsx! { article { class: "social-post", "data-person": user, "data-photo": photo, "data-photo-native": photo_site, "data-incoming": remote_arrival, "data-highlighted": highlighted, "data-fresh": fresh, "data-compact": compact,
        if !reply_to.is_empty() { div { class: "social-reply-context", DemoIcon { kind: Glyph::Reply, size: 13 } "{reply_to} 님에게 답글" } }
        div { class: "social-post-heading", Avatar { author } div { strong { "{name}" } span { "@{user}" if reveal_address { b { class: "social-domain", "data-site": identity.site, "@{domain}" } } } } time { if old { "12분" } else { "방금" } } }
        div { class: "social-post-content",
            if !reply_to.is_empty() { span { class: "social-mention", "@{reply_to} " } }
            span { "{body}" }
        }
        if !scope.is_empty() { small { class: "social-post-scope", "{scope}" } }
        if photo { img { class: "social-photo", src: PHOTO, alt: "햇살이 드는 골목길에 앉아 있는 고양이", width: "960", height: "640" } }
        if article_preview { div { class: "social-article-preview",
            img { src: PHOTO, alt: "산책길에서 만난 고양이", width: "960", height: "640" }
            div { small { "{SITES[3].name} · {USERS[4].name}의 작은 기록" } strong { "산책의 속도로 사는 하루" } p { "늘 지나치던 골목에서 고양이를 만났어요." } }
        } }
        div { class: "social-post-actions",
            span { aria_hidden: "true", class: "post-reply-action", "data-selected": reply_selected, DemoIcon { kind: Glyph::Reply, size: 17 } if reply_count > 0 { small { "{reply_count}" } } if reply_selected { ClickRipple {} } }
            span { aria_hidden: "true", DemoIcon { kind: Glyph::Repeat, size: 18 } }
            button { aria_label: format!("{name}의 글 좋아요"), aria_pressed: liked(), onclick: move |_| liked.set(!liked()), DemoIcon { kind: Glyph::Heart, size: 17, selected: liked() } if liked() { small { "1" } } }
            span { aria_hidden: "true", DemoIcon { kind: Glyph::More, size: 19 } }
        }
    } }
}

/// Shared profile surface; a moved account can have a different home site.
#[component]
pub fn ProfileCard(
    person: Person,
    site: Site,
    state: String,
    #[props(default)] highlighted: bool,
) -> Element {
    rsx! { section { class:"social-profile-card", "data-highlighted":highlighted,
        Avatar { author:person.index() }
        h3 { "{person.name()}" }
        p { "@{person.username()}" b { class:"social-domain", "data-site":site.index(), "@{site.domain()}" } }
        span { class:"social-profile-state", "{state}" }
    } }
}

#[component]
pub fn Avatar(author: usize) -> Element {
    let source = match author {
        0 => SHUNA,
        1 => GOLDEN,
        2 => PATRICIA,
        3 => FILM,
        _ => SOUP,
    };
    rsx! { img { class: "social-avatar", src: source, alt: "", width: "40", height: "40" } }
}
#[component]
pub fn Composer(
    #[props(default)] sending: bool,
    #[props(default)] submit_hint: bool,
    #[props(default)] photo_hint: bool,
    #[props(default)] photo_selected: bool,
    author: usize,
    message: String,
    #[props(default)] reply: bool,
    #[props(default)] recipient: usize,
    #[props(default)] quote: String,
    #[props(default)] photo: bool,
    #[props(default)] interactive: bool,
    #[props(default)] on_close: EventHandler,
    #[props(default)] on_submit: EventHandler<String>,
) -> Element {
    let mut draft = use_signal(String::new);
    let mut attached = use_signal(|| false);
    let visible_text = if interactive { draft() } else { message };
    let can_publish = interactive && !visible_text.trim().is_empty() && (!photo || attached());
    let name = USERS[author].name;
    rsx! { div { class: "social-compose-layer", "data-photo-editor": photo,
        section { class: "social-composer", "data-sending": sending, "data-photo": photo, "data-interactive": interactive, aria_label: "글을 작성하고 전송하는 장면",
            div { class: "compose-titlebar",
                button { class: "compose-close", aria_label: "작성 창 닫기", onclick: move |_| on_close.call(()), DemoIcon { kind: if photo { Glyph::Back } else { Glyph::Close }, size: 18 } }
                strong { if photo { "새 게시물" } else if reply { "답글 작성" } else { "새 글 작성" } }
                if photo { button { class: "social-publish", disabled: !can_publish, onclick: move |_| on_submit.call(draft().trim().to_string()),
                    if submit_hint { ClickRipple {} }
                    if sending { span { class: "social-spinner" } "공유 중" } else { "공유" }
                } }
            }
            if reply { div { class: "compose-reply-preview", Avatar { author: recipient } div { strong { "{USERS[recipient].name}" } p { "{quote}" } } } }
            div { class: "social-compose-author", Avatar { author } div { strong { "{name}" } small { DemoIcon { kind: Glyph::Public, size: 11 } "공개" span { "⌄" } } } }
            if photo {
                if !attached() && (interactive || !photo_selected) { button { class: "compose-photo-picker", onclick: move |_| attached.set(true), DemoIcon { kind: Glyph::Image, size: 32 } span { "사진을 추가하세요" } if photo_hint { ClickRipple {} } } }
                else { div { class: "compose-attachment", img { src: PHOTO, alt: "선택한 산책길 사진", width: "960", height: "640" } span { "ALT" } } }
            }
            if reply { div { class: "compose-recipient", "{USERS[recipient].address()}" } }
            if interactive {
                textarea { class: "social-draft", aria_label: "게시할 글", maxlength: "500", oninput: move |event| draft.set(event.value()), placeholder: if photo { "사진에 대해 이야기해 보세요…" } else if reply { "답글을 남겨보세요…" } else { "무슨 생각을 하고 있나요?" }, value: "{visible_text}" }
            } else {
                TypingPreview { class: "social-draft", label: "게시할 글 (자동 재생)", placeholder: if photo { "사진에 대해 이야기해 보세요…" } else if reply { "답글을 남겨보세요…" } else { "무슨 생각을 하고 있나요?" }, text: visible_text.clone(), caret: !sending }
            }
            if !photo { div { class: "social-compose-tools",
                span { aria_hidden: "true", DemoIcon { kind: Glyph::Image, size: 18 } DemoIcon { kind: Glyph::Smile, size: 18 } span { class: "compose-cw", "CW" } }
                small { "{500usize.saturating_sub(visible_text.chars().count())}" }
                button { class: "social-publish", disabled: !can_publish, "data-empty": visible_text.is_empty(), onclick: move |_| on_submit.call(draft().trim().to_string()),
                    if submit_hint { ClickRipple {} }
                    if sending { span { class: "social-spinner" } "게시 중" } else { "게시하기" }
                }
            } } else { div { class: "photo-post-options", span { "위치 추가" } span { "›" } } }
        }
    } }
}

#[component]
pub fn JournalEditor(
    title: String,
    text: String,
    saved: bool,
    sending: bool,
    submit_hint: bool,
    caret: bool,
) -> Element {
    rsx! {
            section { class: "journal-editor", aria_label: "블로그 글 작성 데모",
                div { class: "journal-editor-toolbar", span { if !saved { "초안 저장됨" } else { "모든 변경사항 저장됨" } }
                    button { disabled: true, class: "journal-publish", "data-sending": sending,
                        if submit_hint { ClickRipple {} }
                        if sending { span { class: "social-spinner" } "발행 중" } else { "발행하기" }
                    }
                }
                label { class: "sr-only", r#for: "journal-draft-title", "글 제목 (자동 재생)" }
                input { id: "journal-draft-title", class: "journal-title-input", readonly: true, tabindex: "-1", value: title }
                img { class: "journal-editor-cover", src: PHOTO, alt: "글에 첨부한 산책길 사진", width: "960", height: "640" }
                div { class: "journal-format-tools", aria_hidden: "true", b { "B" } i { "I" } span { "H₂" } span { "❝" } DemoIcon { kind: Glyph::Image, size: 15 } }
                TypingPreview { class: "journal-body-input", label: "블로그 본문 (자동 재생)", placeholder: "이야기를 시작해 보세요…", text: text.clone(), caret: caret }
                div { class: "journal-editor-status", span { "공개 글 · 연합우주에 공유" } span { "{text.chars().count()}자" } }

            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cat_brand_uses_cat_tower_svg_not_a_unicode_glyph() {
        assert!(matches!(Site::Cat.family(), IconFamily::Heroicons));
        assert!(matches!(brand_glyph(Site::Cat), Some(Glyph::CatTower)));
        assert!(brand_glyph(Site::Dog).is_none());
    }
}
