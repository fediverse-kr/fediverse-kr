//! Best-effort profile decoration, never a source of account ownership.
//! Public proof login stays successful when remote media is unavailable.
use super::{
    auth::{self, AuthenticatedSession},
    config::State,
    crawler::{PublicHttp, SiteTransport},
    db::Database,
    federation::{profile::ActorMedia, transport::safe_url, ResolvedActor},
    media,
    storage::{ObjectRef, ObjectStore},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::Semaphore;
use uuid::Uuid;

pub const AVATAR_LIMIT: usize = 1024 * 1024;
pub const EMOJI_LIMIT: usize = 256 * 1024;

#[derive(Debug)]
pub enum Error {
    Auth(auth::AuthError),
    Unavailable,
    Network,
    ActorChanged,
    Busy,
    Superseded,
}
impl From<auth::AuthError> for Error {
    fn from(e: auth::AuthError) -> Self {
        Self::Auth(e)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth(e) => e.fmt(f),
            Self::Network => f.write_str("연합 계정을 불러오지 못했어요. 기존 사진은 유지합니다."),
            Self::ActorChanged => {
                f.write_str("인증했던 계정과 주소의 주인이 달라졌어요. 기존 연결을 확인해 주세요.")
            }
            Self::Busy => f.write_str("사진 갱신은 1분 뒤 다시 시도해 주세요."),
            Self::Superseded => f.write_str("계정 상태가 변경됐어요. 새로고침 후 확인해 주세요."),
            Self::Unavailable => f.write_str("사진을 저장하지 못했어요. 기존 사진은 유지합니다."),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Clone)]
pub(crate) struct BatchJob {
    pub batch: Uuid,
    pub member: Uuid,
    pub lease: Uuid,
}
pub(crate) enum Authority {
    Member(AuthenticatedSession),
    Batch(BatchJob),
}
pub(crate) struct Ticket {
    pub authority: Authority,
    pub member: Uuid,
    pub account: Option<Uuid>,
    pub actor_id: Option<String>,
    pub handle: String,
    pub request: Uuid,
}
pub(crate) struct Asset {
    pub object: ObjectRef,
    pub bytes: Vec<u8>,
}
pub(crate) enum Download {
    Absent,
    Failed,
    Image(Asset),
}
pub(crate) struct Downloads {
    pub avatar: Download,
    pub emojis: BTreeMap<String, Download>,
    pub failed: u32,
}

// Reuse the crawler's bounded HTTPS/DNS/redirect transport. Never fetch a URL
// supplied by the browser; these originate in the canonical, linked AP actor.
pub(crate) async fn fetch_assets<T: SiteTransport + 'static>(
    transport: Arc<T>,
    media: ActorMedia,
) -> Downloads {
    let mut result = Downloads {
        avatar: Download::Absent,
        emojis: BTreeMap::new(),
        failed: 0,
    };
    let requests = media
        .avatar
        .into_iter()
        .map(|url| (None, url, AVATAR_LIMIT))
        .chain(
            media
                .emojis
                .into_iter()
                .take(10)
                .map(|(name, url)| (Some(name), url, EMOJI_LIMIT)),
        );
    let slots = Arc::new(Semaphore::new(4));
    let mut tasks = tokio::task::JoinSet::new();
    for (name, value, cap) in requests {
        if let Some(name) = &name {
            result.emojis.insert(name.clone(), Download::Failed);
        } else {
            result.avatar = Download::Failed;
        }
        let transport = transport.clone();
        let slots = slots.clone();
        tasks.spawn(async move {
            let _slot = slots.acquire_owned().await.ok();
            let bytes = async {
                let url = safe_url(&value).ok()?;
                let reply = tokio::time::timeout(Duration::from_secs(10), transport.get(&url, cap))
                    .await
                    .ok()?
                    .ok()?;
                if reply.status != 200 {
                    return None;
                }
                let bytes = reply.body.ok()?;
                if bytes.is_empty() || bytes.len() > cap {
                    return None;
                }
                Some(bytes)
            }
            .await;
            let asset = match bytes {
                None => None,
                Some(bytes) => {
                    static DECODERS: OnceLock<Arc<Semaphore>> = OnceLock::new();
                    let permit = DECODERS
                        .get_or_init(|| Arc::new(Semaphore::new(2)))
                        .clone()
                        .acquire_owned()
                        .await
                        .ok();
                    tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        media::validation::validate(&bytes, true)?;
                        Some(Asset {
                            object: ObjectRef::from_bytes(&bytes).ok()?,
                            bytes,
                        })
                    })
                    .await
                    .ok()
                    .flatten()
                }
            };
            (name, asset.map_or(Download::Failed, Download::Image))
        });
    }
    while let Some(task) = tasks.join_next().await {
        match task {
            Ok((name, download)) => {
                if let Some(name) = name {
                    result.emojis.insert(name, download);
                } else {
                    result.avatar = download;
                }
            }
            Err(_) => {
                // Task failure must preserve old files, not masquerade as absence.
                // Each pending slot was initialized to Failed.
            }
        }
    }
    result.failed = std::iter::once(&result.avatar)
        .chain(result.emojis.values())
        .filter(|d| matches!(d, Download::Failed))
        .count() as u32;
    result
}

fn slots() -> Arc<Semaphore> {
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SLOTS.get_or_init(|| Arc::new(Semaphore::new(2))).clone()
}
fn slot() -> Result<tokio::sync::OwnedSemaphorePermit, Error> {
    slots().try_acquire_owned().map_err(|_| Error::Busy)
}
async fn apply(
    db: Database,
    store: ObjectStore,
    ticket: &Ticket,
    actor: ResolvedActor,
) -> Result<u32, Error> {
    let files = download(ticket, actor).await?;
    db.finish_profile_refresh(ticket, &store, files).await
}
async fn download(ticket: &Ticket, actor: ResolvedActor) -> Result<Downloads, Error> {
    if ticket.actor_id.as_ref().is_some_and(|id| &actor.id != id) {
        return Err(Error::ActorChanged);
    }
    Ok(fetch_assets(Arc::new(PublicHttp), actor.media).await)
}

pub(crate) async fn run_batch_job(
    db: &Database,
    store: &ObjectStore,
    signer: Arc<super::federation::http_signature::RsaHttpSigner>,
    job: BatchJob,
) -> Result<(), Error> {
    // Share the same two process-wide image slots with personal refreshes.
    let permit = tokio::time::timeout(Duration::from_secs(45), slots().acquire_owned()).await;
    let Ok(Ok(_permit)) = permit else {
        return db.complete_profile_job(&job, "skipped").await;
    };
    let ticket = match db.begin_batch_profile(&job).await {
        Ok(Some(ticket)) => ticket,
        Ok(None) | Err(Error::Busy) | Err(Error::Superseded) => {
            return db.complete_profile_job(&job, "skipped").await
        }
        Err(e) => return Err(e),
    };
    // Bound network work, not cancellation through an in-progress DB publication.
    let files = tokio::time::timeout(Duration::from_secs(90), async {
        let actor = super::identity::client_with_signer(signer)
            .resolve_account(&ticket.handle)
            .await
            .map_err(|_| Error::Network)?;
        download(&ticket, actor).await
    })
    .await
    .unwrap_or(Err(Error::Network));
    let result = match files {
        Ok(files) => db.finish_profile_refresh(&ticket, store, files).await,
        Err(e) => Err(e),
    };
    let result = completed(db, &ticket, result).await;
    let outcome = match result {
        Ok(0) => "succeeded",
        Ok(_) => "partial",
        Err(Error::Superseded) | Err(Error::Busy) => "skipped",
        Err(_) => "failed",
    };
    db.complete_profile_job(&job, outcome).await
}
async fn completed(
    db: &Database,
    ticket: &Ticket,
    result: Result<u32, Error>,
) -> Result<u32, Error> {
    if result.is_err() {
        let _ = db.fail_profile_refresh(ticket).await;
    }
    result
}

pub async fn refresh(
    state: &'static State,
    session: AuthenticatedSession,
    account: Uuid,
) -> Result<u32, Error> {
    let store = state.media.as_ref().ok_or(Error::Unavailable)?.clone();
    let permit = slot()?;
    let ticket = state
        .db
        .begin_profile_refresh(&session, account, false)
        .await?
        .ok_or(Error::Superseded)?;
    // Owned task retains bounded capacity and publication guards on HTTP cancellation.
    tokio::spawn(async move {
        let _permit = permit;
        let result = match super::identity::client_with_signer(state.signer.clone())
            .resolve_account(&ticket.handle)
            .await
        {
            Ok(actor) => apply(state.db.clone(), store, &ticket, actor).await,
            Err(_) => Err(Error::Network),
        };
        completed(&state.db, &ticket, result).await
    })
    .await
    .map_err(|_| Error::Unavailable)?
}

pub async fn after_login(state: &'static State, token: &str, actor: &ResolvedActor) {
    let Some(store) = state.media.clone() else {
        return;
    };
    let Ok(permit) = slot() else {
        return;
    };
    let Ok(Some(session)) = auth::get_session_details(&state.db, token).await else {
        return;
    };
    let Ok(accounts) = state.db.linked_accounts(session.member.id).await else {
        return;
    };
    let Some(account) = accounts.into_iter().find(|a| a.actor_url == actor.id) else {
        return;
    };
    let Ok(Some(ticket)) = state
        .db
        .begin_profile_refresh(&session, account.id, true)
        .await
    else {
        return;
    };
    let actor = actor.clone();
    tokio::spawn(async move {
        let _permit = permit;
        let result = apply(state.db.clone(), store, &ticket, actor).await;
        let _ = completed(&state.db, &ticket, result).await;
    });
}
