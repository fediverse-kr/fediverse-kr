//! Each fictional client owns an icon family, including remote posts it renders.
//! SVG paths come from the bundled icon packs; no runtime icon API or font request.
use dioxus::prelude::*;
use dioxus_free_icons::{
    IconShape,
    icons::{bs_icons::*, hi_solid_icons::*, io_icons::*},
};
use dioxus_icons::lucide::{Pencil, Smile, X};
use lucide_dioxus::{
    ArrowLeft, Bell, Ellipsis, Globe, Heart, House, Image, MessageCircle, Repeat2, Search,
};

#[derive(Clone, Copy, PartialEq)]
pub enum IconFamily {
    Lucide,
    Heroicons,
    Ionicons,
    Bootstrap,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Glyph {
    Home,
    Bell,
    Search,
    Write,
    Image,
    Public,
    Reply,
    Repeat,
    Heart,
    More,
    Back,
    Close,
    Smile,
    CatTower,
}

/// A compact, single-color cat tower: ear-topped perch, platform, and cubby.
/// It is intentionally a site mark rather than a cat-face/profile icon.
#[component]
fn CatTowerIcon(size: usize) -> Element {
    rsx! { svg { class: "demo-icon", "data-icon-pack": "cat-tower", width: size, height: size,
        view_box: "0 0 24 24", fill: "currentColor", stroke: "none", stroke_width: "0",
        "aria-hidden": "true", "focusable": "false",
        path { fill_rule: "evenodd", clip_rule: "evenodd", d: "M7.3 2.2 9.2 3.7 12 1.7l2.8 2 1.9-1.5v3H7.3V2.2Zm2.8 3h3.8v3H10.1v-3ZM5 8.2h14v2.1H5V8.2Zm2.1 2.1h9.8v10.9H7.1V10.3Zm4.9 2.1a2.8 2.8 0 0 0-2.8 2.8v4h5.6v-4a2.8 2.8 0 0 0-2.8-2.8Z" }
    } }
}

#[component]
pub fn DemoIcon(
    kind: Glyph,
    #[props(default = 20)] size: usize,
    #[props(default)] selected: bool,
) -> Element {
    let family = use_context::<IconFamily>();
    if kind == Glyph::CatTower {
        return rsx! { CatTowerIcon { size } };
    }
    if family == IconFamily::Lucide {
        return match kind {
            Glyph::Home => rsx! { House { size, class: "demo-icon icon-lucide" } },
            Glyph::Bell => rsx! { Bell { size, class: "demo-icon icon-lucide" } },
            Glyph::Search => rsx! { Search { size, class: "demo-icon icon-lucide" } },
            Glyph::Image => rsx! { Image { size, class: "demo-icon icon-lucide" } },
            Glyph::Public => rsx! { Globe { size, class: "demo-icon icon-lucide" } },
            Glyph::Reply => rsx! { MessageCircle { size, class: "demo-icon icon-lucide" } },
            Glyph::Repeat => rsx! { Repeat2 { size, class: "demo-icon icon-lucide" } },
            Glyph::Heart => {
                rsx! { Heart { size, class: "demo-icon icon-lucide", fill: if selected { "currentColor" } else { "none" } } }
            }
            Glyph::More => rsx! { Ellipsis { size, class: "demo-icon icon-lucide" } },
            Glyph::Back => rsx! { ArrowLeft { size, class: "demo-icon icon-lucide" } },
            Glyph::Write => rsx! { Pencil { size, class: "demo-icon icon-lucide" } },
            Glyph::Close => rsx! { X { size, class: "demo-icon icon-lucide" } },
            Glyph::Smile => rsx! { Smile { size, class: "demo-icon icon-lucide" } },
            Glyph::CatTower => unreachable!("cat tower returns before family mapping"),
        };
    }
    let pack = match family {
        IconFamily::Heroicons => "heroicons-solid",
        IconFamily::Ionicons => "ionicons",
        _ => "bootstrap",
    };
    let shape: &dyn IconShape = match (family, kind) {
        (IconFamily::Heroicons, Glyph::Home) => &HiHome,
        (IconFamily::Heroicons, Glyph::Bell) => &HiBell,
        (IconFamily::Heroicons, Glyph::Search) => &HiSearch,
        (IconFamily::Heroicons, Glyph::Write) => &HiPencilAlt,
        (IconFamily::Heroicons, Glyph::Image) => &HiPhotograph,
        (IconFamily::Heroicons, Glyph::Public) => &HiGlobeAlt,
        (IconFamily::Heroicons, Glyph::Reply) => &HiReply,
        (IconFamily::Heroicons, Glyph::Repeat) => &HiRefresh,
        (IconFamily::Heroicons, Glyph::Heart) => &HiHeart,
        (IconFamily::Heroicons, Glyph::More) => &HiDotsHorizontal,
        (IconFamily::Heroicons, Glyph::Back) => &HiArrowLeft,
        (IconFamily::Heroicons, Glyph::Close) => &HiX,
        (IconFamily::Heroicons, Glyph::Smile) => &HiEmojiHappy,
        (IconFamily::Ionicons, Glyph::Home) => &IoHomeOutline,
        (IconFamily::Ionicons, Glyph::Bell) => &IoNotificationsOutline,
        (IconFamily::Ionicons, Glyph::Search) => &IoSearchOutline,
        (IconFamily::Ionicons, Glyph::Write) => &IoCreateOutline,
        (IconFamily::Ionicons, Glyph::Image) => &IoImagesOutline,
        (IconFamily::Ionicons, Glyph::Public) => &IoGlobeOutline,
        (IconFamily::Ionicons, Glyph::Reply) => &IoChatbubbleOutline,
        (IconFamily::Ionicons, Glyph::Repeat) => &IoRepeatOutline,
        (IconFamily::Ionicons, Glyph::Heart) if selected => &IoHeart,
        (IconFamily::Ionicons, Glyph::Heart) => &IoHeartOutline,
        (IconFamily::Ionicons, Glyph::More) => &IoEllipsisHorizontal,
        (IconFamily::Ionicons, Glyph::Back) => &IoArrowBackOutline,
        (IconFamily::Ionicons, Glyph::Close) => &IoCloseOutline,
        (IconFamily::Ionicons, Glyph::Smile) => &IoHappyOutline,
        (_, Glyph::Home) => &BsHouse,
        (_, Glyph::Bell) => &BsBell,
        (_, Glyph::Search) => &BsSearch,
        (_, Glyph::Write) => &BsPencil,
        (_, Glyph::Image) => &BsImage,
        (_, Glyph::Public) => &BsGlobe,
        (_, Glyph::Reply) => &BsChat,
        (_, Glyph::Repeat) => &BsArrowRepeat,
        (_, Glyph::Heart) if selected => &BsHeartFill,
        (_, Glyph::Heart) => &BsHeart,
        (_, Glyph::More) => &BsThreeDots,
        (_, Glyph::Back) => &BsArrowLeft,
        (_, Glyph::Close) => &BsX,
        (_, Glyph::Smile) => &BsEmojiSmile,
        (_, Glyph::CatTower) => unreachable!("cat tower returns before pack mapping"),
    };
    let (fill, stroke, stroke_width) = shape.fill_and_stroke("currentColor");
    // The 0.10 adapter gives Ionicons outline paths a filled root. Restore their
    // outline semantics here; only the dots and selected heart are solid.
    let fill = if family == IconFamily::Ionicons
        && kind != Glyph::More
        && !(kind == Glyph::Heart && selected)
    {
        "none"
    } else {
        fill
    };
    rsx! { svg { class: "demo-icon", "data-icon-pack": pack, width: size, height: size,
        view_box: "{shape.view_box()}", fill, stroke, stroke_width,
        stroke_linecap: "{shape.stroke_linecap()}", stroke_linejoin: "{shape.stroke_linejoin()}", "aria-hidden": "true", "focusable": "false",
        {shape.child_elements()}
    } }
}
