use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use crate::comp::info_card::InfoCard;

/// Renders the home page of your application.
#[component]
pub(crate) fn Index() -> impl IntoView {
    view! {
        <main class="container text-center p-4 mx-auto">
            <section>
                <div class="flex flex-row flex-wrap justify-center items-center gap-10 p-8">
                    <div class="text-left">
                        <img class="h-32 inline-block" src="/favicon.svg" alt="fediverse.kr logo"/>
                    </div>
                    <div>
                        <h1 class="text-5xl mb-5">"fediverse.kr"</h1>
                        <h2 class="text-2xl mt-5">"한국 연합우주 정보 모음"</h2>
                    </div>
                </div>
                <p>
                   "흩어져 있는 한국 연합우주(fediverse) 정보를 한 곳에 모아 찾기 쉽게 만들어봅시다."
                </p>
            </section>
            <section class="mt-16 flex flex-row flex-wrap justify-center gap-8  ">
                <InfoCard
                    icon="/favicon.svg"
                    title="연합우주가 뭐야?"
                    
                    link_text="연합우주 알아보기"
                    link_href="/fediverse"
                >
                    <p class="my-3">"연합우주(fediverse)에 대해 자세히 알아봅시다."</p>
                </InfoCard>
                <InfoCard
                    title="소프트웨어"
                    link_text="소프트웨어 알아보기"
                    link_href="/software"
                >
                    <p class="my-3">"연합우주에 어떤 종류의 홈페이지 엔진이 있는지 알아봅시다."</p>
                    <p class="my-3">"SNS용 어플리케이션도 정리합니다."</p>
                </InfoCard>
                <InfoCard
                    title="서버"
                    link_text="서버 정보 알아보기"
                    link_href="/server"
                >
                    <p class="my-3">"연합우주에 발을 들이려면, 홈페이지를 만드는 것보다 가입하는 게 더 빠릅니다."</p>
                    <p class="my-3">"어떤 서버가 있나 알아봅시다."</p>
                </InfoCard>

                <InfoCard
                    title="운영"
                    link_text="운영 정보 알아보기"
                    link_href="/admin"
                >
                    <p class="my-3">"서버 운영은 곧 커뮤니티 운영!"</p>
                    <p class="my-3">"커뮤니티를 잘 운영하는 요령을 알아봅시다."</p>
                </InfoCard>

                <InfoCard
                    title="개발"
                    link_text="개발 정보 알아보기"
                    link_href="/dev"
                >
                    <p class="my-3">"서버 소프트웨어를 직접 만드려면? 간단한 봇을 만드려면?"</p>
                    <p class="my-3">"프로그래밍 관련 정보를 알아봅시다."</p>
                </InfoCard>
                    <InfoCard
                    title="그 외"
                    link_text="그 외 정보 알아보기"
                    link_href="/etc"
                >
                <p class="my-3">"그 외 유용한 정보를 알아봅시다."</p>
            </InfoCard>
            </section>
        </main>
    }
}
