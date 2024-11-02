use crate::error_template::{AppError, ErrorTemplate};

use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thaw::*;
use pages::AppRoutes;

pub mod error_template;
mod layout;
mod pages;
mod comp;

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        <Stylesheet href="/style.css"/>
        <Stylesheet href="/util.css"/>
        <Link rel="icon" href="/favicon.svg"/>

        // sets the document title
        <Title text="한국 연합우주 정보 모음"/>

        // content for this welcome page
        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! { <ErrorTemplate outside_errors/> }.into_view()
        }>
            <Layout class="flex flex-col">
                <LayoutHeader class="border-gray-200 border-b-2">
                    <crate::layout::nav::FkNav />
                </LayoutHeader>
                <Layout class="grow-1">
                    <main>
                        <AppRoutes />
                    </main>
                </Layout>
            </Layout>
        </Router>
    }
}
