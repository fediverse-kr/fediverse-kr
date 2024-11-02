use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thaw::*;

use crate::layout::{content_layout::FkContentLayout, layout_sider::FkLayoutSider};

mod index;

#[component]
pub(crate) fn DevLayout() -> impl IntoView {
    view! {
        <Layout has_sider=true>
            <FkLayoutSider>
                <a href="/dev">"개발자"</a>
            </FkLayoutSider>
            <FkContentLayout>
                <Outlet />
            </FkContentLayout>
        </Layout>
    }
}

#[component(transparent)]
pub(crate) fn DevRoute() -> impl IntoView {
    view! {
        <Route path="/dev" view=DevLayout>
            <Route path="" view=index::Index/>
        </Route>
    }
}