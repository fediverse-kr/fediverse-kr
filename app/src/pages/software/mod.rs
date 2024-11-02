use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thaw::*;

use crate::layout::{content_layout::FkContentLayout, layout_sider::FkLayoutSider};

mod index;

#[component]
pub(crate) fn SoftwareLayout() -> impl IntoView {
    view! {
        <Layout has_sider=true>
            <FkLayoutSider>
                <a href="/software">"소프트웨어?"</a>
            </FkLayoutSider>
            <FkContentLayout>
                <Outlet />
            </FkContentLayout>
        </Layout>
    }
}

#[component(transparent)]
pub(crate) fn SoftwareRoute() -> impl IntoView {
    view! {
        <Route path="/software" view=SoftwareLayout>
            <Route path="" view=index::Index/>
        </Route>
    }
}