//! Phoenix's comment limits, separated from persistence/HTTP. No account-age rule.
use super::auth::AuthError;
use chrono::NaiveDateTime;
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Auth(AuthError),
    Invalid,
    Missing,
    Conflict,
    TooLate,
    TooSoon,
    DuplicateReport,
    ReportRate,
    Unavailable,
}
impl From<AuthError> for Error {
    fn from(e: AuthError) -> Self {
        Self::Auth(e)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Auth(e) => return e.fmt(f),
            Self::Invalid => "댓글은 1~2,000자, 신고 설명은 500자 이내로 입력해 주세요.",
            Self::Missing => "이 서버나 댓글을 표시하거나 변경할 수 없어요.",
            Self::Conflict => {
                "댓글이 먼저 바뀌었어요. 입력은 남겨두었으니 최신 내용과 비교해 주세요."
            }
            Self::TooLate => {
                "작성 후 30분이 지나 수정할 수 없어요. 필요한 내용은 새 댓글로 남겨 주세요."
            }
            Self::TooSoon => "같은 서버에는 1분 간격으로 댓글을 남길 수 있어요.",
            Self::DuplicateReport => "이미 신고한 댓글입니다.",
            Self::ReportRate => "신고 요청이 많아요. 잠시 후 다시 시도해 주세요.",
            Self::Unavailable => "댓글 정보를 처리하지 못했어요. 잠시 후 다시 시도해 주세요.",
        })
    }
}
impl std::error::Error for Error {}
pub fn id(value: &str) -> Result<Uuid, Error> {
    if value.len() != 36 {
        return Err(Error::Missing);
    }
    Uuid::parse_str(value).map_err(|_| Error::Missing)
}
pub fn domain(value: &str) -> Result<String, Error> {
    super::site_management::domain(value).map_err(|_| Error::Missing)
}
pub fn page(value: u32) -> Result<(), Error> {
    if value > 10_000 {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}
pub fn text(value: String, max: usize, required: bool) -> Result<String, Error> {
    let value = value.trim().replace("\r\n", "\n");
    if (required && value.is_empty())
        || value.chars().count() > max
        || value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        Err(Error::Invalid)
    } else {
        Ok(value)
    }
}
pub fn revision(value: &str) -> Result<NaiveDateTime, Error> {
    if value.len() > 32 {
        return Err(Error::Invalid);
    }
    let date = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.6f")
        .map_err(|_| Error::Invalid)?;
    if date.format("%Y-%m-%dT%H:%M:%S%.6f").to_string() != value {
        return Err(Error::Invalid);
    }
    Ok(date)
}
/// Notification carries only a random site UUID. It is a wakeup, never authority
/// or content. Every reader rechecks database visibility after waiting.
pub mod changes {
    use std::sync::OnceLock;
    use tokio::sync::broadcast;
    use uuid::Uuid;
    static SIGNAL: OnceLock<broadcast::Sender<Uuid>> = OnceLock::new();
    pub fn subscribe() -> broadcast::Receiver<Uuid> {
        SIGNAL.get_or_init(|| broadcast::channel(128).0).subscribe()
    }
    pub fn notify(site: Uuid) {
        let _ = SIGNAL.get_or_init(|| broadcast::channel(128).0).send(site);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_limits_and_revision_are_exact() {
        assert_eq!(text(" 가\r\n나 ".into(), 2000, true).unwrap(), "가\n나");
        assert!(text("가".repeat(2000), 2000, true).is_ok());
        assert!(text("가".repeat(2001), 2000, true).is_err());
        assert!(text(" \n ".into(), 2000, true).is_err());
        assert!(text("a\0b".into(), 2000, true).is_err());
        assert!(revision("2026-09-13T12:00:00.000001").is_ok());
        assert!(revision("2026-09-13T12:00:00").is_err());
    }
}
