use crate::directory::{api, health::Status};
use dioxus::prelude::*;

#[component]
pub(super) fn HealthHistory(domain: String) -> Element {
    let mut history = use_server_future(use_reactive((&domain,), |(domain,)| {
        api::server_health(domain)
    }))?;
    rsx! {
        section { class:"server-health", aria_label:"최근 응답 기록",
            h2 { "최근 응답 기록" }
            match history() {
                Some(Ok(data)) => rsx! {
                    if data.preview { p { class:"server-health-note", "가상의 응답 기록입니다." } }
                    p { class:"server-health-summary", "{data.summary()}" }
                    if !data.checks.is_empty() {
                        div { class:"server-health-strip", aria_hidden:"true",
                            for (index, check) in data.checks.iter().enumerate() {
                                span { key:"{index}", class:"server-health-tick {check.status().class()}", title:check.description() }
                            }
                        }
                        div { class:"server-health-axis", aria_hidden:"true", span { "이전" } span { "최근" } }
                        div { class:"server-health-legend",
                            for status in [Status::Responding, Status::Slow, Status::Unreachable] {
                                span { i { class:"server-health-key {status.class()}", aria_hidden:"true" } "{status.label()} {data.count(status)}회" }
                            }
                        }
                        p { class:"server-health-note", "한 칸은 한 번의 확인 · 느린 응답은 2초 이상" }
                        details {
                            summary { "확인 기록 보기 ({data.checks.len()}회)" }
                            p { class:"server-health-note", "확인 간격은 같지 않을 수 있어요. 시각은 한국 표준시(KST)입니다." }
                            div { class:"server-health-table", role:"region", aria_label:"응답 확인 기록", tabindex:"0",
                                table {
                                    thead { tr { th { scope:"col", "확인 시각 (KST)" } th { scope:"col", "응답" } th { scope:"col", "소요 시간" } } }
                                    tbody {
                                        for (index, check) in data.checks.iter().rev().enumerate() {
                                            tr { key:"{index}",
                                                td { time { datetime:check.checked_at.clone(), "{check.checked_at_kst}" } }
                                                td { "{check.status().label()}" }
                                                td { "{check.response()}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(_)) => rsx! {
                    p { role:"status", "응답 기록을 불러오지 못했어요." }
                    button { class:"text-link", onclick:move |_| history.restart(), "다시 불러오기" }
                },
                None => rsx! { p { role:"status", "응답 기록을 불러오는 중…" } },
            }
        }
    }
}
