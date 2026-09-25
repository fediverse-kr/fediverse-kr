use super::{
    transport::{safe_url, FederationTransport},
    webfinger::AccountHandle,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;
use url::Url;

const GLOBAL_NATIVE_SLOTS: usize = 4;
const ORIGIN_NATIVE_SLOTS: usize = 1;
const MAX_ORIGIN_GATES: usize = 256;
const POSITIVE_CACHE_TTL: Duration = Duration::from_secs(60 * 60);
const NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(30);
const LOOKUP_BUDGET: Duration = Duration::from_secs(12);

#[derive(Clone)]
pub(crate) struct CreationDateCache {
    entries: Arc<Mutex<HashMap<String, CacheEntry>>>,
    origins: Arc<Mutex<HashMap<String, Arc<OriginGate>>>>,
    global: Arc<Semaphore>,
}
#[derive(Clone, Copy)]
enum CacheValue {
    Date(DateTime<Utc>),
    NoDate,
}
struct CacheEntry {
    value: CacheValue,
    expires_at: Instant,
}
struct OriginGate {
    slots: Arc<Semaphore>,
    next_allowed: Mutex<Instant>,
}
impl CreationDateCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            origins: Arc::new(Mutex::new(HashMap::new())),
            global: Arc::new(Semaphore::new(GLOBAL_NATIVE_SLOTS)),
        }
    }
    fn cached(&self, key: &str) -> Option<CacheValue> {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.get(key)?;
        if entry.expires_at <= Instant::now() {
            entries.remove(key);
            None
        } else {
            Some(entry.value)
        }
    }
    fn store(&self, key: String, value: CacheValue) {
        let ttl = match value {
            CacheValue::Date(_) => POSITIVE_CACHE_TTL,
            CacheValue::NoDate => NEGATIVE_CACHE_TTL,
        };
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|_, entry| entry.expires_at > Instant::now());
        if entries.len() >= 4096 {
            if let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.expires_at)
                .map(|(key, _)| key.clone())
            {
                entries.remove(&oldest);
            }
        }
        entries.insert(
            key,
            CacheEntry {
                value,
                expires_at: Instant::now() + ttl,
            },
        );
    }
    fn origin(&self, key: &str) -> Option<Arc<OriginGate>> {
        let mut origins = self.origins.lock().unwrap();
        if let Some(gate) = origins.get(key) {
            return Some(gate.clone());
        }
        // The actor URL is already bounded by safe_url. This cap prevents a
        // remote caller from turning origin shaping into an unbounded map.
        if origins.len() >= MAX_ORIGIN_GATES {
            origins.retain(|_, gate| {
                Arc::strong_count(gate) > 1 || *gate.next_allowed.lock().unwrap() > Instant::now()
            });
            if origins.len() >= MAX_ORIGIN_GATES {
                return None;
            }
        }
        let gate = Arc::new(OriginGate {
            slots: Arc::new(Semaphore::new(ORIGIN_NATIVE_SLOTS)),
            next_allowed: Mutex::new(Instant::now()),
        });
        origins.insert(key.to_owned(), gate.clone());
        Some(gate)
    }
}

pub(crate) fn shared_cache() -> Arc<CreationDateCache> {
    static CACHE: OnceLock<Arc<CreationDateCache>> = OnceLock::new();
    CACHE
        .get_or_init(|| Arc::new(CreationDateCache::new()))
        .clone()
}

fn valid_date(value: &Value) -> Option<DateTime<Utc>> {
    let date = DateTime::parse_from_rfc3339(value.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    (date <= Utc::now()).then_some(date)
}

fn misskey_date(value: &Value, account: &AccountHandle, actor: &Url) -> Option<DateTime<Utc>> {
    if !value["username"]
        .as_str()
        .is_some_and(|username| username.eq_ignore_ascii_case(&account.username))
        || value.get("host") != Some(&Value::Null)
    {
        return None;
    }
    // Misskey's internal `id` is not an ActivityPub actor identifier. Only use
    // `uri` when the compatible response actually supplies the canonical actor.
    if !value["uri"].is_null() && value["uri"].as_str() != Some(actor.as_str()) {
        return None;
    }
    valid_date(&value["createdAt"])
}

fn mastodon_date(value: &Value, account: &AccountHandle, actor: &Url) -> Option<DateTime<Utc>> {
    if !value["username"]
        .as_str()
        .is_some_and(|username| username.eq_ignore_ascii_case(&account.username))
        || value["uri"].as_str() != Some(actor.as_str())
    {
        return None;
    }
    let local_acct = value["acct"]
        .as_str()
        .is_some_and(|acct| acct.eq_ignore_ascii_case(&account.username));
    if !local_acct {
        return None;
    }
    valid_date(&value["created_at"])
}

fn origin_key(actor: &Url) -> String {
    actor.origin().ascii_serialization()
}

fn native_can_continue(error: &super::FederationError, gate: &OriginGate) -> bool {
    if matches!(error, super::FederationError::HttpStatus(429)) {
        *gate.next_allowed.lock().unwrap() = Instant::now() + RATE_LIMIT_BACKOFF;
        false
    } else {
        true
    }
}

/// Bounded creation-date enrichment. It is intentionally best-effort: callers
/// receive `None` on all remote failures and must keep the login path usable.
pub(crate) async fn lookup<T: FederationTransport>(
    transport: &T,
    actor: &Url,
    account: &AccountHandle,
    cache: &CreationDateCache,
) -> Option<DateTime<Utc>> {
    tokio::time::timeout(
        LOOKUP_BUDGET,
        lookup_inner(transport, actor, account, cache),
    )
    .await
    .ok()
    .flatten()
}

async fn lookup_inner<T: FederationTransport>(
    transport: &T,
    actor: &Url,
    account: &AccountHandle,
    cache: &CreationDateCache,
) -> Option<DateTime<Utc>> {
    let actor = safe_url(actor.as_str()).ok()?;
    let key = format!("{}\n{}", actor, account.display());
    if let Some(value) = cache.cached(&key) {
        return match value {
            CacheValue::Date(date) => Some(date),
            CacheValue::NoDate => None,
        };
    }
    let global = cache.global.clone().try_acquire_owned().ok()?;
    let gate = cache.origin(&origin_key(&actor))?;
    if Instant::now() < *gate.next_allowed.lock().unwrap() {
        return None;
    }
    let origin_slot = gate.slots.clone().try_acquire_owned().ok()?;
    let _slots = (global, origin_slot);

    let misskey_url = safe_url(&format!(
        "{}/api/users/show",
        actor.origin().ascii_serialization()
    ))
    .ok()?;
    let misskey = transport
        .post_json(&misskey_url, &json!({ "username": account.username }))
        .await;
    match misskey {
        Ok(value) => {
            if let Some(date) = misskey_date(&value, account, &actor) {
                cache.store(key, CacheValue::Date(date));
                return Some(date);
            }
        }
        Err(error) if !native_can_continue(&error, &gate) => return None,
        Err(_) => {}
    }

    let mut mastodon_url = safe_url(&format!(
        "{}/api/v1/accounts/lookup",
        actor.origin().ascii_serialization()
    ))
    .ok()?;
    mastodon_url
        .query_pairs_mut()
        .append_pair("acct", &account.username);
    let mastodon = transport.get_json(&mastodon_url, false).await;
    match mastodon {
        Ok(value) => {
            if let Some(date) = mastodon_date(&value, account, &actor) {
                cache.store(key, CacheValue::Date(date));
                return Some(date);
            }
            cache.store(key, CacheValue::NoDate);
        }
        Err(error) if !native_can_continue(&error, &gate) => return None,
        Err(error)
            if matches!(
                error,
                super::FederationError::HttpStatus(404 | 405 | 501)
                    | super::FederationError::InvalidJson
            ) =>
        {
            cache.store(key, CacheValue::NoDate)
        }
        Err(_) => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::federation::{transport::JsonFuture, FederationError};
    use serde_json::json;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    use tokio::time::{sleep, Duration};

    const ACTOR: &str = "https://social.example.com/users/alice";
    const HANDLE: &str = "@alice@social.example.com";

    #[derive(Clone)]
    enum Reply {
        Json(Value),
        Error(FederationError),
    }
    struct Fixture {
        misskey: Reply,
        mastodon: Reply,
        calls: Mutex<Vec<(String, String, Value)>>,
        active: AtomicUsize,
        max_active: AtomicUsize,
        delay: Duration,
    }
    impl Fixture {
        fn new(misskey: Reply, mastodon: Reply) -> Self {
            Self {
                misskey,
                mastodon,
                calls: Mutex::new(Vec::new()),
                active: AtomicUsize::new(0),
                max_active: AtomicUsize::new(0),
                delay: Duration::ZERO,
            }
        }
        fn calls(&self) -> Vec<(String, String, Value)> {
            self.calls.lock().unwrap().clone()
        }
        fn track_active(&self) {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(active, Ordering::SeqCst);
        }
        fn untrack_active(&self) {
            self.active.fetch_sub(1, Ordering::SeqCst);
        }
    }
    impl FederationTransport for Fixture {
        fn get_json<'a>(&'a self, url: &'a Url, _: bool) -> JsonFuture<'a> {
            Box::pin(async move {
                self.track_active();
                if !self.delay.is_zero() {
                    sleep(self.delay).await;
                }
                self.calls
                    .lock()
                    .unwrap()
                    .push(("GET".into(), url.to_string(), Value::Null));
                let result = match &self.mastodon {
                    Reply::Json(value) => Ok(value.clone()),
                    Reply::Error(error) => Err(*error),
                };
                self.untrack_active();
                result
            })
        }
        fn post_json<'a>(&'a self, url: &'a Url, body: &'a Value) -> JsonFuture<'a> {
            Box::pin(async move {
                self.track_active();
                if !self.delay.is_zero() {
                    sleep(self.delay).await;
                }
                self.calls
                    .lock()
                    .unwrap()
                    .push(("POST".into(), url.to_string(), body.clone()));
                let result = match &self.misskey {
                    Reply::Json(value) => Ok(value.clone()),
                    Reply::Error(error) => Err(*error),
                };
                self.untrack_active();
                result
            })
        }
    }
    fn account() -> AccountHandle {
        AccountHandle::parse(HANDLE).unwrap()
    }
    fn actor_url() -> Url {
        Url::parse(ACTOR).unwrap()
    }
    fn date(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }
    fn cache() -> CreationDateCache {
        CreationDateCache::new()
    }
    fn missing() -> Reply {
        Reply::Json(json!({"username":"alice","host":null,"uri":null}))
    }
    fn mastodon(date: &str) -> Reply {
        Reply::Json(
            json!({"id":"123","username":"alice","acct":"alice","uri":ACTOR,"created_at":date}),
        )
    }

    #[tokio::test]
    async fn misskey_missing_date_then_mastodon_runs_in_order_and_only_once_each() {
        let fixture = Fixture::new(missing(), mastodon("2024-01-16T16:17:18.000Z"));
        let got = lookup(&fixture, &actor_url(), &account(), &cache()).await;
        assert_eq!(got, Some(date("2024-01-16T16:17:18.000Z")));
        let calls = fixture.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "POST");
        assert_eq!(calls[0].1, "https://social.example.com/api/users/show");
        assert_eq!(calls[0].2, json!({"username":"alice"}));
        assert_eq!(calls[1].0, "GET");
        assert!(calls[1]
            .1
            .starts_with("https://social.example.com/api/v1/accounts/lookup?acct=alice"));
    }

    #[test]
    fn misskey_date_requires_explicit_local_host_and_identity() {
        let missing_host = json!({"username":"alice","createdAt":"2024-01-01T00:00:00Z"});
        assert_eq!(misskey_date(&missing_host, &account(), &actor_url()), None);
    }

    #[tokio::test]
    async fn origin_capacity_never_bypasses_per_origin_admission() {
        let cache = cache();
        for i in 0..MAX_ORIGIN_GATES {
            let _ = cache.origin(&format!("https://s{i}.example.com"));
        }
        let mut fixture = Fixture::new(missing(), mastodon("2024-01-16T16:17:18Z"));
        fixture.delay = Duration::from_millis(10);
        let actor = actor_url();
        let account = account();
        let _ = tokio::join!(
            lookup(&fixture, &actor, &account, &cache),
            lookup(&fixture, &actor, &account, &cache)
        );
        assert!(fixture.max_active.load(Ordering::SeqCst) <= 1);
    }

    #[tokio::test]
    async fn valid_misskey_date_short_circuits_mastodon() {
        let fixture = Fixture::new(
            Reply::Json(
                json!({"id":"misskey-internal-id","username":"alice","host":null,"uri":ACTOR,"createdAt":"2024-01-16T16:17:18.000Z"}),
            ),
            mastodon("2024-01-17T00:00:00Z"),
        );
        assert_eq!(
            lookup(&fixture, &actor_url(), &account(), &cache()).await,
            Some(date("2024-01-16T16:17:18.000Z"))
        );
        assert_eq!(fixture.calls().len(), 1);
    }

    #[tokio::test]
    async fn mastodon_missing_or_invalid_identity_never_produces_a_date() {
        for response in [
            json!({"username":"alice","acct":"alice","created_at":"2024-01-16T16:17:18Z"}),
            json!({"username":"bob","acct":"bob","uri":ACTOR,"created_at":"2024-01-16T16:17:18Z"}),
            json!({"username":"alice","acct":"alice","uri":"https://evil.example/users/alice","created_at":"2024-01-16T16:17:18Z"}),
            json!({"username":"alice","acct":"alice@foreign.example","uri":ACTOR,"created_at":"2024-01-16T16:17:18Z"}),
            json!({"username":"alice","acct":"alice","uri":ACTOR,"created_at":"not-a-date"}),
            json!({"username":"alice","acct":"alice","uri":ACTOR,"created_at":"2999-01-16T16:17:18Z"}),
        ] {
            let fixture = Fixture::new(missing(), Reply::Json(response));
            assert_eq!(
                lookup(&fixture, &actor_url(), &account(), &cache()).await,
                None
            );
            assert!(fixture.calls().len() <= 2);
        }
    }

    #[tokio::test]
    async fn transient_failures_are_bounded_and_do_not_break_the_lookup() {
        for error in [
            FederationError::HttpStatus(429),
            FederationError::HttpStatus(503),
            FederationError::Timeout,
        ] {
            let fixture = Fixture::new(Reply::Error(error), mastodon("2024-01-16T16:17:18Z"));
            let result = lookup(&fixture, &actor_url(), &account(), &cache()).await;
            if error == FederationError::HttpStatus(429) {
                assert!(result.is_none());
            } else {
                assert_eq!(result, Some(date("2024-01-16T16:17:18Z")));
            }
            assert!(fixture.calls().len() <= 2);
        }
    }

    #[tokio::test]
    async fn unsupported_results_are_negative_cached() {
        let fixture = Fixture::new(
            Reply::Error(FederationError::HttpStatus(404)),
            Reply::Error(FederationError::HttpStatus(404)),
        );
        let cache = cache();
        assert_eq!(
            lookup(&fixture, &actor_url(), &account(), &cache).await,
            None
        );
        assert_eq!(
            lookup(&fixture, &actor_url(), &account(), &cache).await,
            None
        );
        assert_eq!(fixture.calls().len(), 2);
    }

    #[tokio::test]
    async fn same_origin_requests_are_serialized_and_attempts_remain_two() {
        let mut fixture = Fixture::new(missing(), mastodon("2024-01-16T16:17:18Z"));
        fixture.delay = Duration::from_millis(10);
        let fixture = Arc::new(fixture);
        let cache = Arc::new(cache());
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let fixture = fixture.clone();
            let cache = cache.clone();
            tasks.push(tokio::spawn(async move {
                lookup(&*fixture, &actor_url(), &account(), &cache).await
            }));
        }
        let mut found = false;
        for task in tasks {
            found |= task.await.unwrap() == Some(date("2024-01-16T16:17:18Z"));
        }
        assert!(found);
        assert!(fixture.max_active.load(Ordering::SeqCst) <= 1);
        assert!(fixture.calls().len() <= 2);
    }
}
