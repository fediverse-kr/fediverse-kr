use super::*;
use dioxus::prelude::*;

#[get("/api/public/server-health?domain")]
pub async fn server_health(domain: String) -> Result<health::History, ServerFnError> {
    if domain.is_empty() || domain.len() > 253 || domain.chars().any(char::is_control) {
        return Err(public_error(400, "서버 주소를 확인해 주세요."));
    }
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return preview::health_history(&domain)
            .ok_or_else(|| public_error(404, "표시할 수 있는 서버 정보가 없어요."));
    }
    crate::backend::config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .public_health_history(&domain)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| public_error(404, "표시할 수 있는 서버 정보가 없어요."))
}

#[get("/api/public/software-search?filters")]
pub async fn search_software(filters: String) -> Result<catalog::CatalogPage, ServerFnError> {
    let filters = catalog::CatalogQuery::parse(&filters).map_err(|e| public_error(400, e))?;
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(preview::search_catalog(&filters));
    }
    crate::backend::config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .search_catalog(&filters)
        .await
        .map_err(|_| unavailable())
}

#[get("/api/public/server-search?filters")]
pub async fn search_servers(filters: String) -> Result<search::SearchPage, ServerFnError> {
    let filters = search::ServerQuery::parse(&filters).map_err(|e| public_error(400, e))?;
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(preview::search(&filters));
    }
    crate::backend::config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .search_sites(&filters)
        .await
        .map_err(|_| unavailable())
}
#[get("/api/public/server-filters")]
pub async fn server_filters() -> Result<search::SearchOptions, ServerFnError> {
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(preview::search_options());
    }
    crate::backend::config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .search_options()
        .await
        .map_err(|_| unavailable())
}

#[get("/api/public/catalog")]
pub async fn catalog() -> Result<Catalog, ServerFnError> {
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(preview::catalog());
    }
    crate::backend::config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .public_catalog()
        .await
        .map_err(|_| unavailable())
}

#[get("/api/public/software/:name")]
pub async fn software(name: String) -> Result<SoftwareInfo, ServerFnError> {
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(public_error(400, "소프트웨어 이름을 확인해 주세요."));
    }
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        let catalog = preview::catalog();
        return Ok(SoftwareInfo {
            preview: true,
            software: catalog.software.into_iter().find(|s| s.name == name),
            categories: catalog.categories,
        });
    }
    crate::backend::config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .public_software(&name)
        .await
        .map_err(|_| unavailable())
}

#[get("/api/public/servers?query&software&page")]
pub async fn servers(
    query: String,
    software: String,
    page: u32,
) -> Result<SitePage, ServerFnError> {
    if query.len() > 256 || software.len() > 128 || page > 10_000 {
        return Err(public_error(400, "검색 조건이 너무 길어요."));
    }
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(preview::servers(&query, &software, page));
    }
    crate::backend::config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .public_sites(&query, &software, page)
        .await
        .map_err(|_| unavailable())
}

#[get("/api/public/server/:domain")]
pub async fn server(domain: String) -> Result<SiteDetail, ServerFnError> {
    if domain.len() > 253 {
        return Err(public_error(400, "서버 주소를 확인해 주세요."));
    }
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(SiteDetail {
            preview: true,
            site: preview::all_sites()
                .into_iter()
                .find(|s| s.domain == domain),
        });
    }
    Ok(SiteDetail {
        preview: false,
        site: crate::backend::config::state()
            .await
            .map_err(|_| unavailable())?
            .db
            .public_site(&domain)
            .await
            .map_err(|_| unavailable())?,
    })
}

#[get("/api/public/statistics")]
pub async fn statistics() -> Result<Statistics, ServerFnError> {
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(Statistics {
            preview: true,
            sites: 78,
            accounts: Some(97269),
            counted_sites: 78,
            oldest_observation: Some("2026-09-10".into()),
        });
    }
    crate::backend::config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .public_statistics()
        .await
        .map_err(|_| unavailable())
}

#[cfg(feature = "server")]
fn unavailable() -> ServerFnError {
    public_error(503, "정보를 불러오지 못했어요. 잠시 후 다시 시도해 주세요.")
}

#[cfg(feature = "server")]
fn public_error(code: u16, message: &str) -> ServerFnError {
    ServerFnError::ServerError {
        code,
        message: message.into(),
        details: None,
    }
}
