//! Opt-in, one-attempt delivery. Durable outboxes and retries belong to the host.
//! No task is spawned and no network request occurs at construction time.
use crate::{
    http_signature::ActivityRequestSigner,
    publication::PreparedActivity,
    transport::{pinned_request, safe_url, MAX_BODY_BYTES, REQUEST_TIMEOUT_SECONDS},
    FederationError,
};
use chrono::{DateTime, Utc};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryOutcome {
    /// The remote HTTP endpoint accepted the request, not proof of display.
    Accepted {
        status: u16,
    },
    Retryable {
        status: u16,
        retry_after_seconds: Option<u64>,
    },
    Rejected {
        status: u16,
    },
}

pub type DeliveryFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DeliveryOutcome, FederationError>> + Send + 'a>>;

/// Keep this write capability separate from read-only compatibility API POSTs.
pub trait ActivitySender: Send + Sync {
    fn deliver<'a>(&'a self, inbox: &'a Url, activity: &'a PreparedActivity) -> DeliveryFuture<'a>;
}

pub struct HttpActivitySender {
    actor_id: String,
    signer: Arc<dyn ActivityRequestSigner>,
    slots: tokio::sync::Semaphore,
}

impl HttpActivitySender {
    /// v1 supports an actor-owned key of the form actor_id#key-name.
    /// The host must publish that actor/key and persist the activity before send.
    pub fn new(
        actor_id: &str,
        signer: Arc<dyn ActivityRequestSigner>,
    ) -> Result<Self, FederationError> {
        let actor = safe_url(actor_id)?;
        let key = Url::parse(signer.key_id()).map_err(|_| FederationError::InvalidSigningKey)?;
        if actor.as_str() != actor_id
            || actor.query().is_some()
            || key.fragment().is_none_or(str::is_empty)
            || key.as_str() != signer.key_id()
            || signer.key_id().split_once('#').map(|(owner, _)| owner) != Some(actor_id)
        {
            return Err(FederationError::InvalidSigningKey);
        }
        Ok(Self {
            actor_id: actor_id.into(),
            signer,
            slots: tokio::sync::Semaphore::new(8),
        })
    }

    async fn send_once(
        &self,
        inbox: &Url,
        activity: &PreparedActivity,
    ) -> Result<DeliveryOutcome, FederationError> {
        // Resolve no DNS and use no signing key until the explicit actor matches.
        if activity.actor_id() != self.actor_id {
            return Err(FederationError::InvalidActor);
        }
        let inbox = safe_url(inbox.as_str())?;
        let bytes = activity.as_bytes();
        if bytes.len() > MAX_BODY_BYTES {
            return Err(FederationError::BodyTooLarge);
        }
        let _permit = self
            .slots
            .acquire()
            .await
            .map_err(|_| FederationError::Network)?;
        let mut request = pinned_request(reqwest::Method::POST, &inbox)
            .await?
            .header("user-agent", "fedkr-federation/0.1")
            .header("accept", "application/activity+json")
            .body(bytes.to_vec());
        for (name, value) in self.signer.sign_post(&inbox, bytes, Utc::now())? {
            request = request.header(name, value);
        }
        let response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                FederationError::Timeout
            } else {
                FederationError::Network
            }
        })?;
        let status = response.status().as_u16();
        if response.status().is_redirection() {
            return Err(FederationError::RedirectNotAllowed);
        }
        // Never retain a remote response body (HTML, personal data, or an unbounded stream).
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok());
        Ok(classify_status(status, retry_after, Utc::now()))
    }
}

impl ActivitySender for HttpActivitySender {
    fn deliver<'a>(&'a self, inbox: &'a Url, activity: &'a PreparedActivity) -> DeliveryFuture<'a> {
        Box::pin(async move {
            tokio::time::timeout(
                Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
                self.send_once(inbox, activity),
            )
            .await
            .map_err(|_| FederationError::Timeout)?
        })
    }
}

fn classify_status(status: u16, retry_after: Option<&str>, now: DateTime<Utc>) -> DeliveryOutcome {
    match status {
        200..=299 => DeliveryOutcome::Accepted { status },
        408 | 425 | 429 | 500..=599 => DeliveryOutcome::Retryable {
            status,
            retry_after_seconds: retry_after.and_then(|v| parse_retry_after(v, now)),
        },
        _ => DeliveryOutcome::Rejected { status },
    }
}
fn parse_retry_after(value: &str, now: DateTime<Utc>) -> Option<u64> {
    let value = value.trim();
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        return value.parse().ok();
    }
    let date = DateTime::parse_from_rfc2822(value).ok()?;
    Some(date.signed_duration_since(now).num_seconds().max(0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct RejectSigner {
        key_id: String,
        calls: AtomicUsize,
    }
    impl ActivityRequestSigner for RejectSigner {
        fn key_id(&self) -> &str {
            &self.key_id
        }
        fn sign_post(
            &self,
            _: &Url,
            _: &[u8],
            _: DateTime<Utc>,
        ) -> Result<Vec<(String, String)>, FederationError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(FederationError::InvalidSigningKey)
        }
    }
    fn signer(owner: &str) -> Arc<RejectSigner> {
        Arc::new(RejectSigner {
            key_id: format!("{owner}#main-key"),
            calls: AtomicUsize::new(0),
        })
    }
    fn activity(actor: &str) -> PreparedActivity {
        PreparedActivity::public_note(
            "https://social.example.org/activities/1",
            "https://social.example.org/notes/1",
            actor,
            "https://social.example.org/followers",
            "합성 테스트",
            Utc::now(),
        )
        .unwrap()
    }
    #[test]
    fn sender_requires_the_configured_actor_owning_the_key() {
        assert!(HttpActivitySender::new(
            "https://social.example.org/actor",
            signer("https://other.example.org/actor")
        )
        .is_err());
        assert!(HttpActivitySender::new(
            "http://127.0.0.1/actor",
            signer("http://127.0.0.1/actor")
        )
        .is_err());
    }
    #[tokio::test]
    async fn unsafe_destinations_and_different_actors_fail_before_signing() {
        let key = signer("https://social.example.org/actor");
        let sender =
            HttpActivitySender::new("https://social.example.org/actor", key.clone()).unwrap();
        for url in [
            "https://127.0.0.1/inbox",
            "http://remote.example.org/inbox",
            "https://remote.example.org/inbox#x",
        ] {
            assert!(sender
                .deliver(
                    &Url::parse(url).unwrap(),
                    &activity("https://social.example.org/actor")
                )
                .await
                .is_err());
        }
        assert_eq!(
            sender
                .deliver(
                    &Url::parse("https://remote.example.org/inbox").unwrap(),
                    &activity("https://social.example.org/other")
                )
                .await,
            Err(FederationError::InvalidActor)
        );
        assert_eq!(key.calls.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn acceptance_retry_and_rejection_are_distinct_and_retry_after_is_preserved() {
        let now = DateTime::parse_from_rfc3339("2026-10-23T06:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            classify_status(202, None, now),
            DeliveryOutcome::Accepted { status: 202 }
        );
        assert_eq!(
            classify_status(429, Some("120"), now),
            DeliveryOutcome::Retryable {
                status: 429,
                retry_after_seconds: Some(120)
            }
        );
        assert_eq!(
            classify_status(503, Some("Fri, 23 Oct 2026 06:01:00 GMT"), now),
            DeliveryOutcome::Retryable {
                status: 503,
                retry_after_seconds: Some(60)
            }
        );
        assert_eq!(
            classify_status(410, None, now),
            DeliveryOutcome::Rejected { status: 410 }
        );
        assert_eq!(parse_retry_after("-1", now), None);
        assert_eq!(parse_retry_after("not a date", now), None);
    }
}
