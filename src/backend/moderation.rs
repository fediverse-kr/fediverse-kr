use super::auth::AuthError;
use crate::moderation::{Action, ActionRequest};
use chrono::NaiveDateTime;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Auth(AuthError),
    Forbidden,
    Missing,
    Invalid,
    Conflict,
    Protected,
    RefreshTooSoon,
    Unavailable,
}
impl From<AuthError> for Error {
    fn from(e: AuthError) -> Self {
        Self::Auth(e)
    }
}
impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Auth(e) => return e.fmt(f),
            Self::Forbidden => "관리자 권한이 필요합니다.",
            Self::Missing => "관리할 대상을 찾을 수 없습니다.",
            Self::Invalid => "입력 내용을 확인해 주세요. 조치 사유는 1~1,000자입니다.",
            Self::Conflict => {
                "내용이나 상태가 바뀌었습니다. 최신 내용을 확인한 뒤 다시 조치해 주세요."
            }
            Self::Protected => {
                "관리자 계정은 여기서 차단할 수 없습니다. 먼저 관리 권한을 회수해 주세요."
            }
            Self::Unavailable => "지금은 처리할 수 없습니다. 잠시 후 다시 시도해 주세요.",
            Self::RefreshTooSoon => {
                "이 서버는 최근 1시간 안에 수집을 요청했습니다. 잠시 후 다시 요청해 주세요."
            }
        })
    }
}
pub fn id(value: &str) -> Result<Uuid, Error> {
    super::community::id(value).map_err(|_| Error::Invalid)
}
pub fn page(value: u32) -> Result<(), Error> {
    super::community::page(value).map_err(|_| Error::Invalid)
}
pub fn filter(value: &str) -> Result<&str, Error> {
    match value {
        "pending" | "resolved" | "dismissed" | "all" => Ok(value),
        _ => Err(Error::Invalid),
    }
}
pub struct Command {
    pub report: Uuid,
    pub revision: NaiveDateTime,
    pub comment_revision: Option<NaiveDateTime>,
    pub author: Option<Uuid>,
    pub banned: Option<bool>,
    pub author_revision: Option<NaiveDateTime>,
    pub action: Action,
    pub note: String,
}
impl TryFrom<ActionRequest> for Command {
    type Error = Error;
    fn try_from(r: ActionRequest) -> Result<Self, Error> {
        let rev = |s: &str| super::community::revision(s).map_err(|_| Error::Invalid);
        let note = super::community::text(r.note, 1000, true).map_err(|_| Error::Invalid)?;
        if note.len() > 4000 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            report: id(&r.report)?,
            revision: rev(&r.revision)?,
            comment_revision: r.comment_revision.as_deref().map(rev).transpose()?,
            author: r.author_id.as_deref().map(id).transpose()?,
            banned: r.author_banned,
            author_revision: r.author_revision.as_deref().map(rev).transpose()?,
            action: r.action,
            note,
        })
    }
}
/// Explicit host-operator action, not an HTTP registration or automatic bootstrap.
pub async fn role_from_env(member: &str, grant: bool) -> Result<bool, &'static str> {
    let member = id(member).map_err(|_| "Administrator command requires an exact member UUID")?;
    let config = super::settings::Config::from_env()?;
    config.migrate().await?;
    config
        .connect()
        .await?
        .set_admin_role(member, grant)
        .await
        .map_err(|_| "Administrator role change failed; verify member and database state")
}
