use dioxus::prelude::*;
#[cfg(feature = "server")]
mod backend;
mod branding;
mod media_ui;
mod membership;
mod moderation;
use community::personal::MyComments;
use membership::pages::{Account, Login};
use moderation::catalog::pages::{
    ModerationCatalog, ModerationCategories, ModerationCategory, ModerationCategoryNew,
    ModerationSoftware, ModerationSoftwareEditor,
};
use moderation::overview::Moderation;
use moderation::pages::{ModerationReport, ModerationReports};
use moderation::profiles::ModerationProfiles;
use moderation::sites::pages::{ModerationSite, ModerationSites};
use moderation::workers::pages::ModerationWorkers;
mod community;
mod components;
mod demo_icons;
mod directory;
mod experience;
mod explainer;
mod explore;
mod information;
mod landing;
mod portal;
mod social_demo;
mod typing;
use directory::catalog_editing::pages::{
    SoftwareEditor, SoftwareHistory, SoftwareNew, SoftwareRestore,
};
use directory::management::pages::ManagedSites;
use directory::pages::{Platforms, ServerDetail, Servers, SoftwareDetail, SoftwareServers};
use directory::registration::pages::RegisterSite;
use explainer::Explain;
use information::{About, Apps, Develop, Migration, Operate, People, SelfHosting};
use portal::*;

#[derive(Debug, Clone, Routable, PartialEq)]
enum Route {
    #[layout(Shell)]
    #[route("/")]
    Home {},
    #[route("/start")]
    Start {},
    #[route("/start/:topic")]
    Explain { topic: String },
    #[route("/platforms?:..filters")]
    Platforms {
        filters: directory::catalog::CatalogQuery,
    },
    #[route("/software/:name")]
    SoftwareDetail { name: String },
    #[route("/software/:name/servers")]
    SoftwareServers { name: String },
    #[route("/software/:name/history")]
    SoftwareHistory { name: String },
    #[route("/people")]
    People {},
    #[route("/community")]
    Community {},
    #[route("/operate")]
    Operate {},
    #[route("/develop")]
    Develop {},
    #[route("/about")]
    About {},
    #[route("/apps")]
    Apps {},
    #[route("/guides/self-hosting")]
    SelfHosting {},
    #[route("/guides/migration")]
    Migration {},
    #[route("/servers?:..filters")]
    Servers {
        filters: directory::search::ServerQuery,
    },
    #[route("/servers/:slug")]
    ServerDetail { slug: String },
    #[route("/login")]
    Login {},
    #[route("/account")]
    Account {},
    #[route("/account/comments")]
    MyComments {},
    #[route("/account/moderation")]
    Moderation {},
    #[route("/account/moderation/reports")]
    ModerationReports {},
    #[route("/account/moderation/profiles")]
    ModerationProfiles {},
    #[route("/account/moderation/workers")]
    ModerationWorkers {},
    #[route("/account/moderation/sites")]
    ModerationSites {},
    #[route("/account/moderation/sites/:id")]
    ModerationSite { id: String },
    #[route("/account/moderation/catalog")]
    ModerationCatalog {},
    #[route("/account/moderation/catalog/software/:name")]
    ModerationSoftware { name: String },
    #[route("/account/moderation/catalog/software/:name/edit")]
    ModerationSoftwareEditor { name: String },
    #[route("/account/moderation/catalog/categories")]
    ModerationCategories {},
    #[route("/account/moderation/catalog/categories/new")]
    ModerationCategoryNew {},
    #[route("/account/moderation/catalog/category/:name")]
    ModerationCategory { name: String },
    #[route("/account/moderation/:id")]
    ModerationReport { id: String },
    #[route("/account/sites")]
    ManagedSites {},
    #[route("/account/sites/new")]
    RegisterSite {},
    #[route("/account/software/new")]
    SoftwareNew {},
    #[route("/account/software/:name/edit")]
    SoftwareEditor { name: String },
    #[route("/account/software/:name/restore/:revision")]
    SoftwareRestore { name: String, revision: i64 },
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}
#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(App);
}
#[cfg(all(feature = "server", debug_assertions))]
fn main() {
    if std::env::args().len() > 1 {
        let result = tokio::runtime::Runtime::new()
            .expect("development runtime")
            .block_on(migration_argument());
        if let Err(message) = result {
            eprintln!("{message}");
            std::process::exit(1);
        }
        return;
    }
    // Keep Dioxus's own development hot-reload loop. Initialization is once per
    // process, not once per router reload or HTTP request.
    dioxus::serve(|| async {
        if std::env::var_os("FEDKR_DATABASE_URL").is_some() {
            let state = backend::config::state()
                .await
                .map_err(std::io::Error::other)?;
            backend::runtime::start_development_once(
                state.db.clone(),
                state.media.clone(),
                state.signer.clone(),
            )
            .await;
        }
        Ok(dioxus::server::router(App)
            .layer(dioxus::fullstack::axum::middleware::from_fn(
                membership::http::guard,
            ))
            .layer(dioxus::fullstack::axum::middleware::from_fn(
                backend::http::guard,
            )))
    });
}
#[cfg(all(feature = "server", not(debug_assertions)))]
#[tokio::main]
async fn main() {
    if let Err(message) = run_server().await {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
#[cfg(feature = "server")]
async fn migration_argument() -> Result<bool, &'static str> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--legacy-activate"] || args == ["--legacy-activate", "--apply"] {
        match backend::activation::run_from_env(args.len() == 2).await {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|_| "Activation report failed")?
            ),
            Err(error) => {
                eprintln!("{error}");
                return Err("Offline activation did not complete");
            }
        }
        return Ok(true);
    }
    if args == ["--legacy-assets"] || args == ["--legacy-assets", "--apply"] {
        match backend::legacy_assets::run_from_env(args.len() == 2).await {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|_| "Asset report failed")?
            ),
            Err(error) => {
                eprintln!("{error}");
                return Err("Offline asset preservation did not complete");
            }
        }
        return Ok(true);
    }
    if args.len() == 3 && args[0] == "--admin" && matches!(args[1].as_str(), "grant" | "revoke") {
        let changed = backend::moderation::role_from_env(&args[2], args[1] == "grant").await?;
        println!(
            "{}",
            if changed {
                "Administrator role updated; audit event stored. HTTP and workers were not started."
            } else {
                "Administrator role already has the requested state. HTTP and workers were not started."
            }
        );
        return Ok(true);
    }
    if args == ["--legacy-import"] || args == ["--legacy-import", "--apply"] {
        let mode = if args.len() == 2 {
            backend::legacy::ImportMode::Apply
        } else {
            backend::legacy::ImportMode::DryRun
        };
        match backend::legacy::run_from_env(mode).await {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|_| "Import report failed")?
            ),
            Err(error) => {
                eprintln!("{error}");
                return Err("Offline import did not complete");
            }
        }
        return Ok(true);
    }
    if args == ["--migrate"] {
        let config = backend::settings::Config::from_env()?;
        if !config.is_local_database() {
            return Err("Explicit development migration accepts only the isolated local database");
        }
        config.migrate().await?;
        config
            .connect()
            .await?
            .ensure_signing_key()
            .await
            .map_err(|_| "Signing key initialization failed")?;
        println!("Embedded migrations applied; persistent signing key retained. No legacy members imported.");
        return Ok(true);
    }
    if !args.is_empty() {
        return Err("Usage: fediversekr2 [--migrate | --legacy-import [--apply] | --legacy-assets [--apply] | --legacy-activate [--apply] | --admin grant|revoke <member-uuid>]");
    }
    Ok(false)
}
#[cfg(all(feature = "server", not(debug_assertions)))]
async fn run_server() -> Result<(), &'static str> {
    if migration_argument().await? {
        return Ok(());
    }
    // Dioxus's release convention is public/ next to the executable. Reject a
    // partial deploy before migration/key initialization or background work.
    let executable = std::env::current_exe().map_err(|_| "Cannot locate release executable")?;
    if !executable
        .parent()
        .is_some_and(|dir| dir.join("public/index.html").is_file())
    {
        return Err(
            "Web bundle is missing; deploy the server and matching public directory together",
        );
    }
    let addr = dioxus_cli_config::fullstack_address_or_localhost();
    let router = dioxus::server::router(App)
        .layer(dioxus::fullstack::axum::middleware::from_fn(
            membership::http::guard,
        ))
        .layer(dioxus::fullstack::axum::middleware::from_fn(
            backend::http::guard,
        ));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|_| "HTTP listener failed")?;
    let mut services = if std::env::var_os("FEDKR_DATABASE_URL").is_some() {
        let state = backend::config::state().await?;
        Some(backend::runtime::Services::start(
            state.db.clone(),
            state.media.clone(),
            state.signer.clone(),
        ))
    } else {
        None
    }; // Unconfigured UI preview never creates a database or worker.
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let http = async {
        dioxus::fullstack::axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = stopped.changed().await;
            })
            .await
    };
    tokio::pin!(http);
    let result = tokio::select! {
        result = &mut http => result.map_err(|_| "HTTP server failed"),
        _ = backend::runtime::shutdown_signal() => {
            // Stop accepting HTTP immediately, not after the worker's 25s drain.
            // Bound existing HTTP connections separately (slow clients included).
            let _ = stop.send(true);
            let drain_http = async {
                tokio::time::timeout(std::time::Duration::from_secs(30), &mut http)
                    .await
                    .map_err(|_| "HTTP shutdown deadline exceeded")?
                    .map_err(|_| "HTTP server failed")
            };
            let drain_workers = async {
                if let Some(services) = services.take() {
                    services.shutdown().await;
                }
            };
            tokio::join!(drain_http, drain_workers).0
        }
    };
    if let Some(services) = services {
        services.shutdown().await;
    }
    result
}
#[component]
fn App() -> Element {
    let mut ready = use_signal(|| false);
    use_context_provider(move || ready);
    use_effect(move || ready.set(true));
    rsx! {
        document::Link { rel: "icon", href: branding::BRAND_MARK }
        document::Meta { name: "theme-color", content: "#fffaf2" }
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        document::Stylesheet { href: asset!("/assets/styling/portal.css") }
        document::Stylesheet { href: asset!("/assets/styling/experience.css") }
        document::Stylesheet { href: asset!("/assets/styling/landing.css") }
        document::Stylesheet { href: asset!("/assets/styling/social-demo.css") }
        document::Stylesheet { href: asset!("/assets/styling/membership.css") }
        document::Stylesheet { href: asset!("/assets/styling/backoffice.css") }
        document::Stylesheet { href: asset!("/assets/styling/directory.css") }
        document::Stylesheet { href: asset!("/assets/styling/media.css") }
        FontStyles {}
        document::Meta { name: "description", content: "서로 다른 서버에서도 이어지는 대화. 연합우주를 만나고, 내게 맞는 플랫폼과 서버를 찾아보세요." }
        div { lang: "ko", class: "portal-root", "data-ready": ready().to_string(), Router::<Route> {} }
    }
}

#[component]
fn FontStyles() -> Element {
    let font = asset!("/assets/fonts/PretendardVariable.woff2");
    let font_face = format!("@font-face{{font-family:Pretendard;src:url('{font}') format('woff2');font-weight:100 900;font-display:swap}}");
    rsx! { document::Style { "{font_face}" } }
}

#[cfg(test)]
mod route_tests {
    use super::*;

    #[test]
    fn moderation_overview_list_and_detail_have_distinct_canonical_routes() {
        assert_eq!(Route::Moderation {}.to_string(), "/account/moderation");
        assert_eq!(
            Route::ModerationReports {}.to_string(),
            "/account/moderation/reports"
        );
        assert_eq!(
            Route::ModerationReport {
                id: "report-id".into()
            }
            .to_string(),
            "/account/moderation/report-id"
        );
    }
}
