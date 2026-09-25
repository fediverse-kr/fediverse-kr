use dioxus::prelude::*;

#[component]
pub(super) fn ManagementIntroduction() -> Element {
    rsx! {
        header { class: "membership-heading",
            h1 { "내 서버 관리" }
            p { "운영자 인증 후, 소개와 목록 표시를 직접 관리해요." }
        }
        section { class: "membership-card", aria_label: "운영자 인증과 정보 수정 안내",
            h2 { "DNS TXT로 인증하고 직접 수정하세요" }
            ol {
                li { "먼저 로그인하세요. 서버 인증은 로그인한 회원 계정에 연결됩니다." }
                li { "아래 ‘DNS로 운영자 인증’에서 서버 도메인을 입력하고, 안내받은 이름과 값으로 DNS TXT 레코드를 추가하세요. DNS 제어권이 있다면 연동된 운영자 계정 없이도 인증할 수 있어요." }
                li { "DNS 반영 후 ‘설정했어요 · 확인’을 누르세요. 인증값과 최근 로그인은 15분 동안 유효하므로, 시간이 지나면 다시 로그인하고 새 인증값을 받으세요." }
                li { "인증이 끝나면 ‘내가 관리하는 서버’에서 도메인을 펼치고 수정한 뒤 ‘변경 저장’을 누르세요. 이미 인증한 서버는 다시 인증할 필요가 없어요." }
            }
            p { class: "membership-hint", "이름·소개·규칙·언어·태그·운영 안내·초대/가입 승인 여부·목록 숨김을 관리자 승인 없이 수정할 수 있어요. 변경 이력은 보관됩니다." }
            p { class: "membership-hint", "DNS 인증은 기존 관리 권한을 교체할 수 있어요. 마스토돈 연락 계정이나 미스키 관리자 계정이 연동되어 있다면 ‘연동 계정으로 운영자 인증’도 이용할 수 있습니다." }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_introduction_explains_dns_and_editing_before_login() {
        let mut dom = VirtualDom::new(|| rsx! { ManagementIntroduction {} });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        for text in [
            "DNS TXT",
            "로그인",
            "도메인",
            "15분",
            "내가 관리하는 서버",
            "변경 저장",
            "운영 안내",
            "관리자 승인 없이",
        ] {
            assert!(html.contains(text), "missing {text}: {html}");
        }
        assert!(!html.contains("fk-verify="));
        assert!(!html.contains("owner_method"));
    }
}
