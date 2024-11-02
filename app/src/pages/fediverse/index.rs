use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thaw::*;

#[component]
pub(crate) fn Index() -> impl IntoView {
    view! {
        <section>
            <h2 class="text-4xl">"연합우주(페디버스)가 뭐야?"</h2>
            <p>
                "소셜망 서비스가 특정 기업에 의해 오락가락 하지 않는다면 얼마나 좋을까요?"
            </p>
            <p>
                "서로서로 홈페이지를 만들고 홈페이지끼리 소통할 수 있다면, 회사와는 관계 없이 나만의 아늑한 공간을 만들 수 있을 겁니다."
            </p>
            <p>
                "연합우주(fediverse)란, 서로 소통할 수 있는 소셜 홈페이지들의 연합을 말합니다. 마치 이메일처럼, 내 계정을 어떤 홈페이지에 만들면, 다른 홈페이지에 있는 사람과 소통할 수 있습니다."
            </p>
            
        </section>
        <section>
            <h2 class="text-4xl">"어떤 홈페이지가 있을까요?"</h2>
            <p>"(* 준비중)"</p>
            <A class="fk-button text-right" href="/software">"더 알아보기"</A>
        </section>
        <section>
            <h2 class="text-4xl">"어떻게 홈페이지끼리 서로 소통하나요?"</h2>
            <p>
                "홈페이지끼리 소통하는 방식은 여러가지가 있지만, AcitivityPub 가 주로 쓰입니다."
            </p>
            <p>"(* 준비중)"</p>
        </section>
    }
}
