//! Registration validates a public server; it never grants operator ownership.
use super::{
    auth::{self, AuthError, AuthenticatedSession},
    crawler::{self, SiteTransport},
    db::{Database, StoreError},
    directory::{CrawlJob, Observation},
};
use crate::directory::registration::Preview;
use std::{fmt, time::Duration};
use uuid::Uuid;

// Kept separate from login and identity linking. This follows the earlier
// registration requirement; final age-policy review does not reshape auth.
pub const MIN_ACCOUNT_AGE_DAYS: i64 = 180;
pub const MAX_REGISTRATIONS_PER_DAY: i64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Auth(AuthError),
    InvalidDomain,
    Existing,
    NotFederated,
    Unavailable,
    Ineligible,
    RateLimited,
}
impl From<AuthError> for Error {
    fn from(e: AuthError) -> Self {
        Self::Auth(e)
    }
}
impl From<StoreError> for Error {
    fn from(_: StoreError) -> Self {
        Self::Unavailable
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
        Self::Auth(e)=>return e.fmt(f),
        Self::InvalidDomain=>"서버 주소만 입력해 주세요. 글·프로필 주소나 내부 주소는 사용할 수 없어요.",
        Self::Existing=>"이미 등록되어 있거나 새로 등록할 수 없는 주소예요. 기존 정보는 변경하지 않았습니다.",
        Self::NotFederated=>"서버의 응답과 NodeInfo를 확인하지 못했어요. 주소를 확인하고 다시 시도해 주세요.",
        Self::Unavailable=>"지금은 서버를 확인할 수 없어요. 잠시 후 다시 시도해 주세요.",
        Self::Ineligible=>"등록하려면 생성된 지 180일 이상 된 연합 계정을 연결해 주세요. 생성일을 확인할 수 없는 계정은 사용할 수 없어요.",
        Self::RateLimited=>"서버 등록은 하루에 10개까지 가능해요. 나중에 다시 시도해 주세요.",
    })
    }
}
impl std::error::Error for Error {}

pub fn domain(input: &str) -> Result<String, Error> {
    let trimmed = input.trim();
    if input.len() > 1024
        || trimmed.chars().any(|c| c.is_control() || c.is_whitespace())
        || trimmed.contains(['\\', '%'])
    {
        return Err(Error::InvalidDomain);
    }
    let host = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host = host.strip_suffix('/').unwrap_or(host);
    if host.len() > 253 {
        return Err(Error::InvalidDomain);
    }
    super::directory::canonical_domain(host).map_err(|_| Error::InvalidDomain)
}

// No Deserialize/HTTP input constructor. Only bounded server-side collection
// creates a checked registration; submitted preview fields are never trusted.
pub struct CheckedSite {
    domain: String,
    observation: Observation,
}
impl CheckedSite {
    pub(crate) fn domain(&self) -> &str {
        &self.domain
    }
    pub(crate) fn observation(&self) -> &Observation {
        &self.observation
    }
    pub fn preview(&self) -> Preview {
        let n = self
            .observation
            .nodeinfo
            .as_ref()
            .expect("checked NodeInfo");
        Preview {
            domain: self.domain.clone(),
            name: n.name.clone(),
            description: n.description.clone(),
            software: n.software.clone().unwrap_or_default(),
            users: n.users,
            registration_open: n.registration_open,
        }
    }
}

pub async fn inspect<T: SiteTransport>(
    db: &Database,
    s: &AuthenticatedSession,
    input: &str,
    transport: &T,
) -> Result<CheckedSite, Error> {
    let domain = domain(input)?;
    db.check_site_registration(s, &domain).await?;
    auth::consume_attempt(db, "site_registration", s.member.id.as_bytes(), 20).await?;
    auth::consume_attempt(db, "site_registration", b"network:global", 120).await?;
    let job = CrawlJob {
        id: Uuid::new_v4(),
        site_id: Uuid::new_v4(),
        domain: domain.clone(),
        lease_token: Uuid::new_v4(),
        needs_icon: false,
    };
    let observation =
        tokio::time::timeout(Duration::from_secs(30), crawler::collect(transport, &job))
            .await
            .map_err(|_| Error::Unavailable)?;
    if !observation.alive
        || observation
            .nodeinfo
            .as_ref()
            .and_then(|n| n.software.as_deref())
            .is_none()
    {
        return Err(Error::NotFederated);
    }
    // Network IO never holds a PG connection. Recheck revocation/ban/eligibility
    // and duplicates before disclosing a successful preview as well as on commit.
    db.check_site_registration(s, &domain).await?;
    Ok(CheckedSite {
        domain,
        observation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn domains_accept_site_urls_but_not_paths_ports_or_internal_targets() {
        for text in [
            " example.org ",
            "https://EXAMPLE.ORG/",
            "http://example.org/",
        ] {
            assert_eq!(domain(text).unwrap(), "example.org");
        }
        assert!(domain("https://멍멍.한국/").unwrap().starts_with("xn--"));
        for text in [
            "https://example.org/@user",
            "https://example.org/?x=1",
            "https://example.org#x",
            "https://example.org:443",
            "https://me@example.org/",
            "localhost",
            "127.0.0.1",
            "https://example.org//",
            "example.org\\other",
            "example.org%2fsecret",
            "exam\nple.org",
        ] {
            assert!(domain(text).is_err(), "{text}");
        }
    }
}
