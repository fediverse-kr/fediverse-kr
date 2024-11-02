use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thaw::*;

#[component]
pub(crate) fn FkContentLayout(
    children: Children
) -> impl IntoView {
    view! {
        <Layout class="container mx-auto text-left my-10 fk-article p-6">
            {children()}
        </Layout>
    }
}
