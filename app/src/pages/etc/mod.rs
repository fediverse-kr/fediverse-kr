use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thaw::*;

use crate::layout::{content_layout::FkContentLayout, layout_sider::FkLayoutSider};

mod index;

#[component]
pub(crate) fn EtcLayout() -> impl IntoView {
    view! {
        <Layout has_sider=true>
            <FkLayoutSider>
                // only root path has problem always ending with /. use full path for only root path
                <a href="/etc">"그 외 정보"</a>
                // continue with A (router's)
            </FkLayoutSider>
            <FkContentLayout>
                <Outlet />
            </FkContentLayout>
        </Layout>
    }
}

#[component(transparent)]
pub(crate) fn EtcRoute() -> impl IntoView {
    view! {
        <Route path="/etc" view=EtcLayout>
            <Route path="/" view=index::Index/>
        </Route>
    }
}