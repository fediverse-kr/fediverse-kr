use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thaw::*;

#[component]
pub(crate) fn FkLayoutSider(
    children: Children
) -> impl IntoView {
    view! {
        <LayoutSider class="border-r-2 border-gray-200 shadow-sm p-4">
            {children()}
        </LayoutSider>
    }
}
