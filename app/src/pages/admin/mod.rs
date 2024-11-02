use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thaw::*;

use crate::layout::{content_layout::FkContentLayout, layout_sider::FkLayoutSider};

mod index;

#[component]
pub(crate) fn AdminLayout() -> impl IntoView {
    view! {
        <Layout has_sider=true>
            <FkLayoutSider>
                <a href="/admin">"운영"</a>
            </FkLayoutSider>
            <FkContentLayout>
                <Outlet />
            </FkContentLayout>
        </Layout>
    }
}

#[component(transparent)]
pub(crate) fn AdminRoute() -> impl IntoView {
    view! {
        <Route path="/admin" view=AdminLayout>
            <Route path="" view=index::Index/>
        </Route>
    }
}