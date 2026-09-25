use super::{api, JobIssue, JobIssueKind, SiteIssue, WorkerOverview};
use crate::{
    moderation::{
        layout::{BackofficePage, BackofficeSection},
        pages::{message, timestamp},
    },
    Route,
};
use dioxus::prelude::*;

const DISPLAY_LIMIT: usize = 10;

#[component]
pub(crate) fn OperationsNav(#[props(default)] profiles: bool) -> Element {
    rsx! {nav{class:"operations-subnav",aria_label:"작업·유지보수 메뉴",
        Link{class:"text-link",aria_current:(!profiles).then_some("page"),to:Route::ModerationWorkers{},"작업 현황"}
        Link{class:"text-link",aria_current:profiles.then_some("page"),to:Route::ModerationProfiles{},"프로필 유지보수"}
    }}
}

fn job_kind_label(kind: JobIssueKind) -> &'static str {
    match kind {
        JobIssueKind::LeaseExpired => "이전 작업 회수 대기",
        JobIssueKind::Dead => "작업 중단",
    }
}

fn job_kind_hint(kind: JobIssueKind) -> &'static str {
    match kind {
        JobIssueKind::LeaseExpired => "자동 시도는 최대 3회입니다.",
        JobIssueKind::Dead => "자동 재시도 한도에 도달했습니다.",
    }
}

fn site_issue_label(issue: &SiteIssue) -> &'static str {
    match (issue.alive, issue.nodeinfo_failed) {
        (false, true) => "상대 서버 응답·NodeInfo 확인 실패",
        (false, false) => "상대 서버 응답 실패",
        (true, true) => "NodeInfo 확인 실패",
        (true, false) => "상대 서버 확인 지연",
    }
}

#[component]
pub fn ModerationWorkers() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut fetched = use_server_future(api::overview)?;
    #[cfg(target_arch = "wasm32")]
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(15_000).await;
            if fetched.peek().as_ref().is_some_and(|result| result.is_ok()) {
                fetched.restart();
            }
        }
    });

    rsx! {
        document::Title { "작업 현황 — fediverse.kr" }
        document::Stylesheet { href: asset!("/assets/styling/backoffice-operations.css") }
        BackofficePage {
            section: BackofficeSection::Operations,
            title: "작업 현황".to_string(),
            description: "수집 대기열과 상대 서버 확인 결과를 분리해, 확인할 대상만 안전하게 살펴봅니다.".to_string(),
            OperationsNav {}
            div { class: "worker-monitor",
                p { class: "backoffice-state worker-snapshot-note",
                    "현재 스냅샷입니다. DB에 기록된 실행 중 상태는 워커의 실제 응답을 증명하지 않습니다."
                }
                p { class: "membership-hint",
                    "정상적으로 읽힌 상태일 때만 15초 뒤 다시 확인합니다. 갱신 실패는 성공한 상태로 바꾸지 않습니다."
                }
                match fetched() {
                    Some(Ok(data)) => rsx! { WorkerSnapshot { data } },
                    Some(Err(error)) => rsx! {
                        p { class: "membership-message membership-error", role: "alert", "{message(&error)}" }
                        Link { class: "text-link", to: Route::Login {}, "관리자 계정에서 다시 인증" }
                    },
                    None => rsx! { p { role: "status", "수집 현황을 불러오고 있어요." } },
                }
                button {
                    class: "secondary-button worker-refresh",
                    disabled: !ready || fetched().is_none(),
                    aria_label: "수집 현황 새로고침",
                    onclick: move |_| fetched.restart(),
                    "수집 현황 새로고침"
                }
            }
        }
    }
}

#[component]
fn WorkerSnapshot(data: WorkerOverview) -> Element {
    let WorkerOverview {
        captured_at,
        queue,
        job_issues_total,
        job_issues,
        site_issues_total,
        site_issues,
    } = data;
    rsx! {
        section { class: "backoffice-panel worker-summary", aria_label: "수집 작업 요약",
            h2 { "대기열 스냅샷" }
            p { class: "membership-hint", "완료는 수집 시도가 끝났다는 뜻입니다. 상대 서버가 응답하지 않아도 완료될 수 있습니다." }
            dl { class: "backoffice-operation-facts",
                div { dt { "실행 대기" } dd { "{queue.ready}건" } }
                div { dt { "예약" } dd { "{queue.scheduled}건" } }
                div { dt { "종료 서버 보류" } dd { "{queue.paused}건" } }
                div { dt { "실행 중" } dd { "{queue.running}건" } }
                div { dt { "회수 대기" } dd { "{queue.expired}건" } }
                div { dt { "중단" } dd { "{queue.dead}건" } }
                div { dt { "최근 24시간 완료" } dd { "{queue.completed_24h}건" } }
                div { class: "worker-time-fact", dt { "마지막 완료" } dd {
                    if let Some(value) = queue.latest_completion.as_deref() {
                        time { datetime: value, "{timestamp(value)}" }
                    } else {
                        "아직 없음"
                    }
                } }
                div { class: "worker-time-fact", dt { "조회 시각" }
                    dd { time { datetime: captured_at.clone(), "{timestamp(&captured_at)}" } }
                }
            }
        }
        JobIssues { issues: job_issues, total: job_issues_total }
        SiteIssues { issues: site_issues, total: site_issues_total }
    }
}

#[component]
fn JobIssues(issues: Vec<JobIssue>, total: i64) -> Element {
    let displayed = issues.len().min(DISPLAY_LIMIT);
    rsx! {
        section { class: "backoffice-panel worker-issues", aria_label: "작업 확인 필요 항목",
            h2 { "작업 확인 필요" }
            p { class: "membership-hint",
                "중단 기록은 이후 수집이 성공해도 남습니다. 개별 재수집은 서버 관리에서 요청할 수 있습니다."
            }
            p { class: "worker-issue-count", "전체 {total}건 · {displayed}건 표시" }
            if issues.is_empty() {
                p { class: "membership-notice", "확인이 필요한 작업 항목이 없습니다." }
            } else {
                ul { class: "worker-issue-list",
                    for issue in issues.into_iter().take(DISPLAY_LIMIT) {
                        li { key: "{issue.site_id}-{job_kind_label(issue.kind)}-{issue.scheduled_at}",
                            h3 { class: "worker-domain", "{issue.domain}" }
                            p { class: "worker-issue-meta",
                                strong { "{job_kind_label(issue.kind)}" }
                                " · {job_kind_hint(issue.kind)}"
                                " · 시도 {issue.attempts}회 · "
                                time { datetime: issue.scheduled_at.clone(), "{timestamp(&issue.scheduled_at)}" }
                            }
                            Link {
                                class: "text-link",
                                to: Route::ModerationSite { id: issue.site_id.clone() },
                                "서버 관리에서 확인"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SiteIssues(issues: Vec<SiteIssue>, total: i64) -> Element {
    let displayed = issues.len().min(DISPLAY_LIMIT);
    rsx! {
        section { class: "backoffice-panel worker-issues", aria_label: "상대 서버 확인 필요 항목",
            h2 { "상대 서버 확인 필요" }
            p { class: "membership-hint",
                "최근 응답·NodeInfo 확인에 실패한 서버입니다. 이 목록만으로 워커 장애를 뜻하지는 않습니다."
            }
            p { class: "worker-issue-count", "전체 {total}건 · {displayed}건 표시" }
            if issues.is_empty() {
                p { class: "membership-notice", "확인이 필요한 상대 서버 항목이 없습니다." }
            } else {
                ul { class: "worker-issue-list",
                    for issue in issues.into_iter().take(DISPLAY_LIMIT) {
                        li { key: "{issue.site_id}-{issue.checked_at}",
                            h3 { class: "worker-domain", "{issue.domain}" }
                            p { class: "worker-issue-meta",
                                strong { "{site_issue_label(&issue)}" }
                                " · "
                                if let Some(code) = issue.status_code {
                                    "응답 {code} · "
                                } else {
                                    "응답 코드 없음 · "
                                }
                                time { datetime: issue.checked_at.clone(), "{timestamp(&issue.checked_at)}" }
                            }
                            Link {
                                class: "text-link",
                                to: Route::ModerationSite { id: issue.site_id.clone() },
                                "서버 관리에서 확인"
                            }
                        }
                    }
                }
            }
        }
    }
}
