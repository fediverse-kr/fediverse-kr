//! Compatibility discovery only; ActivityPub remains the authority for proof.
use super::{
    activitypub,
    transport::{safe_url, FederationTransport},
    webfinger::AccountHandle,
    FederationError, ResolvedActor,
};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

// Per-process load bounds supplement the existing persistent challenge limits.
// Do not cache proof misses: a user can publish the challenge immediately after.
pub(super) struct DiscoveryState {
    origins: Mutex<HashMap<String, Arc<OriginGate>>>,
    global: Semaphore,
}
struct OriginGate {
    permit: Semaphore,
    cooldown: Mutex<Option<(Instant, FederationError)>>,
}
impl DiscoveryState {
    pub(super) fn new() -> Self {
        Self {
            origins: Mutex::new(HashMap::new()),
            global: Semaphore::new(4),
        }
    }
    fn origin(&self, origin: &str) -> Result<Arc<OriginGate>, FederationError> {
        let mut origins = self.origins.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(gate) = origins.get(origin) {
            return Ok(gate.clone());
        }
        if origins.len() >= 256 {
            origins.retain(|_, gate| {
                Arc::strong_count(gate) > 1
                    || gate
                        .cooldown
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .is_some_and(|(until, _)| until > Instant::now())
            });
        }
        if origins.len() >= 256 {
            return Err(FederationError::HttpStatus(429));
        }
        let gate = Arc::new(OriginGate {
            permit: Semaphore::new(1),
            cooldown: Mutex::new(None),
        });
        origins.insert(origin.to_owned(), gate.clone());
        Ok(gate)
    }
}
pub(super) fn shared_state() -> Arc<DiscoveryState> {
    static STATE: OnceLock<Arc<DiscoveryState>> = OnceLock::new();
    STATE
        .get_or_init(|| Arc::new(DiscoveryState::new()))
        .clone()
}

const MAX_NATIVE_NOTES: usize = 20;
const MAX_AP_CANDIDATES: usize = 3;

pub(super) async fn find<T: FederationTransport>(
    transport: &T,
    actor: &ResolvedActor,
    code: &str,
    issued_at: DateTime<Utc>,
    now: DateTime<Utc>,
    state: &DiscoveryState,
) -> Result<String, FederationError> {
    let origin = safe_url(&actor.id)?.origin().ascii_serialization();
    let gate = state.origin(&origin)?;
    let _origin_permit = gate
        .permit
        .try_acquire()
        .map_err(|_| FederationError::HttpStatus(429))?;
    if let Some((until, error)) = *gate.cooldown.lock().unwrap_or_else(|e| e.into_inner()) {
        if until > Instant::now() {
            return Err(error);
        }
    }
    let _global_permit = state
        .global
        .try_acquire()
        .map_err(|_| FederationError::HttpStatus(429))?;
    let result = tokio::time::timeout(
        Duration::from_secs(12),
        discover(transport, actor, code, issued_at, now),
    )
    .await
    .map_err(|_| FederationError::Timeout)?;
    let cooldown = match result {
        Err(FederationError::HttpStatus(429)) => Some(Duration::from_secs(60)),
        Err(FederationError::HttpStatus(404 | 405 | 501) | FederationError::InvalidJson) => {
            Some(Duration::from_secs(600))
        }
        _ => None,
    };
    if let (Some(ttl), Err(error)) = (cooldown, &result) {
        *gate.cooldown.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((Instant::now() + ttl, *error));
    }
    result
}
async fn discover<T: FederationTransport>(
    transport: &T,
    actor: &ResolvedActor,
    code: &str,
    issued_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<String, FederationError> {
    let actor_url = safe_url(&actor.id)?;
    let origin = actor_url.origin().ascii_serialization();
    let account = AccountHandle::parse(&actor.handle)?;
    let user = transport
        .post_json(
            &safe_url(&format!("{origin}/api/users/show"))?,
            &json!({"username":account.username}),
        )
        .await?;
    // This is only a candidate-discovery identity. The canonical AP author
    // must still match; native username/id never grants account ownership.
    if !user["username"]
        .as_str()
        .is_some_and(|u| u.eq_ignore_ascii_case(&account.username))
        || user.get("host") != Some(&serde_json::Value::Null)
        || (!user["uri"].is_null() && user["uri"].as_str() != Some(actor.id.as_str()))
    {
        return Err(FederationError::InvalidActor);
    }
    let user_id = user["id"]
        .as_str()
        .filter(|id| valid_id(id))
        .ok_or(FederationError::InvalidActor)?;
    let value = transport
        .post_json(
            &safe_url(&format!("{origin}/api/users/notes"))?,
            &json!({
                "userId":user_id,"limit":MAX_NATIVE_NOTES,"withReplies":true,"withRenotes":false,
                "withChannelNotes":false,"withCats":false,"allowPartial":false,
            }),
        )
        .await?;
    let notes = value.as_array().ok_or(FederationError::InvalidJson)?;
    let mut seen = HashSet::new();
    for note in notes.iter().take(MAX_NATIVE_NOTES) {
        if note["userId"].as_str() != Some(user_id)
            || note["visibility"].as_str() != Some("public")
            || note["localOnly"].as_bool() != Some(false)
            || !note["text"]
                .as_str()
                .is_some_and(|text| activitypub::contains_code(text, code))
        {
            continue;
        }
        let Some(id) = note["id"].as_str().filter(|id| valid_id(id)) else {
            continue;
        };
        if seen.contains(id) {
            continue;
        }
        if seen.len() >= MAX_AP_CANDIDATES {
            break;
        }
        seen.insert(id);
        // Ignore remote-supplied uri/url strings. This Misskey-compatible
        // endpoint uses a bounded opaque note ID under the verified origin.
        let url = safe_url(&format!("{origin}/notes/{id}"))?;
        let canonical = match transport.get_json(&url, true).await {
            Ok(note) => note,
            // A deleted/malformed individual note does not imply that this
            // origin lacks native APIs, and cannot hide the next candidate.
            Err(FederationError::HttpStatus(404 | 410) | FederationError::InvalidJson) => continue,
            Err(error) => return Err(error),
        };
        if canonical["id"].as_str() != Some(url.as_str()) {
            continue;
        }
        if let Some(id) = activitypub::matching_note(&canonical, actor, code, issued_at, now) {
            return Ok(id);
        }
    }
    Err(FederationError::ProofNotFound)
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}
