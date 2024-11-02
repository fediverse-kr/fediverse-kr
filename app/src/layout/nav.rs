use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub(crate) fn FkNav() -> impl IntoView {
    view! {
        <header class="flex flex-row gap-4 justify-center align-center items-center mx-auto">
            <h1 class="text-2xl">
                <A href="/" class="inline-flex flex-row justify-center gap-2 p-4">
                    <img src="/favicon.svg" class="h-10 inline-block"/>
                    <span class="text-bold inline-block">"fediverse.kr"</span>
                </A>
            </h1>
            <nav class="text-xl">
                <ul class="flex flex-row gap-4 list-none">
                    <li><A href="/fediverse">"연합우주?"</A></li>
                    <li><A href="/software">"소프트웨어"</A></li>
                    <li><A href="/server">"서버"</A></li>
                    <li><A href="/admin">"운영"</A></li>
                    <li><A href="/dev">"개발자"</A></li>
                    <li><A href="/etc">"그 외"</A></li>
                </ul>
            </nav>
        </header>
        
    }
}
