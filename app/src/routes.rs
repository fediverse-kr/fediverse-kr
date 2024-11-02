use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub(crate) fn AppRoutes() -> impl IntoView {
    view! {
        <Routes>
            <Route path="" view=crate::pages::index::Index/>
        </Routes>
    }
}