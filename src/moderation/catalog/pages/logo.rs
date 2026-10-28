use super::*;
use crate::directory::{MediaFit, SoftwareMediaMetadata};
use base64::{engine::general_purpose::STANDARD, Engine};

#[component]
pub(super) fn MediaEditor(
    mut current: Signal<Software>,
    mut pending: Signal<bool>,
    mut generation: Signal<u32>,
    role: MediaRole,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut selected = use_signal(|| None::<(String, usize, String)>);
    let mut remove = use_signal(|| false);
    let mut note = use_signal(String::new);
    let mut notice = use_signal(String::new);
    let mut input_key = use_signal(|| 0u32);
    let initial = current();
    let media = match role {
        MediaRole::Logo => initial.logo.clone(),
        MediaRole::Header => initial.header.clone(),
    };
    let mut source_url = use_signal(|| {
        media
            .as_ref()
            .and_then(|m| m.metadata.source_url.clone())
            .unwrap_or_default()
    });
    let mut attribution = use_signal(|| {
        media
            .as_ref()
            .map(|m| m.metadata.attribution.clone())
            .unwrap_or_default()
    });
    let mut license = use_signal(|| {
        media
            .as_ref()
            .map(|m| m.metadata.license.clone())
            .unwrap_or_default()
    });
    let mut license_url = use_signal(|| {
        media
            .as_ref()
            .and_then(|m| m.metadata.license_url.clone())
            .unwrap_or_default()
    });
    let mut caption = use_signal(|| {
        media
            .as_ref()
            .map(|m| m.metadata.caption.clone())
            .unwrap_or_default()
    });
    let mut fit = use_signal(|| media.as_ref().map(|m| m.metadata.fit).unwrap_or_default());
    let mut background = use_signal(|| {
        media
            .as_ref()
            .and_then(|m| m.metadata.background.clone())
            .unwrap_or_default()
    });
    let is_header = role == MediaRole::Header;
    let title = if is_header {
        "제품 헤더 이미지"
    } else {
        "제품 로고"
    };
    let file_label = if is_header {
        "새 헤더 이미지 선택"
    } else {
        "새 로고 선택"
    };
    let max_bytes = if is_header {
        HEADER_MAX_BYTES
    } else {
        LOGO_MAX_BYTES
    };
    let has_media = media.is_some();
    let max_text = if is_header {
        "PNG·JPEG·WebP·SVG 한 개, 최대 8 MiB. 래스터는 가로·세로 8,192px 이하, 32메가픽셀 이하입니다. 원본 파일은 그대로 저장합니다."
    } else {
        "PNG·JPEG·WebP·SVG 한 개, 최대 500 KiB. 래스터는 가로·세로 4,096px 이하의 정지 이미지입니다. 원본 파일은 그대로 저장합니다."
    };
    let file_id = if is_header {
        "catalog-header-file"
    } else {
        "catalog-logo-file"
    };
    let note_id = if is_header {
        "catalog-header-note"
    } else {
        "catalog-logo-note"
    };
    rsx! {
        section { class: "membership-card membership-form", aria_label: "{title} 관리",
            h2 { "{title}" }
            if let Some(media) = media.clone() {
                crate::media_ui::ImageMark {
                    key: "{initial.revision}", source: Some(media.url),
                    fallback: "{initial.display_name}", class: if is_header { "software-header-preview" } else { "software-mark" },
                    size: if is_header { 240 } else { 64 }, decorative: false,
                    style: media.metadata.background.as_ref().map(|color| format!("background-color:{color};")).unwrap_or_default(),
                    image_fit: Some(if media.metadata.fit == MediaFit::Cover { "cover" } else { "contain" }.to_owned()),
                }
            } else { p { "아직 등록된 이미지가 없습니다." } }
            p { class: "membership-hint", "{max_text}" }
            form { class: "membership-form", onsubmit: move |event| {
                event.prevent_default();
                if !ready || pending() || (!remove() && selected().is_none() && !has_media) { return; }
                let data = current();
                let metadata = if remove() { None } else { Some(SoftwareMediaMetadata {
                    source_url: (!source_url().trim().is_empty()).then(|| source_url().trim().to_owned()),
                    attribution: attribution().trim().to_owned(), license: license().trim().to_owned(),
                    license_url: (!license_url().trim().is_empty()).then(|| license_url().trim().to_owned()),
                    caption: caption().trim().to_owned(), fit: fit(),
                    background: (!background().trim().is_empty()).then(|| background().trim().to_owned()),
                })};
                let request = MediaRequest { name: data.name, revision: data.revision, role, note: note(), data: if remove() { None } else { selected().map(|(_, _, data)| data) }, metadata };
                pending.set(true);
                spawn(async move {
                    let result = api::media(request).await;
                    pending.set(false);
                    match result {
                        Ok(saved) => {
                            current.set(saved); selected.set(None); remove.set(false); note.set(String::new());
                            input_key += 1; generation += 1; notice.set("미디어 설정을 저장했습니다.".into());
                        },
                        Err(e) => notice.set(message(&e)),
                    }
                });
            },
                div { class: "membership-field",
                    label { r#for: file_id, "{file_label}" }
                    input { key: "{input_key}", id: file_id, r#type: "file",
                        accept: ".png,.jpg,.jpeg,.webp,.svg", disabled: pending() || !ready,
                        onchange: move |event| {
                            if pending() || !ready { return; }
                            selected.set(None); remove.set(false); notice.set(String::new());
                            let files = event.files();
                            if files.len() != 1 { notice.set("파일 한 개를 선택해 주세요.".into()); return; }
                            let file = files[0].clone();
                            if file.size() == 0 || file.size() > max_bytes as u64 {
                                notice.set(if is_header { "빈 파일은 사용할 수 없으며, 최대 크기는 8 MiB입니다." } else { "빈 파일은 사용할 수 없으며, 최대 크기는 500 KiB입니다." }.into()); return;
                            }
                            pending.set(true);
                            spawn(async move {
                                match file.read_bytes().await {
                                    Ok(bytes) if !bytes.is_empty() && bytes.len() <= max_bytes => selected.set(Some((file.name(), bytes.len(), STANDARD.encode(&bytes)))),
                                    _ => notice.set("파일을 읽지 못했습니다. 크기를 확인하고 다시 선택해 주세요.".into()),
                                }
                                pending.set(false);
                            });
                        },
                    }
                }
                if let Some((name, bytes, _)) = selected() {
                    p { class: "membership-hint", "선택: {name} · {bytes}바이트. 저장 전에는 공개되지 않습니다." }
                }
                fieldset { class: "software-media-metadata",
                    legend { "공개 출처와 표시" }
                    label { "출처 URL", input { value: source_url, maxlength: 2048, disabled: pending() || !ready, oninput: move |e| source_url.set(e.value()) } }
                    label { "제작자·귀속", input { value: attribution, maxlength: 500, required: !remove(), disabled: pending() || !ready, oninput: move |e| attribution.set(e.value()) } }
                    label { "라이선스", input { value: license, maxlength: 2000, required: !remove(), disabled: pending() || !ready, oninput: move |e| license.set(e.value()) } }
                    label { "라이선스 URL", input { value: license_url, maxlength: 2048, disabled: pending() || !ready, oninput: move |e| license_url.set(e.value()) } }
                    label { "캡션", input { value: caption, maxlength: 1000, disabled: pending() || !ready, oninput: move |e| caption.set(e.value()) } }
                    label { "맞춤", select { value: if fit() == MediaFit::Cover { "cover" } else { "contain" }, disabled: pending() || !ready, oninput: move |e| fit.set(if e.value() == "cover" { MediaFit::Cover } else { MediaFit::Contain }), option { value: "contain", "Contain" } option { value: "cover", "Cover" } } }
                    label { "배경색", input { value: background, maxlength: 7, placeholder: "#RRGGBB", disabled: pending() || !ready, oninput: move |e| background.set(e.value()) } }
                }
                if media.is_some() {
                    label { class: "owner-checkbox",
                        input { r#type: "checkbox", checked: remove(), disabled: pending() || !ready, onchange: move |e| remove.set(e.checked()) }
                        "기존 이미지를 제거합니다"
                    }
                }
                div { class: "membership-field",
                    label { r#for: note_id, "관리 사유 · 관리자에게만 공개" }
                    textarea { id: note_id, rows: 2, maxlength: 1000, required: true, value: note, disabled: pending() || !ready, oninput: move |e| note.set(e.value()) }
                }
                p { class: "membership-hint", "저장에는 최근 15분 이내 인증이 필요합니다. 최신 버전 충돌 시 위의 ‘최신 설정 확인’에서 비교할 수 있습니다." }
                button { class: "primary-button", disabled: pending() || !ready || note().trim().is_empty() || (!remove() && ((selected().is_none() && !has_media) || attribution().trim().is_empty() || license().trim().is_empty())),
                    if remove() { "확인하고 이미지 제거" } else { "확인하고 이미지 저장" }
                }
            }
            if !notice().is_empty() { p { class: "membership-notice", role: "status", "{notice}" } }
        }
    }
}
