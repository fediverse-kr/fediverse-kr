use leptos::*;
use leptos_meta::*;
use leptos_router::*;

/// Renders the home page of your application.
#[component]
pub(crate) fn Index() -> impl IntoView {
    view! {
        <main class="container text-center p-4 mx-auto">
            <section class="flex flex-col gap-2 p-8">
                <div>
                    <h1 class="text-5xl">"fediverse.kr"</h1>
                    <h2 class="text-2xl">"한국 연합우주 정보 모음"</h2>
                </div>
                <div>
                    <p>"서로 연결된 소셜 웹에서"</p>
                    <p>"자유롭게, 마음대로"</p>
                </div>
            </section>
        </main>
    }
}
