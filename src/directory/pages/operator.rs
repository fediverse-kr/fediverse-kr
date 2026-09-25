use dioxus::prelude::*;

#[component]
pub(super) fn OperatorInformation(owner_comment: String) -> Element {
    rsx! {
        section { aria_label: "운영 정보",
            h2 { "운영 정보" }
            if !owner_comment.trim().is_empty() {
                h3 { "등록된 운영 안내" }
                p { class: "server-description", "{owner_comment}" }
            } else {
                p { "아직 등록된 운영 안내가 없어요." }
            }
            p { class: "review-note", "기존에 등록된 안내를 포함하며, 현재 운영자 신원이나 인증 상태를 보증하지 않아요. 최신 내용은 서버에서 확인해 주세요." }
            h3 { "이 서버를 운영하시나요?" }
            p { "로그인 후 DNS TXT로 도메인 제어권을 확인하면 소개·규칙·운영 안내를 직접 수정할 수 있어요. 이미 인증했다면 내 서버 관리에서 수정하세요." }
            a { class: "secondary-button", href: "/account/sites", "운영자 인증 · 정보 수정" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_information_without_note_still_explains_the_dns_edit_path() {
        let mut dom = VirtualDom::new(|| rsx! { OperatorInformation { owner_comment: "  \n" } });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert!(html.contains("아직 등록된 운영 안내가 없어요."), "{html}");
        assert!(!html.contains("<h3>등록된 운영 안내"));
        assert!(html.contains("DNS TXT"), "{html}");
        assert!(html.contains("운영자 인증 · 정보 수정"), "{html}");
        assert!(html.contains("href=\"/account/sites\""), "{html}");
        assert!(!html.contains("fk-verify="));
    }

    #[test]
    fn operator_information_surfaces_existing_note_without_claiming_verified_identity() {
        let mut dom = VirtualDom::new(|| {
            rsx! {
                OperatorInformation { owner_comment: "대리 등록 안내 <script>alert(1)</script>" }
            }
        });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert!(html.contains("운영 정보"), "{html}");
        assert!(html.contains("대리 등록 안내 &#60;script&#62;"), "{html}");
        assert!(!html.contains("<script>"));
        assert!(html.contains("등록된 운영 안내"), "{html}");
        assert!(!html.contains("인증된 운영자"));
    }
}
