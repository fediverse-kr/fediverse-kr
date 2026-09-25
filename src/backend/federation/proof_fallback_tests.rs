use super::*;
use serde_json::{json, Value};
use std::sync::Mutex;
use transport::JsonFuture;
use url::Url;

const ACTOR: &str = "https://social.example.com/users/alice";
const NOTE: &str = "https://social.example.com/notes/n1";
const CODE: &str = "fedkr-proof-0123456789abcdef0123456789abcdef";

struct Fixture {
    outbox: Result<Value, FederationError>,
    user: Result<Value, FederationError>,
    notes: Result<Value, FederationError>,
    canonical: Result<Value, FederationError>,
    canonical_overrides: std::collections::HashMap<String, Result<Value, FederationError>>,
    calls: Mutex<Vec<(String, String, Value)>>,
    actor_published: Option<String>,
    actor_delay: std::time::Duration,
    outbox_delay: std::time::Duration,
    native_delay: std::time::Duration,
}
impl Fixture {
    fn new() -> Self {
        Self {
            outbox: Ok(json!({"type":"OrderedCollectionPage","orderedItems":[]})),
            user: Ok(json!({"id":"local1","username":"alice","host":null,"uri":ACTOR})),
            notes: Ok(json!([{"id":"n1","userId":"local1","visibility":"public",
                "localOnly":false,"text":CODE,"createdAt":"2026-09-12T12:00:30Z"}])),
            canonical: Ok(json!({"id":NOTE,"type":"Note","attributedTo":ACTOR,
                "to":[activitypub::PUBLIC],"published":"2026-09-12T12:00:30Z","content":CODE})),
            calls: Mutex::new(Vec::new()),
            canonical_overrides: Default::default(),
            actor_published: Some("2020-01-01T00:00:00Z".into()),
            actor_delay: std::time::Duration::ZERO,
            outbox_delay: std::time::Duration::ZERO,
            native_delay: std::time::Duration::ZERO,
        }
    }
    fn actor() -> ResolvedActor {
        ResolvedActor {
            id: ACTOR.into(),
            handle: "@alice@social.example.com".into(),
            name: None,
            published: None,
            outbox: format!("{ACTOR}/outbox"),
            profile_url: None,
            media: Default::default(),
        }
    }
    fn issued() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }
}
impl FederationTransport for Fixture {
    fn get_json<'a>(&'a self, url: &'a Url, signed: bool) -> JsonFuture<'a> {
        Box::pin(async move {
            self.calls.lock().unwrap().push((
                "GET".into(),
                url.to_string(),
                json!({"signed":signed}),
            ));
            if url.path() == "/users/alice" {
                tokio::time::sleep(self.actor_delay).await;
            }
            if url.path() == "/users/alice/outbox" && url.query().is_none() {
                tokio::time::sleep(self.outbox_delay).await;
            }
            match url.path() {
                "/.well-known/webfinger" => Ok(json!({"subject":"acct:alice@social.example.com",
                    "links":[{"rel":"self","type":"application/activity+json","href":ACTOR}]})),
                "/users/alice" => Ok(
                    json!({"id":ACTOR,"type":"Person","preferredUsername":"alice",
                    "published":self.actor_published,"outbox":format!("{ACTOR}/outbox")}),
                ),
                "/users/alice/outbox" if url.query().is_none() => {
                    Ok(json!({"type":"OrderedCollection",
                    "totalItems":54,"first":format!("{ACTOR}/outbox?page=true")}))
                }
                "/users/alice/outbox" => self.outbox.clone(),
                path if path.starts_with("/notes/") => self
                    .canonical_overrides
                    .get(path)
                    .cloned()
                    .unwrap_or_else(|| self.canonical.clone()),
                _ => Err(FederationError::HttpStatus(404)),
            }
        })
    }
    fn post_json<'a>(&'a self, url: &'a Url, body: &'a Value) -> JsonFuture<'a> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap()
                .push(("POST".into(), url.to_string(), body.clone()));
            tokio::time::sleep(self.native_delay).await;
            match url.path() {
                "/api/users/show" => self.user.clone(),
                "/api/users/notes" => self.notes.clone(),
                _ => Err(FederationError::HttpStatus(404)),
            }
        })
    }
}
async fn verify(client: &FederationClient<Fixture>) -> Result<VerifiedIdentity, FederationError> {
    client
        .verify_public_post_at(
            &Fixture::actor(),
            CODE,
            Fixture::issued(),
            Fixture::issued() + chrono::Duration::minutes(1),
        )
        .await
}

#[tokio::test]
async fn outbox_success_never_calls_native_api() {
    let mut fixture = Fixture::new();
    fixture.outbox = Ok(
        json!({"type":"OrderedCollectionPage","orderedItems":[fixture.canonical.clone().unwrap()]}),
    );
    let client = FederationClient::new(fixture);
    assert_eq!(verify(&client).await.unwrap().post_id(), NOTE);
    assert!(client
        .transport
        .calls
        .lock()
        .unwrap()
        .iter()
        .all(|(method, _, _)| method == "GET"));
}

#[tokio::test]
async fn empty_outbox_falls_back_to_native_candidates_then_canonical_ap_note() {
    let client = FederationClient::new(Fixture::new());
    let proof = verify(&client)
        .await
        .expect("empty CherryPick outbox must use bounded native fallback");
    assert_eq!(proof.post_id(), NOTE);
    assert_eq!(proof.actor().id, ACTOR);
    assert_eq!(proof.challenge_issued_at(), Fixture::issued());
    assert_eq!(
        proof.challenge_code_hash(),
        <[u8; 32]>::from(Sha256::digest(CODE.as_bytes()))
    );
    let calls = client.transport.calls.lock().unwrap();
    let urls: Vec<_> = calls.iter().map(|(_, url, _)| url.as_str()).collect();
    assert_eq!(urls, vec![
        "https://social.example.com/.well-known/webfinger?resource=acct%3Aalice%40social.example.com",
        ACTOR, "https://social.example.com/users/alice/outbox",
        "https://social.example.com/users/alice/outbox?page=true",
        "https://social.example.com/api/users/show", "https://social.example.com/api/users/notes", NOTE,
    ]);
    assert_eq!(calls[4].2, json!({"username":"alice"}));
    assert_eq!(calls[5].2["withCats"], false);
    assert_eq!(calls[5].2["userId"], "local1");
    assert!(calls[5].2["limit"].as_u64().unwrap() <= 30);
    assert_eq!(calls[6].2["signed"], true);
}

#[tokio::test]
async fn deleted_native_candidate_does_not_hide_the_next_valid_ap_note() {
    let mut fixture = Fixture::new();
    let mut deleted = fixture.notes.as_ref().unwrap()[0].clone();
    deleted["id"] = json!("deleted");
    fixture
        .notes
        .as_mut()
        .unwrap()
        .as_array_mut()
        .unwrap()
        .insert(0, deleted);
    fixture.canonical_overrides.insert(
        "/notes/deleted".into(),
        Err(FederationError::HttpStatus(404)),
    );
    let client = FederationClient::new(fixture);
    assert_eq!(verify(&client).await.unwrap().post_id(), NOTE);
}

#[tokio::test]
async fn native_rate_limit_is_not_retried_during_origin_cooldown() {
    let mut fixture = Fixture::new();
    fixture.user = Err(FederationError::HttpStatus(429));
    let client = FederationClient::new(fixture);
    for _ in 0..2 {
        assert!(matches!(
            verify(&client).await,
            Err(FederationError::HttpStatus(429))
        ));
    }
    assert_eq!(
        client
            .transport
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.0 == "POST")
            .count(),
        1
    );
}

#[tokio::test]
async fn ap_failures_never_trigger_native_discovery() {
    for error in [
        FederationError::Timeout,
        FederationError::HttpStatus(401),
        FederationError::HttpStatus(403),
        FederationError::HttpStatus(429),
        FederationError::InvalidOutbox,
        FederationError::ScanLimitReached,
    ] {
        let mut fixture = Fixture::new();
        fixture.outbox = Err(error);
        let client = FederationClient::new(fixture);
        assert!(matches!(verify(&client).await, Err(actual) if actual == error));
        assert!(client
            .transport
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|c| c.0 == "GET"));
    }
}

#[tokio::test]
async fn native_candidates_never_replace_canonical_ap_validation() {
    for (field, bad_value) in [
        ("id", json!("https://other.example.com/notes/n1")),
        (
            "attributedTo",
            json!("https://social.example.com/users/bob"),
        ),
        (
            "to",
            json!(["https://social.example.com/users/alice/followers"]),
        ),
        ("type", json!("Article")),
        ("content", json!("not the code")),
        ("content", json!(format!("prefix{CODE}suffix"))),
        ("published", json!("2020-01-01T00:00:00Z")),
        ("published", json!("2027-01-01T00:00:00Z")),
    ] {
        let mut fixture = Fixture::new();
        fixture.canonical.as_mut().unwrap()[field] = bad_value;
        assert!(
            matches!(
                verify(&FederationClient::new(fixture)).await,
                Err(FederationError::ProofNotFound)
            ),
            "{field}"
        );
    }
}

#[tokio::test]
async fn private_remote_or_malformed_native_notes_are_not_dereferenced() {
    for (field, bad_value) in [
        ("userId", json!("someoneelse")),
        ("visibility", json!("followers")),
        ("localOnly", json!(true)),
        ("localOnly", json!(null)),
        ("id", json!("../../admin")),
        ("id", json!("https://other.example.com/")),
        ("text", json!("no challenge")),
    ] {
        let mut fixture = Fixture::new();
        fixture.notes.as_mut().unwrap()[0][field] = bad_value;
        let client = FederationClient::new(fixture);
        assert!(matches!(
            verify(&client).await,
            Err(FederationError::ProofNotFound)
        ));
        assert!(!client
            .transport
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.1.contains("/notes/")));
    }
}

#[tokio::test]
async fn native_identity_mismatch_stops_before_notes_request() {
    for (field, value) in [
        ("username", json!("bob")),
        ("host", json!("remote.example.com")),
        ("uri", json!("https://social.example.com/users/bob")),
        ("id", json!("../alice")),
    ] {
        let mut fixture = Fixture::new();
        fixture.user.as_mut().unwrap()[field] = value;
        let client = FederationClient::new(fixture);
        assert!(matches!(
            verify(&client).await,
            Err(FederationError::InvalidActor)
        ));
        assert_eq!(
            client
                .transport
                .calls
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.0 == "POST")
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn native_scan_and_canonical_requests_are_bounded() {
    let mut fixture = Fixture::new();
    let mut notes = vec![];
    for i in 0..100 {
        let mut note = fixture.notes.as_ref().unwrap()[0].clone();
        note["id"] = json!(format!("n{i}"));
        notes.push(note);
    }
    fixture.notes = Ok(json!(notes));
    fixture.canonical.as_mut().unwrap()["content"] = json!("no proof");
    let client = FederationClient::new(fixture);
    assert!(matches!(
        verify(&client).await,
        Err(FederationError::ProofNotFound)
    ));
    let calls = client.transport.calls.lock().unwrap();
    assert_eq!(calls.iter().filter(|c| c.1.contains("/notes/")).count(), 3);
    assert_eq!(calls.iter().filter(|c| c.0 == "POST").count(), 2);
}

#[tokio::test]
async fn creation_date_recheck_prefers_fresh_canonical_ap_date() {
    let client = FederationClient::new(Fixture::new());
    let date = client
        .creation_date_for(ACTOR, &Fixture::actor().handle)
        .await;
    assert_eq!(date.unwrap().to_rfc3339(), "2020-01-01T00:00:00+00:00");
    assert!(client
        .transport
        .calls
        .lock()
        .unwrap()
        .iter()
        .all(|c| c.0 == "GET"));
}

#[tokio::test]
async fn slow_optional_age_lookup_does_not_break_account_resolution() {
    let mut fixture = Fixture::new();
    fixture.actor_published = None;
    fixture.actor_delay = std::time::Duration::from_secs(10);
    fixture.native_delay = std::time::Duration::from_secs(60);
    let client = FederationClient::new(fixture);
    let actor = client
        .resolve_account(&Fixture::actor().handle)
        .await
        .expect("optional age timeout must not fail resolved identity");
    assert_eq!(actor.id, ACTOR);
    assert!(actor.published.is_none());
}

#[tokio::test]
async fn slow_optional_age_lookup_does_not_cancel_verified_proof() {
    let mut fixture = Fixture::new();
    fixture.actor_published = None;
    fixture.actor_delay = std::time::Duration::from_secs(10);
    fixture.outbox_delay = std::time::Duration::from_secs(10);
    fixture.native_delay = std::time::Duration::from_secs(60);
    let issued = Utc::now();
    fixture.canonical.as_mut().unwrap()["published"] = json!(issued.to_rfc3339());
    fixture.outbox = Ok(
        json!({"type":"OrderedCollectionPage","orderedItems":[fixture.canonical.clone().unwrap()]}),
    );
    let client = FederationClient::new(fixture);
    let proof = client
        .verify_public_post(&Fixture::actor(), CODE, issued)
        .await
        .expect("optional age lookup must not cancel a valid AP proof");
    assert_eq!(proof.post_id(), NOTE);
}
