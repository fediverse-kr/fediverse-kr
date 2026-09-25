use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};

#[component]
pub(super) fn LogoEditor(
    mut current: Signal<Software>,
    mut pending: Signal<bool>,
    mut generation: Signal<u32>,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut selected = use_signal(|| None::<(String, usize, String)>);
    let mut remove = use_signal(|| false);
    let mut note = use_signal(String::new);
    let mut notice = use_signal(String::new);
    let mut input_key = use_signal(|| 0u32);
    let data = current();
    rsx! {
        section { class: "membership-card membership-form", aria_label: "제품 로고 관리",
            h2 { "제품 로고" }
            if data.logo_available {
                crate::media_ui::ImageMark {
                    key: "{data.revision}", source: crate::media_ui::software_source(&data.name),
                    fallback: "로고", class: "software-mark", size: 64, decorative: false,
                }
            } else { p { "아직 등록된 로고가 없습니다." } }
            p { class: "membership-hint", "PNG·JPEG·WebP·SVG 한 개, 최대 500 KiB. 래스터는 가로·세로 4,096px 이하의 정지 이미지입니다. 원본 파일은 그대로 저장합니다." }
            form { class: "membership-form", onsubmit: move |event| {
                event.prevent_default();
                if !ready || pending() || (!remove() && selected().is_none()) { return; }
                let data = current();
                let request = LogoRequest { name: data.name, revision: data.revision, note: note(), data: if remove() { None } else { selected().map(|(_, _, data)| data) } };
                pending.set(true);
                spawn(async move {
                    let result = api::logo(request).await;
                    pending.set(false);
                    match result {
                        Ok(saved) => {
                            current.set(saved); selected.set(None); remove.set(false); note.set(String::new());
                            input_key += 1; generation += 1; notice.set("로고 설정을 저장했습니다.".into());
                        },
                        Err(e) => notice.set(message(&e)),
                    }
                });
            },
                div { class: "membership-field",
                    label { r#for: "catalog-logo-file", "새 로고 선택" }
                    input { key: "{input_key}", id: "catalog-logo-file", r#type: "file",
                        accept: ".png,.jpg,.jpeg,.webp,.svg", disabled: pending() || !ready,
                        onchange: move |event| {
                            if pending() || !ready { return; }
                            selected.set(None); remove.set(false); notice.set(String::new());
                            let files = event.files();
                            if files.len() != 1 { notice.set("로고 파일 한 개를 선택해 주세요.".into()); return; }
                            let file = files[0].clone();
                            if file.size() == 0 || file.size() > LOGO_MAX_BYTES as u64 {
                                notice.set("빈 파일은 사용할 수 없으며, 최대 크기는 500 KiB입니다.".into()); return;
                            }
                            pending.set(true);
                            spawn(async move {
                                match file.read_bytes().await {
                                    Ok(bytes) if !bytes.is_empty() && bytes.len() <= LOGO_MAX_BYTES => selected.set(Some((file.name(), bytes.len(), STANDARD.encode(&bytes)))),
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
                if data.logo_available {
                    label { class: "owner-checkbox",
                        input { r#type: "checkbox", checked: remove(), disabled: pending() || !ready,
                            onchange: move |e| remove.set(e.checked()),
                        }
                        "기존 로고를 제거합니다"
                    }
                }
                div { class: "membership-field",
                    label { r#for: "catalog-logo-note", "로고 관리 사유 · 관리자에게만 공개" }
                    textarea { id: "catalog-logo-note", rows: 2, maxlength: 1000, required: true, value: note,
                        disabled: pending() || !ready, oninput: move |e| note.set(e.value()),
                    }
                }
                p { class: "membership-hint", "저장에는 최근 15분 이내 인증이 필요합니다. 최신 버전 충돌 시 위의 ‘최신 설정 확인’에서 비교할 수 있습니다." }
                button { class: "primary-button", disabled: pending() || !ready || note().trim().is_empty() || (!remove() && selected().is_none()),
                    if remove() { "확인하고 로고 제거" } else { "확인하고 로고 저장" }
                }
            }
            if !notice().is_empty() { p { class: "membership-notice", role: "status", "{notice}" } }
        }
    }
}
