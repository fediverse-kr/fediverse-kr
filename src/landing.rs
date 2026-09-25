use crate::social_demo::*;
use crate::typing::typed_text;
use dioxus::prelude::*;
use lucide_dioxus::{ArrowLeft, ArrowRight, Pause, Play};

const HEADLINES: [[&str; 3]; 4] = [
    ["다른 서버의", "내 친구를", "구독해요."],
    ["난 이 서버", "넌 저 서버", "우리 맞팔하자"],
    ["서버는 달라도", "우리 이야기는", "이어져요."],
    ["내가 고른 곳", "다른 곳 친구도", "만나요."],
];
const HEADLINE_HOLD_SECONDS: u16 = 20;
const HEADLINE_FADE_MS: u32 = 700;

fn next_headline(current: usize) -> usize {
    (current + 1) % HEADLINES.len()
}
const SCENES: [&str; 3] = [
    "처음 만난 이웃",
    "사진으로 이어지는 대화",
    "블로그에서 시작된 이야기",
];
const CAPTIONS: [[&str; 4]; 3] = [
    [
        "새로 가입한 슈나가 첫 인사를 써요",
        "첫 인사가 다른 서버에도 보여요",
        "다른 서버의 페트리샤도 답글을 써요",
        "슈나의 서버에도 환영 인사가 도착해요",
    ],
    [
        "사진을 고르고 함께 올릴 글을 써요",
        "구독한 사진이 내 타임라인에 보여요",
        "슈나는 쓰던 앱에서 답글을 써요",
        "사진 앱에도 슈나의 답글이 도착해요",
    ],
    [
        "곰국이 블로그에 새로운 글을 써요",
        "구독한 블로그 글이 타임라인에 보여요",
        "슈나는 쓰던 SNS에서 감상을 남겨요",
        "블로그에도 슈나의 답글이 도착해요",
    ],
];

// Four story beats; typing, clicks and sending happen within them, not as extra steps.
const STEP_STARTS: [u16; 5] = [0, 100, 170, 270, 340];
const TOTAL_TICKS: u16 = STEP_STARTS[4];
const COMPOSE_OPEN: u16 = 10;
const SUBMIT_CLICK: u16 = 62;
const SENDING: u16 = 70;
const PUBLISHED: u16 = 80;
const ORIGINAL_RECEIVED: u16 = STEP_STARTS[1] + 10;
const LOCAL_REPLY: u16 = 140;
const REPLY_CLICK: u16 = STEP_STARTS[2];
const REPLY_OPEN: u16 = REPLY_CLICK + COMPOSE_OPEN;
const REPLY_PUBLISHED: u16 = REPLY_CLICK + PUBLISHED;
const REPLY_RECEIVED: u16 = STEP_STARTS[3] + 10;
const SCENE_FADE_OUT_TICKS: u8 = 3;
const SCENE_FADE_IN_TICKS: u8 = 5;

#[derive(Clone, Copy, PartialEq, Debug)]
enum SceneFade {
    Idle,
    Out(u8),
    In(u8),
}

impl SceneFade {
    fn phase(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Out(_) => "out",
            Self::In(_) => "in",
        }
    }

    fn advance(self) -> Self {
        match self {
            Self::Out(tick) if tick + 1 < SCENE_FADE_OUT_TICKS => Self::Out(tick + 1),
            Self::Out(_) => Self::In(0),
            Self::In(tick) if tick + 1 < SCENE_FADE_IN_TICKS => Self::In(tick + 1),
            _ => Self::Idle,
        }
    }
}

fn active_side(scene: usize, tick: u16) -> bool {
    let origin_is_remote = scene != 0;
    match story_step(tick) {
        0 | 3 => origin_is_remote,
        _ => !origin_is_remote,
    }
}

fn original_highlight(tick: u16) -> bool {
    story_step(tick) <= 1
}
fn reply_highlight(tick: u16) -> bool {
    story_step(tick) >= 2
}

fn story_step(tick: u16) -> usize {
    STEP_STARTS[1..4]
        .iter()
        .filter(|&&start| tick >= start)
        .count()
}

fn segment_progress(tick: u16, step: usize) -> f32 {
    let duration = STEP_STARTS[step + 1] - STEP_STARTS[step];
    f32::from(tick.saturating_sub(STEP_STARTS[step]).min(duration)) / f32::from(duration) * 100.0
}

fn step_playback_tick(step: usize) -> u16 {
    STEP_STARTS[step]
}

#[component]
pub fn RandomHeadline() -> Element {
    // Keep the first SSR/client render identical; this clock never reads demo state.
    #[allow(unused_mut)]
    let mut selected = use_signal(|| 0usize);
    #[allow(unused_mut)]
    let mut previous = use_signal(|| None::<usize>);
    #[cfg(target_arch = "wasm32")]
    use_future(move || async move {
        if let Ok(index) = document::eval(&format!(
            "return Math.floor(Math.random() * {});",
            HEADLINES.len()
        ))
        .join::<usize>()
        .await
        {
            selected.set(index.min(HEADLINES.len() - 1));
        }
        let mut held_seconds = 0;
        loop {
            // Count only visible time. Returning to a tab gives the current
            // phrase a fresh reading interval without suppressing the demo.
            let visible = document::eval("return !document.hidden;")
                .join::<bool>()
                .await
                .unwrap_or(true);
            if !visible {
                held_seconds = 0;
            } else if held_seconds >= HEADLINE_HOLD_SECONDS {
                previous.set(Some(selected()));
                selected.set(next_headline(selected()));
                gloo_timers::future::TimeoutFuture::new(HEADLINE_FADE_MS).await;
                previous.set(None);
                held_seconds = 0;
            }
            gloo_timers::future::TimeoutFuture::new(1000).await;
            held_seconds += 1;
        }
    });
    let lines = HEADLINES[selected()];
    rsx! {
        div { class: "headline-rotation",
            style: "--headline-fade-duration: {HEADLINE_FADE_MS}ms",
            h1 { class: "landing-headline", "data-headline": selected(), aria_label: lines.join(" "),
                if let Some(index) = previous() {
                    span { class: "headline-frame headline-outgoing", aria_hidden: "true",
                        for (line_index, line) in HEADLINES[index].iter().enumerate() {
                            span { class: if line_index == 2 { "headline-line headline-accent" } else { "headline-line" }, "{line}" }
                        }
                    }
                }
                span { class: "headline-frame headline-current", "data-entering": previous().is_some(),
                    for (index, line) in lines.iter().enumerate() {
                        span { class: if index == 2 { "headline-line headline-accent" } else { "headline-line" }, "{line}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn LandingDemo() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut scene = use_signal(|| 0usize);
    let mut pending_scene = use_signal(|| None::<usize>);
    let mut pending_tick = use_signal(|| 0u16);
    #[allow(unused_mut)]
    let mut scene_fade = use_signal(|| SceneFade::Idle);
    let mut side_override = use_signal(|| None::<bool>);
    let mut tick = use_signal(|| 0u16);
    #[allow(unused_mut)]
    let mut playback_id = use_signal(|| 0u32);
    let mut paused = use_signal(|| false);
    #[cfg(target_arch = "wasm32")]
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(100).await;
            // Complete scene fades even when playback is paused. The content
            // clock waits until the new windows are fully readable.
            if scene_fade() != SceneFade::Idle {
                let next = scene_fade().advance();
                if next == SceneFade::In(0) {
                    if let Some(target) = pending_scene() {
                        scene.set(target);
                        tick.set(pending_tick());
                        playback_id += 1;
                        side_override.set(None);
                        pending_scene.set(None);
                    }
                }
                scene_fade.set(next);
                continue;
            }
            if pending_scene().is_some() {
                scene_fade.set(SceneFade::Out(0));
                continue;
            }
            if !paused() {
                if tick() < TOTAL_TICKS {
                    tick += 1;
                } else {
                    pending_tick.set(0);
                    pending_scene.set(Some((scene() + 1) % SCENES.len()));
                }
            }
        }
    });
    let elapsed = tick();
    let finished = elapsed == TOTAL_TICKS;
    let progress = f32::from(elapsed) / f32::from(TOTAL_TICKS) * 100.0;
    let step = story_step(elapsed);
    let side = if paused() {
        side_override().unwrap_or(active_side(scene(), elapsed))
    } else {
        active_side(scene(), elapsed)
    };
    let caption = CAPTIONS[scene()][step];
    rsx! {
        section { class: "landing-demo", "data-paused": paused(), "data-scene": scene(), "data-tick": elapsed, "data-step": step, "data-complete": finished, "data-scene-transition": scene_fade().phase(),
            style: "--scene-fade-out: {u16::from(SCENE_FADE_OUT_TICKS) * 100}ms; --scene-fade-in: {u16::from(SCENE_FADE_IN_TICKS) * 100}ms",
            aria_label: "연합우주 이용 장면",
            div { class: "landing-site-switch", role: "group", aria_label: "모바일에서 볼 사이트",
                button { disabled: !ready, aria_pressed: !side, onclick: move |_| { side_override.set(Some(false)); paused.set(true); }, "{SITES[0].name}" }
                button { disabled: !ready, aria_pressed: side, onclick: move |_| { side_override.set(Some(true)); paused.set(true); }, "{SITES[scene() + 1].name}" }
            }
            div { class: "landing-windows",
                for remote in [false, true] {
                    LandingWindow { key: "{scene()}-{remote}-{playback_id()}", remote, scene: scene(), tick: elapsed, visible: side == remote, on_interact: move |_| { side_override.set(Some(remote)); paused.set(true); } }
                }
            }
            div { class: "landing-step", role: "status", aria_atomic: "true",
                p { key: "{scene()}-{step}", "{caption}" }
            }
            div { class: "landing-timeline", role: "group", aria_label: "재생 단계 선택",
                for index in 0..4 {
                    button { class: "timeline-segment", r#type: "button", disabled: !ready,
                        "data-active": index == step, "data-done": elapsed >= STEP_STARTS[index + 1],
                        aria_current: if index == step { "step" } else { "false" },
                        aria_label: format!("{}단계부터 재생: {}", index + 1, CAPTIONS[scene()][index]),
                        title: "{index + 1}단계: {CAPTIONS[scene()][index]}",
                        onclick: move |_| {
                            pending_tick.set(step_playback_tick(index));
                            pending_scene.set(Some(scene()));
                            paused.set(false);
                        },
                        span { key: "{playback_id()}", class: "timeline-track", aria_hidden: "true", span { style: "width: {segment_progress(elapsed, index)}%" } }
                    }
                }
            }
            div { class: "sr-only", role: "progressbar", aria_label: "현재 이용 장면 재생 진행률", aria_valuemin: "0", aria_valuemax: "100", aria_valuenow: "{progress:.0}", aria_valuetext: "4단계 중 {step + 1}단계: {caption}" }
            div { class: "landing-demo-controls",
                div { class: "landing-scene-name", span { class: "scene-count", "0{scene() + 1} / 0{SCENES.len()}" } span { "{SCENES[scene()]}" } }
                div { class: "landing-playback",
                    button { disabled: !ready, aria_label: if paused() { "애니메이션 재생" } else { "애니메이션 일시 정지" }, onclick: move |_| { side_override.set(None); paused.set(!paused()); },
                        if paused() { Play { size: 16 } } else { Pause { size: 16 } }
                    }
                    button { disabled: !ready, aria_label: "이전 이용 장면", onclick: move |_| { pending_tick.set(0); pending_scene.set(Some((pending_scene().unwrap_or(scene()) + SCENES.len() - 1) % SCENES.len())); paused.set(false); }, ArrowLeft { size: 19 } }
                    button { disabled: !ready, aria_label: "다음 이용 장면", onclick: move |_| { pending_tick.set(0); pending_scene.set(Some((pending_scene().unwrap_or(scene()) + 1) % SCENES.len())); paused.set(false); }, ArrowRight { size: 19 } }
                }
            }
        }
    }
}

#[component]
fn LandingWindow(
    remote: bool,
    scene: usize,
    tick: u16,
    visible: bool,
    on_interact: EventHandler,
) -> Element {
    let photo_site = remote && scene == 1;
    let blog_site = remote && scene == 2;
    let site = Site::ALL[if remote { scene + 1 } else { 0 }];
    let account = if remote { scene + 2 } else { 0 };
    let origin_author = if scene == 0 { 0 } else { scene + 2 };
    let reply_author = if scene == 0 { 2 } else { 0 };
    let source = if scene == 0 { !remote } else { remote };
    let step = story_step(tick);
    let mut manual_compose = use_signal(|| false);
    let mut manual_posts = use_signal(Vec::<(String, bool)>::new);
    let mut dismissed = use_signal(|| false);
    let original = if source {
        tick >= PUBLISHED
    } else {
        tick >= ORIGINAL_RECEIVED
    };
    let reply = if source {
        tick >= REPLY_RECEIVED
    } else {
        tick >= REPLY_PUBLISHED
    };
    let composing = !dismissed()
        && ((source && (COMPOSE_OPEN..PUBLISHED).contains(&tick))
            || (!source && (REPLY_OPEN..REPLY_PUBLISHED).contains(&tick)));
    let compose_tick = if source {
        tick
    } else {
        tick.saturating_sub(REPLY_CLICK)
    };
    let reply_body = match scene {
        0 => "멍멍이가 또 하나 늘었네용",
        1 => "산책길에 이런 이웃이! 🐾",
        _ => "내일은 저도 천천히 걸어볼래요.",
    };
    let original_body = match scene {
        0 => "안녕하세요 처음이에요",
        1 => "산책하다 만난 이웃. 잠깐 쉬어 가요.",
        _ => "조금 느리게 걸으면 보이는 것들.",
    };
    rsx! {
        SiteWindow { site, viewer: Person::ALL[account], visible, interactive: true,
            journal_status: if tick < PUBLISHED { "글 / 편집기" } else { "글 / 발행됨" },
            compose_hint: source && (2..COMPOSE_OPEN).contains(&tick),
            on_compose: move |_| { on_interact.call(()); manual_compose.set(true); },
            overlays: rsx! {
                if manual_compose() {
                    DemoComposer { key: "manual-compose-{remote}", elapsed: 0, author: account, message: "", interactive: true, photo: photo_site,
                        on_close: move |_| manual_compose.set(false),
                        on_submit: move |body| { manual_posts.write().push((body, photo_site)); manual_compose.set(false); dismissed.set(true); },
                    }
                } else if composing && !blog_site {
                    DemoComposer { key: "compose-{source}", elapsed: compose_tick,
                        author: account, recipient: origin_author,
                        message: if source { original_body } else { reply_body },
                        reply: !source, quote: original_body, photo: photo_site && source,
                        on_close: move |_| { on_interact.call(()); dismissed.set(true); },
                    }
                }
                if (PUBLISHED..STEP_STARTS[1]).contains(&tick) && source || (REPLY_PUBLISHED..STEP_STARTS[3]).contains(&tick) && !source {
                    div { class: "social-toast", role: "status", "✓ 게시했습니다" }
                }

            },
                    if blog_site { JournalDemo { tick } }
                    else {
                        for (index, (body, photo)) in manual_posts().iter().enumerate().rev() { SocialPost { key: "manual-{index}", author: account, body: body.clone(), photo: *photo, photo_site: *photo && photo_site } }
                        if reply && !photo_site { Arrival { SocialPost { author: reply_author, body: reply_body,
                            reply_to: USERS[origin_author].name, remote_arrival: source, highlighted: reply_highlight(tick), compact: true,
                        } } }
                        if scene == 0 && tick >= LOCAL_REPLY { Arrival { SocialPost { author: 1, body: "오 반가워요!", reply_to: "슈나", compact: true } } }
                        if original {
                            SocialPost { author: origin_author, body: original_body,
                                photo: scene == 1, photo_site, article_preview: scene == 2,
                                remote_arrival: !source, fresh: source, highlighted: original_highlight(tick), reply_count: if reply { if scene == 0 { 2 } else { 1 } } else if scene == 0 && tick >= LOCAL_REPLY { 1 } else { 0 },
                                reply_selected: !source && (REPLY_CLICK..REPLY_OPEN).contains(&tick),
                            }
                        }
                        if reply && photo_site { Arrival { div { class: "photo-comment", "data-incoming": "true", "data-highlighted": step == 3,
                            onmounted: move |_| { reveal_photo_comment(); },
                            Avatar { author: 0 } div { strong { "{USERS[0].name}" } small { "{USERS[0].address()}" } p { "{reply_body}" } }
                        } } }
                        if !photo_site {
                            SocialPost { author: if remote { 2 } else { 1 }, body: if remote { "오늘도 창가 자리는 제 차지예요. ☀️" } else { "오늘 산책은 조금 멀리 다녀왔어요." }, old: true }
                            SocialPost { author: if remote { 1 } else { scene + 2 }, body: "선선한 바람이 부네요. 좋은 하루 보내세요!", old: true }
                        }
                    }

        }
    }
}

fn reveal_photo_comment() {
    #[cfg(target_arch = "wasm32")]
    spawn(async move {
        // Scroll only this app's feed, never the landing page. Wait for the insertion to settle.
        gloo_timers::future::TimeoutFuture::new(900).await;
        let _ = document::eval(r#"
            const comment = document.querySelector('.photo-comment');
            const feed = comment?.closest('.social-feed');
            if (feed && comment) {
                const bottom = comment.getBoundingClientRect().bottom - feed.getBoundingClientRect().bottom;
                if (bottom > 0) feed.scrollTo({top: feed.scrollTop + bottom + 12,
                    behavior: 'smooth'});
            }
        "#).join::<()>().await;
    });
}

#[component]
fn JournalDemo(tick: u16) -> Element {
    let published = tick >= PUBLISHED;
    let title = "산책의 속도로 사는 하루";
    let body = "늘 지나치던 골목에서 고양이를 만났어요. 오늘은 잠깐 멈춰 보기로 했습니다.";
    let draft = typed_text(body, tick.saturating_sub(COMPOSE_OPEN), 42);
    rsx! {
        if !published {
            JournalEditor { title, text: draft, saved: tick >= 32, sending: tick >= SENDING, submit_hint: tick >= SUBMIT_CLICK, caret: tick >= COMPOSE_OPEN && tick < SENDING }
        } else { JournalArticle { title, body, highlighted: original_highlight(tick), replies: if tick >= REPLY_RECEIVED { 1 } else { 0 },
            if tick >= REPLY_RECEIVED { Arrival { JournalComment { author: 0, body: "내일은 저도 천천히 걸어볼래요.", highlighted: reply_highlight(tick) } } }
        } }
    }
}

#[component]
fn DemoComposer(
    elapsed: u16,
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
    rsx! { Composer {
        author, recipient, reply, quote, photo, interactive, on_close, on_submit,
        message: typed_text(&message, elapsed.saturating_sub(if photo { 23 } else { COMPOSE_OPEN }), if photo { 29 } else { 42 }),
        sending: !interactive && elapsed >= SENDING,
        submit_hint: !interactive && elapsed >= SUBMIT_CLICK,
        photo_hint: !interactive && elapsed >= 18,
        photo_selected: elapsed >= 28,
    } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headlines_are_three_nonempty_lines() {
        assert!(
            HEADLINES
                .iter()
                .all(|lines| lines.iter().all(|line| !line.is_empty()))
        );
        assert_eq!(HEADLINES[0], ["다른 서버의", "내 친구를", "구독해요."]);
    }
    #[test]
    fn scene_fade_replaces_content_once_then_finishes_before_playback() {
        let mut fade = SceneFade::Out(0);
        let mut replacements = 0;
        for step in 1..=SCENE_FADE_OUT_TICKS + SCENE_FADE_IN_TICKS {
            fade = fade.advance();
            if fade == SceneFade::In(0) {
                replacements += 1;
                assert_eq!(step, SCENE_FADE_OUT_TICKS);
            }
            if step < SCENE_FADE_OUT_TICKS {
                assert_eq!(fade.phase(), "out");
            } else if step < SCENE_FADE_OUT_TICKS + SCENE_FADE_IN_TICKS {
                assert_eq!(fade.phase(), "in");
            }
        }
        assert_eq!(fade, SceneFade::Idle);
        assert_eq!(replacements, 1);
    }
    #[test]
    fn headlines_rotate_without_repeating_before_a_full_cycle() {
        assert!(HEADLINE_HOLD_SECONDS >= 20);
        for start in 0..HEADLINES.len() {
            let mut current = start;
            let mut visited = vec![false; HEADLINES.len()];
            for _ in 0..HEADLINES.len() {
                assert!(!visited[current]);
                visited[current] = true;
                current = next_headline(current);
            }
            assert_eq!(current, start);
        }
    }
    #[test]
    fn progress_clock_covers_the_whole_story() {
        assert_eq!(STEP_STARTS[4], TOTAL_TICKS);
        assert_eq!(u32::from(TOTAL_TICKS) * 100, 34_000);
        for step in 0..4 {
            assert!(STEP_STARTS[step + 1] > STEP_STARTS[step]);
            assert_eq!(segment_progress(STEP_STARTS[step], step), 0.0);
            assert_eq!(segment_progress(STEP_STARTS[step + 1], step), 100.0);
        }
    }
    #[test]
    fn seeking_starts_the_selected_beat_even_with_system_reduced_motion() {
        for step in 0..4 {
            let tick = step_playback_tick(step);
            assert_eq!(tick, STEP_STARTS[step]);
            assert_eq!(story_step(tick), step);
        }
    }
    #[test]
    fn three_scenes_have_short_ordered_captions() {
        assert_eq!(SCENES.len(), 3);
        for captions in &CAPTIONS {
            for caption in captions {
                assert!(
                    (16..=32).contains(&caption.chars().count()),
                    "{caption}: {}",
                    caption.chars().count()
                );
            }
        }
        assert_eq!(story_step(0), 0);
        assert_eq!(story_step(TOTAL_TICKS), 3);
        for tick in 1..=TOTAL_TICKS {
            assert!(story_step(tick) >= story_step(tick - 1));
        }
    }
    #[test]
    fn draft_starts_empty_and_finishes_before_send() {
        let text = "안녕하세요 처음이에요";
        assert_eq!(typed_text(text, 0, 42), "");
        assert_eq!(typed_text(text, 5, 42), "");
        assert!(!typed_text(text, 20, 42).is_empty());
        assert_ne!(typed_text(text, 20, 42), text);
        assert_eq!(typed_text(text, 47, 42), text);
        assert_eq!(typed_text(text, 49, 42), text);
    }
    #[test]
    fn mobile_follows_the_actor_then_the_recipient_in_every_scene() {
        for scene in 0..SCENES.len() {
            let source = scene != 0;
            for tick in 0..=TOTAL_TICKS {
                let expected = if tick < STEP_STARTS[1] || tick >= STEP_STARTS[3] {
                    source
                } else {
                    !source
                };
                assert_eq!(
                    active_side(scene, tick),
                    expected,
                    "scene {scene}, tick {tick}"
                );
            }
        }
    }
    #[test]
    fn receipts_hold_pair_highlights_until_the_next_beat() {
        for tick in ORIGINAL_RECEIVED..STEP_STARTS[2] {
            assert!(original_highlight(tick));
            assert!(!reply_highlight(tick));
        }
        for tick in REPLY_RECEIVED..=TOTAL_TICKS {
            assert!(reply_highlight(tick));
            assert!(!original_highlight(tick));
        }
    }
    #[test]
    fn actions_have_time_to_settle_before_the_next_action() {
        assert!(STEP_STARTS[1] - PUBLISHED >= 20);
        assert!(STEP_STARTS[3] - REPLY_PUBLISHED >= 20);
        assert!(ORIGINAL_RECEIVED - STEP_STARTS[1] >= 10);
        assert!(REPLY_RECEIVED - STEP_STARTS[3] >= 10);
        assert!(LOCAL_REPLY - ORIGINAL_RECEIVED >= 30);
        assert!(REPLY_CLICK - LOCAL_REPLY >= 30);
        assert!(TOTAL_TICKS - REPLY_RECEIVED >= 50);
        assert!(SUBMIT_CLICK < SENDING && SENDING < PUBLISHED);
        assert_eq!(
            typed_text("사진을 올려요", SUBMIT_CLICK - 23, 29),
            "사진을 올려요"
        );
    }
    #[test]
    fn phoenix_identities_are_distinct_and_consistent() {
        assert_eq!(
            SITES.map(|site| site.name),
            ["멍멍.개집", "냥냥.타워", "추억.사진", "곰국.블로그"]
        );
        for scene in 0..SCENES.len() {
            assert_eq!(USERS[scene + 2].site, scene + 1);
        }
        assert_eq!(USERS[3].address(), "@film@photo.town");
        assert_eq!(USERS[4].address(), "@soup@blog.town");
    }
}
