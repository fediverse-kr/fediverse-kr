use super::{
    api,
    pages::timestamp,
    workers::{api as workers_api, WorkerOverview},
    ReportPage, ReportSummary,
};
use crate::Route;
use dioxus::prelude::*;

#[derive(Clone, Debug, PartialEq)]
enum OverviewData<T> {
    Loading,
    Ready(T),
    Failed,
}

fn report_reason(value: &str) -> &'static str {
    match value {
        "spam" => "스팸",
        "abuse" => "괴롭힘·욕설",
        "offtopic" => "주제와 무관",
        _ => "기타",
    }
}

#[component]
pub fn Moderation() -> Element {
    let reports = use_server_future(|| api::reports("pending".into(), 0))?;
    let operations = use_server_future(workers_api::overview)?;
    let report_data = match reports() {
        Some(Ok(data)) => OverviewData::Ready(data),
        Some(Err(_)) => OverviewData::Failed,
        None => OverviewData::Loading,
    };
    let operation_data = match operations() {
        Some(Ok(data)) => OverviewData::Ready(data),
        Some(Err(_)) => OverviewData::Failed,
        None => OverviewData::Loading,
    };

    rsx! {
        document::Title { "운영 개요 — fediverse.kr" }
        super::layout::BackofficePage {
            section: super::layout::BackofficeSection::Overview,
            title: "운영 개요".to_string(),
            description: "지금 먼저 확인할 신고와 수집 상태를 한곳에서 살펴봅니다.".to_string(),
            OverviewContent { reports: report_data, operations: operation_data }
        }
    }
}

#[component]
fn OverviewContent(
    reports: OverviewData<ReportPage>,
    operations: OverviewData<WorkerOverview>,
) -> Element {
    rsx! {
        div { class: "backoffice-overview",
            RecentReports { data: reports }
            OperationSummary { data: operations }
        }
    }
}

#[component]
fn RecentReports(data: OverviewData<ReportPage>) -> Element {
    rsx! {
        section { class: "backoffice-panel backoffice-overview-reports", aria_label: "최근 미처리 신고",
            div { class: "backoffice-panel-heading",
                div {
                    p { class: "backoffice-kicker", "검토 대기" }
                    h2 { "최근 미처리 신고" }
                }
                a { class: "text-link", href: Route::ModerationReports {}.to_string(), "신고 목록 전체 보기" }
            }
            p { class: "backoffice-panel-note", "최근 항목을 최대 5개만 보여줍니다. 이 표본의 길이는 전체 미처리 신고 수가 아닙니다." }
            match data {
                OverviewData::Loading => rsx! { p { class: "backoffice-state", role: "status", "미처리 신고를 불러오고 있습니다." } },
                OverviewData::Failed => rsx! {
                    p { class: "backoffice-state is-error", role: "alert", "미처리 신고를 불러오지 못했습니다. 신고 목록에서 다시 확인해 주세요." }
                },
                OverviewData::Ready(data) if data.reports.is_empty() => rsx! {
                    p { class: "backoffice-state is-empty", "현재 표본에 미처리 신고가 없습니다." }
                },
                OverviewData::Ready(data) => rsx! {
                    ul { class: "backoffice-report-sample",
                        for report in data.reports.into_iter().take(5) {
                            ReportRow { report }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn ReportRow(report: ReportSummary) -> Element {
    let href = Route::ModerationReport {
        id: report.id.clone(),
    }
    .to_string();
    let domain = report.domain.as_deref().unwrap_or("연결된 서버 없음");
    rsx! {
        li { key: "{report.id}", "data-overview-report": "true",
            div { class: "backoffice-report-copy",
                p { class: "backoffice-report-meta",
                    span { "{report_reason(&report.reason)}" }
                    time { datetime: report.created_at.clone(), "{timestamp(&report.created_at)}" }
                }
                h3 { a { href, "{domain}" } }
                p { class: "backoffice-report-excerpt", "{report.excerpt}" }
            }
            a { class: "text-link", href: Route::ModerationReport { id: report.id }.to_string(), "검토" }
        }
    }
}

#[component]
fn OperationSummary(data: OverviewData<WorkerOverview>) -> Element {
    rsx! {
        section { class: "backoffice-panel backoffice-overview-operations", aria_label: "수집 작업 상태",
            div { class: "backoffice-panel-heading",
                div {
                    p { class: "backoffice-kicker", "수집 작업" }
                    h2 { "작업 현황" }
                }
                a { class: "text-link", href: Route::ModerationWorkers {}.to_string(), "작업 현황 자세히 보기" }
            }
            match data {
                OverviewData::Loading => rsx! { p { class: "backoffice-state", role: "status", "작업 현황을 불러오고 있습니다." } },
                OverviewData::Failed => rsx! {
                    p { class: "backoffice-state is-error", role: "alert", "작업 현황을 불러오지 못했습니다. 작업 현황에서 다시 확인해 주세요." }
                },
                OverviewData::Ready(data) => {
                    let attention = data.job_issues_total.saturating_add(data.site_issues_total);
                    rsx! {
                        p { class: "backoffice-snapshot", "조회 시각 " time { datetime: data.captured_at.clone(), "{timestamp(&data.captured_at)}" } }
                        dl { class: "backoffice-operation-facts",
                            div { dt { "확인 필요" } dd { "{attention}건" } }
                            div { dt { "실행 대기" } dd { "{data.queue.ready}건" } }
                            div { dt { "실행 중" } dd { "{data.queue.running}건" } }
                            div { dt { "중단" } dd { "{data.queue.dead}건" } }
                        }
                        p { class: "backoffice-panel-note", "DB에 기록된 작업 기준입니다. ‘실행 중’은 워커의 현재 응답을 보장하지 않으며, 완료는 상대 서버 수집 성공과 다를 수 있습니다." }
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moderation::workers::QueueSummary;

    fn report(index: usize) -> ReportSummary {
        ReportSummary {
            id: format!("report-{index}"),
            status: "pending".into(),
            reason: "spam".into(),
            domain: Some(format!("server-{index}.example")),
            reporter_name: None,
            excerpt: format!("excerpt {index}"),
            created_at: "2026-09-21T00:00:00Z".into(),
        }
    }

    fn workers() -> WorkerOverview {
        WorkerOverview {
            captured_at: "2026-09-21T00:00:00Z".into(),
            queue: QueueSummary {
                ready: 2,
                running: 1,
                ..Default::default()
            },
            job_issues_total: 2,
            job_issues: vec![],
            site_issues_total: 3,
            site_issues: vec![],
        }
    }

    fn render(
        reports: OverviewData<ReportPage>,
        operations: OverviewData<WorkerOverview>,
    ) -> String {
        let mut dom = VirtualDom::new_with_props(
            OverviewContent,
            OverviewContentProps {
                reports,
                operations,
            },
        );
        dom.rebuild_in_place();
        dioxus::ssr::render(&dom)
    }

    #[test]
    fn overview_limits_report_sample_and_keeps_partial_empty_error_states_honest() {
        let reports = ReportPage {
            reports: (1..=6).map(report).collect(),
            page: 0,
            has_next: true,
        };
        let html = render(OverviewData::Ready(reports), OverviewData::Failed);
        assert_eq!(html.matches("data-overview-report").count(), 5, "{html}");
        assert!(!html.contains("server-6.example"), "{html}");
        assert!(html.contains("작업 현황을 불러오지 못했습니다"), "{html}");
        assert!(
            !html.contains("미처리 신고 5건"),
            "sample size must not be presented as a total: {html}"
        );

        let html = render(
            OverviewData::Ready(ReportPage {
                reports: vec![],
                page: 0,
                has_next: false,
            }),
            OverviewData::Ready(workers()),
        );
        assert!(
            html.contains("현재 표본에 미처리 신고가 없습니다"),
            "{html}"
        );
        assert!(html.contains("확인 필요</dt><dd>5건"), "{html}");
        assert!(html.contains("실행 대기</dt><dd>2건"), "{html}");
        assert!(html.contains("실행 중</dt><dd>1건"), "{html}");

        let html = render(OverviewData::Loading, OverviewData::Ready(workers()));
        assert!(html.contains("미처리 신고를 불러오고 있습니다"), "{html}");
        assert!(html.contains("확인 필요</dt><dd>5건"), "{html}");
    }
}
