pub(crate) mod index;
mod fediverse;
mod software;
mod server;
mod admin;
mod dev;
mod etc;

use admin::AdminRoute;
use dev::DevRoute;
use etc::EtcRoute;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use fediverse::FediverseRoute;
use server::ServerRoute;
use software::SoftwareRoute;

#[component]
pub(crate) fn AppRoutes() -> impl IntoView {
    view! {
        <Routes>
            <Route path="" view=crate::pages::index::Index/>
            <FediverseRoute />
            <SoftwareRoute />
            <ServerRoute />
            <AdminRoute />
            <DevRoute />
            <EtcRoute />
        </Routes>
    }
}