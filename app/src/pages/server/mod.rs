use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thaw::*;

use crate::layout::{content_layout::FkContentLayout, layout_sider::FkLayoutSider};

mod index;

#[component]
pub(crate) fn ServerLayout() -> impl IntoView {
    view! {
        <Layout has_sider=true>
            <FkLayoutSider>
                // only root path has problem always ending with /. use full path for only root path
                <a href="/server">"서버"</a>
                // continue with A (router's)
            </FkLayoutSider>
            <FkContentLayout>
                <Outlet />
            </FkContentLayout>
        </Layout>
    }
}

#[component(transparent)]
pub(crate) fn ServerRoute() -> impl IntoView {
    view! {
        <Route path="/server" view=ServerLayout>
            <Route path="/" view=index::Index/>
        </Route>
    }
}