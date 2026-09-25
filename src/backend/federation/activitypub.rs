use super::{
    transport::{safe_url, FederationTransport},
    webfinger::AccountHandle,
    FederationError, ResolvedActor,
};
use chrono::{DateTime, Duration, Utc};
use html5ever::{
    tendril::StrTendril,
    tokenizer::{BufferQueue, Token, TokenSink, TokenSinkResult, Tokenizer},
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
};
use url::Url;

pub const PUBLIC: &str = "https://www.w3.org/ns/activitystreams#Public";
pub const MAX_CHALLENGE_AGE_MINUTES: i64 = 30;
pub const MAX_PAGES: usize = 3;
pub const MAX_ITEMS: usize = 30;
pub const MAX_DEREFERENCES: usize = 12;

pub async fn fetch_actor<T: FederationTransport>(
    transport: &T,
    account: &AccountHandle,
    actor_url: &Url,
) -> Result<ResolvedActor, FederationError> {
    let json = transport.get_json(actor_url, true).await?;
    if json["id"].as_str() != Some(actor_url.as_str())
        || !matches!(
            json["type"].as_str(),
            Some("Person" | "Service" | "Application" | "Organization" | "Group")
        )
        || !json["preferredUsername"]
            .as_str()
            .is_some_and(|name| name.eq_ignore_ascii_case(&account.username))
    {
        return Err(FederationError::InvalidActor);
    }
    let outbox = json["outbox"]
        .as_str()
        .ok_or(FederationError::InvalidActor)?;
    same_origin_url(outbox, actor_url)?;
    let name = json["name"]
        .as_str()
        .filter(|name| name.len() <= 1024)
        .map(str::to_owned);
    let profile_url = json["url"]
        .as_str()
        .and_then(|url| safe_url(url).ok())
        .map(|url| url.to_string());
    Ok(ResolvedActor {
        id: actor_url.to_string(),
        handle: account.display(),
        name,
        published: parse_date(&json["published"]),
        outbox: outbox.to_owned(),
        profile_url,
        media: super::profile::parse(&json),
    })
}

pub fn validate_challenge(
    code: &str,
    issued_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(), FederationError> {
    if !(20..=128).contains(&code.len()) || !code.bytes().all(token_byte) {
        return Err(FederationError::InvalidChallenge);
    }
    if issued_at > now || now - issued_at >= Duration::minutes(MAX_CHALLENGE_AGE_MINUTES) {
        return Err(FederationError::ChallengeExpired);
    }
    Ok(())
}

pub async fn find_public_proof<T: FederationTransport>(
    transport: &T,
    actor: &ResolvedActor,
    code: &str,
    issued_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<String, FederationError> {
    let actor_url = safe_url(&actor.id)?;
    let outbox_url = same_origin_url(&actor.outbox, &actor_url)?;
    let mut page = transport.get_json(&outbox_url, true).await?;
    let mut seen = HashSet::from([outbox_url.to_string()]);
    let mut scanned = 0;
    let mut dereferences = 0;
    for page_index in 0..MAX_PAGES {
        if !matches!(
            page["type"].as_str(),
            Some("OrderedCollection" | "OrderedCollectionPage" | "Collection" | "CollectionPage")
        ) {
            return Err(FederationError::InvalidOutbox);
        }
        let items = page["orderedItems"]
            .as_array()
            .or_else(|| page["items"].as_array());
        if let Some(items) = items {
            for item in items {
                if scanned >= MAX_ITEMS {
                    return Err(FederationError::ScanLimitReached);
                }
                scanned += 1;
                let mut item = dereference(transport, item, &actor_url, &mut dereferences).await?;
                if item["type"] == "Create" {
                    // An Announce or another person's Create is never proof.
                    if !exact_actor(&item["actor"], &actor.id) {
                        continue;
                    }
                    let object =
                        dereference(transport, &item["object"], &actor_url, &mut dereferences)
                            .await?;
                    item["object"] = object;
                }
                if let Some(post_id) = matching_note(&item, actor, code, issued_at, now) {
                    return Ok(post_id);
                }
            }
        }
        // Some collections explicitly contain [] alongside their first link.
        // It is still a collection envelope, not proof that the outbox is empty.
        let has_first = page.get("first").is_some_and(|value| !value.is_null());
        let next = if items.is_none() || (items.is_some_and(Vec::is_empty) && has_first) {
            page.get("first")
        } else {
            page.get("next")
        };
        match next {
            None | Some(Value::Null) => {
                return if items.is_some() {
                    Err(FederationError::ProofNotFound)
                } else {
                    Err(FederationError::InvalidOutbox)
                }
            }
            Some(value)
                if value.is_object()
                    && (value.get("orderedItems").is_some() || value.get("items").is_some()) =>
            {
                if page_index + 1 >= MAX_PAGES {
                    return Err(FederationError::ScanLimitReached);
                }
                page = value.clone();
            }
            Some(value) => {
                if page_index + 1 >= MAX_PAGES {
                    return Err(FederationError::ScanLimitReached);
                }
                let href = link_id(value).ok_or(FederationError::InvalidOutbox)?;
                let url = same_origin_url(href, &actor_url)?;
                if !seen.insert(url.to_string()) {
                    return Err(FederationError::ScanLimitReached);
                }
                page = transport.get_json(&url, true).await?;
            }
        }
    }
    Err(FederationError::ScanLimitReached)
}

async fn dereference<T: FederationTransport>(
    transport: &T,
    item: &Value,
    origin: &Url,
    count: &mut usize,
) -> Result<Value, FederationError> {
    if item.is_object() && item["type"] != "Link" {
        return Ok(item.clone());
    }
    let Some(href) = link_id(item) else {
        return Ok(Value::Null);
    };
    if *count >= MAX_DEREFERENCES {
        return Err(FederationError::ScanLimitReached);
    }
    let url = same_origin_url(href, origin)?;
    *count += 1;
    let object = transport.get_json(&url, true).await?;
    if object["id"].as_str() != Some(url.as_str()) {
        return Err(FederationError::InvalidOutbox);
    }
    Ok(object)
}

pub(super) fn matching_note(
    item: &Value,
    actor: &ResolvedActor,
    code: &str,
    issued_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<String> {
    let note = match item["type"].as_str()? {
        "Create" if exact_actor(&item["actor"], &actor.id) => &item["object"],
        "Note" => item,
        _ => return None,
    };
    if note["type"] != "Note"
        || !exact_actor(&note["attributedTo"], &actor.id)
        || !(has_public(&note["to"]) || has_public(&note["cc"]))
    {
        return None;
    }
    let post_id = note["id"].as_str()?;
    let actor_url = safe_url(&actor.id).ok()?;
    same_origin_url(post_id, &actor_url).ok()?;
    let published = parse_date(&note["published"])?;
    if published < issued_at || published > now {
        return None;
    }
    let content = note["content"].as_str().or_else(|| note["text"].as_str())?;
    if content.len() > 64 * 1024 || !contains_code(&visible_text(content), code) {
        return None;
    }
    Some(post_id.to_owned())
}

fn same_origin_url(value: &str, origin: &Url) -> Result<Url, FederationError> {
    let url = safe_url(value)?;
    if url.origin() != origin.origin() || url.as_str() != value {
        return Err(FederationError::UnsafeUrl);
    }
    Ok(url)
}
fn link_id(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value["href"].as_str())
        .or_else(|| value["id"].as_str())
}
fn exact_actor(value: &Value, expected: &str) -> bool {
    value.as_str() == Some(expected)
        || value
            .as_array()
            .is_some_and(|values| values.len() == 1 && values[0].as_str() == Some(expected))
}
fn has_public(value: &Value) -> bool {
    value.as_str() == Some(PUBLIC)
        || value
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(PUBLIC)))
}
fn parse_date(value: &Value) -> Option<DateTime<Utc>> {
    let date = DateTime::parse_from_rfc3339(value.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    (date <= Utc::now()).then_some(date)
}
fn token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}
pub(super) fn contains_code(text: &str, code: &str) -> bool {
    text.match_indices(code).any(|(start, matched)| {
        let end = start + matched.len();
        (start == 0 || !token_byte(text.as_bytes()[start - 1]))
            && (end == text.len() || !token_byte(text.as_bytes()[end]))
    })
}
fn visible_text(content: &str) -> String {
    // Tokenization decodes entities and excludes attributes/comments. We only
    // need a normal text post, not a CSS selector engine or arbitrary rendered
    // page. Reject the entire candidate if hidden/active/unsupported markup is
    // present; never try to approximate the browser's execution of that markup.
    struct ProofText {
        text: RefCell<String>,
        unsafe_markup: Cell<bool>,
    }
    impl TokenSink for ProofText {
        type Handle = ();
        fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
            match token {
                Token::CharacterTokens(value) => self.text.borrow_mut().push_str(&value),
                Token::TagToken(tag) => {
                    if !matches!(
                        tag.name.as_ref(),
                        "p" | "br"
                            | "span"
                            | "a"
                            | "strong"
                            | "em"
                            | "b"
                            | "i"
                            | "u"
                            | "s"
                            | "del"
                            | "code"
                            | "pre"
                            | "blockquote"
                            | "img"
                            | "ul"
                            | "ol"
                            | "li"
                            | "div"
                    ) || tag.attrs.iter().any(|attribute| {
                        matches!(attribute.name.local.as_ref(), "hidden" | "style")
                            || (attribute.name.local.as_ref() == "aria-hidden"
                                && attribute.value.as_ref() == "true")
                    }) {
                        self.unsafe_markup.set(true);
                    }
                    self.text.borrow_mut().push(' ');
                }
                Token::ParseError(_) | Token::NullCharacterToken => self.unsafe_markup.set(true),
                _ => {}
            }
            TokenSinkResult::Continue
        }
    }
    let input = BufferQueue::default();
    input.push_back(StrTendril::from(content));
    let tokenizer = Tokenizer::new(
        ProofText {
            text: RefCell::new(String::new()),
            unsafe_markup: Cell::new(false),
        },
        Default::default(),
    );
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    if tokenizer.sink.unsafe_markup.get() {
        String::new()
    } else {
        tokenizer.sink.text.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::federation::{transport::JsonFuture, FederationClient};
    use serde_json::json;
    use std::{collections::HashMap, sync::Mutex};

    const CODE: &str = "fedkr-proof-0123456789abcdef0123456789abcdef";
    const ACTOR: &str = "https://social.example.com/users/alice";
    const OUTBOX: &str = "https://social.example.com/users/alice/outbox";
    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }
    fn actor() -> ResolvedActor {
        ResolvedActor {
            id: ACTOR.into(),
            handle: "@alice@social.example.com".into(),
            name: Some("Alice".into()),
            published: None,
            outbox: OUTBOX.into(),
            profile_url: None,
            media: Default::default(),
        }
    }
    fn note() -> Value {
        json!({"type":"Note","id":"https://social.example.com/notes/123", "attributedTo":ACTOR,
        "to":[PUBLIC], "published":"2026-09-12T11:59:30Z", "content":format!("<p>인증합니다: {CODE}</p>")})
    }
    fn issued() -> DateTime<Utc> {
        now() - Duration::minutes(1)
    }
    struct FakeTransport {
        docs: HashMap<String, Value>,
        calls: Mutex<Vec<(String, bool)>>,
    }
    impl FakeTransport {
        fn new(outbox: Value) -> Self {
            let account = AccountHandle::parse("alice@social.example.com").unwrap();
            Self {
                calls: Mutex::new(vec![]),
                docs: HashMap::from([
                    (
                        account.lookup_url().to_string(),
                        json!({"subject":"acct:alice@social.example.com", "links":[{"rel":"self","type":"application/activity+json","href":ACTOR}]}),
                    ),
                    (
                        ACTOR.into(),
                        json!({"id":ACTOR,"type":"Person","preferredUsername":"alice", "name":"Alice", "outbox":OUTBOX, "published":"2000-01-01T00:00:00Z"}),
                    ),
                    (OUTBOX.into(), outbox),
                ]),
            }
        }
    }
    impl FederationTransport for FakeTransport {
        fn post_json<'a>(&'a self, url: &'a Url, _: &'a Value) -> JsonFuture<'a> {
            // This AP-only fixture explicitly models unsupported native APIs.
            self.get_json(url, false)
        }
        fn get_json<'a>(&'a self, url: &'a Url, signed: bool) -> JsonFuture<'a> {
            Box::pin(async move {
                self.calls.lock().unwrap().push((url.to_string(), signed));
                self.docs
                    .get(url.as_str())
                    .cloned()
                    .ok_or(FederationError::HttpStatus(404))
            })
        }
    }

    #[tokio::test]
    async fn resolves_account_and_verifies_fresh_public_create_without_network() {
        let transport = FakeTransport::new(
            json!({"type":"OrderedCollection", "orderedItems":[{"type":"Create","actor":ACTOR,"object":note()}]}),
        );
        let client = FederationClient::new(transport);
        let resolved = client
            .resolve_account("alice@social.example.com")
            .await
            .unwrap();
        assert_eq!(resolved.id, ACTOR);
        assert_eq!(
            resolved.published,
            Some(
                DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );
        let proof = client
            .verify_public_post_at(&resolved, CODE, issued(), now())
            .await
            .unwrap();
        assert_eq!(proof.actor().id, ACTOR);
        assert_eq!(proof.post_id(), "https://social.example.com/notes/123");
        assert_eq!(proof.verified_at(), now());
        for (url, signed) in client.transport.calls.lock().unwrap().iter() {
            assert_eq!(*signed, !url.contains(".well-known/webfinger"));
        }
    }

    #[tokio::test]
    async fn follows_bounded_first_page_and_note_references() {
        let mut transport = FakeTransport::new(
            json!({"type":"OrderedCollection", "first":format!("{OUTBOX}?page=true")}),
        );
        transport.docs.insert(format!("{OUTBOX}?page=true"), json!({"type":"OrderedCollectionPage", "orderedItems":["https://social.example.com/notes/123"]}));
        transport
            .docs
            .insert("https://social.example.com/notes/123".into(), note());
        let client = FederationClient::new(transport);
        assert!(client
            .verify_public_post_at(&actor(), CODE, issued(), now())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn accepts_inline_first_page_but_rejects_cycles_and_cross_origin_pages() {
        let transport = FakeTransport::new(
            json!({"type":"OrderedCollection", "first":{"type":"OrderedCollectionPage","orderedItems":[note()]}}),
        );
        assert!(FederationClient::new(transport)
            .verify_public_post_at(&actor(), CODE, issued(), now())
            .await
            .is_ok());
        let transport = FakeTransport::new(
            json!({"type":"OrderedCollection", "orderedItems":[], "next":OUTBOX}),
        );
        assert!(matches!(
            FederationClient::new(transport)
                .verify_public_post_at(&actor(), CODE, issued(), now())
                .await,
            Err(FederationError::ScanLimitReached)
        ));
        let transport = FakeTransport::new(
            json!({"type":"OrderedCollection", "first":"https://evil.example.com/page"}),
        );
        assert!(matches!(
            FederationClient::new(transport)
                .verify_public_post_at(&actor(), CODE, issued(), now())
                .await,
            Err(FederationError::UnsafeUrl)
        ));
    }

    #[test]
    fn refuses_other_authors_private_stale_future_boosted_or_attribute_only_proofs() {
        assert!(matching_note(&note(), &actor(), CODE, issued(), now()).is_some());
        for (field, value) in [
            (
                "attributedTo",
                json!("https://social.example.com/users/bob"),
            ),
            (
                "attributedTo",
                json!([ACTOR, "https://social.example.com/users/bob"]),
            ),
            ("id", json!("https://evil.example.com/notes/123")),
            (
                "to",
                json!(["https://social.example.com/users/alice/followers"]),
            ),
            ("published", json!("2026-09-11T00:00:00Z")),
            ("published", json!("2026-09-13T00:00:00Z")),
            ("published", Value::Null),
            (
                "content",
                json!(format!("<a href='https://example.com/{CODE}'>link</a>")),
            ),
            ("content", json!(format!("<script>{CODE}</script>"))),
            ("content", json!(format!("<span hidden>{CODE}</span>"))),
            ("content", json!(format!("prefix{CODE}suffix"))),
        ] {
            let mut invalid = note();
            invalid[field] = value;
            assert!(
                matching_note(&invalid, &actor(), CODE, issued(), now()).is_none(),
                "accepted {field}"
            );
        }
        for item in [
            json!({"type":"Announce","actor":ACTOR,"object":note()}),
            json!({"type":"Create","actor":"https://social.example.com/users/bob","object":note()}),
        ] {
            assert!(matching_note(&item, &actor(), CODE, issued(), now()).is_none());
        }
        let mut cc = note();
        cc["to"] = json!([]);
        cc["cc"] = json!([PUBLIC]);
        assert!(matching_note(&cc, &actor(), CODE, issued(), now()).is_some());
    }

    #[test]
    fn code_validation_and_html_entity_handling_are_deterministic() {
        assert!(validate_challenge(CODE, issued(), now()).is_ok());
        assert_eq!(
            validate_challenge("short", issued(), now()),
            Err(FederationError::InvalidChallenge)
        );
        assert_eq!(
            validate_challenge(CODE, now() - Duration::minutes(30), now()),
            Err(FederationError::ChallengeExpired)
        );
        assert_eq!(
            validate_challenge(CODE, now() + Duration::seconds(1), now()),
            Err(FederationError::ChallengeExpired)
        );
        assert!(contains_code(
            &visible_text(&format!("<p>&quot;{CODE}&quot;</p>")),
            CODE
        ));
        assert!(!contains_code(
            &visible_text(&format!("<!-- {CODE} -->")),
            CODE
        ));
    }

    #[tokio::test]
    async fn fresh_resolution_rejects_spoofed_actor_or_webfinger_subject() {
        let mut transport =
            FakeTransport::new(json!({"type":"OrderedCollection", "orderedItems":[note()]}));
        transport.docs.get_mut(ACTOR).unwrap()["id"] =
            json!("https://social.example.com/users/bob");
        assert!(matches!(
            FederationClient::new(transport)
                .resolve_account("alice@social.example.com")
                .await,
            Err(FederationError::InvalidActor)
        ));
        let mut transport =
            FakeTransport::new(json!({"type":"OrderedCollection", "orderedItems":[note()]}));
        let lookup = AccountHandle::parse("alice@social.example.com")
            .unwrap()
            .lookup_url();
        transport.docs.get_mut(lookup.as_str()).unwrap()["subject"] =
            json!("acct:bob@social.example.com");
        assert!(matches!(
            FederationClient::new(transport)
                .resolve_account("alice@social.example.com")
                .await,
            Err(FederationError::InvalidWebFinger)
        ));
    }

    #[tokio::test]
    async fn item_limit_and_no_proof_are_not_false_successes() {
        let mut old = note();
        old["published"] = json!("2025-01-01T00:00:00Z");
        let mut items = vec![old; MAX_ITEMS];
        items.push(note());
        let transport =
            FakeTransport::new(json!({"type":"OrderedCollection", "orderedItems":items}));
        assert!(matches!(
            FederationClient::new(transport)
                .verify_public_post_at(&actor(), CODE, issued(), now())
                .await,
            Err(FederationError::ScanLimitReached)
        ));
        let transport = FakeTransport::new(json!({"type":"OrderedCollection", "orderedItems":[]}));
        assert!(matches!(
            FederationClient::new(transport)
                .verify_public_post_at(&actor(), CODE, issued(), now())
                .await,
            Err(FederationError::ProofNotFound)
        ));
    }

    #[tokio::test]
    async fn provider_failure_is_explicit_and_not_rest_fallback_or_success() {
        struct Denied;
        impl FederationTransport for Denied {
            fn get_json<'a>(&'a self, _: &'a Url, _: bool) -> JsonFuture<'a> {
                Box::pin(async { Err(FederationError::HttpStatus(403)) })
            }
        }
        assert!(matches!(
            FederationClient::new(Denied)
                .resolve_account("alice@social.example.com")
                .await,
            Err(FederationError::HttpStatus(403))
        ));
    }

    #[tokio::test]
    async fn empty_collection_items_do_not_hide_the_first_page() {
        let page_url = format!("{OUTBOX}?page=1");
        let mut transport = FakeTransport::new(
            json!({"type":"OrderedCollection", "orderedItems":[], "first":page_url}),
        );
        transport.docs.insert(
            page_url,
            json!({"type":"OrderedCollectionPage", "orderedItems":[note()]}),
        );
        assert!(FederationClient::new(transport)
            .verify_public_post_at(&actor(), CODE, issued(), now())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn page_bound_is_checked_before_fetching_an_extra_page() {
        let mut transport = FakeTransport::new(
            json!({"type":"OrderedCollection", "first":format!("{OUTBOX}?page=1")}),
        );
        for page in 1..=MAX_PAGES {
            transport.docs.insert(format!("{OUTBOX}?page={page}"), json!({"type":"OrderedCollectionPage", "orderedItems":[], "next":format!("{OUTBOX}?page={}", page+1)}));
        }
        let client = FederationClient::new(transport);
        assert!(matches!(
            client
                .verify_public_post_at(&actor(), CODE, issued(), now())
                .await,
            Err(FederationError::ScanLimitReached)
        ));
        let calls = client.transport.calls.lock().unwrap();
        assert_eq!(
            calls
                .iter()
                .filter(|(url, _)| url.starts_with(OUTBOX))
                .count(),
            MAX_PAGES
        );
        assert!(!calls
            .iter()
            .any(|(url, _)| url == &format!("{OUTBOX}?page={MAX_PAGES}")));
    }

    #[tokio::test]
    async fn account_domain_may_differ_from_authoritative_actor_server() {
        let alias = AccountHandle::parse("alice@accounts.example.com").unwrap();
        let mut transport =
            FakeTransport::new(json!({"type":"OrderedCollection", "orderedItems":[note()]}));
        transport.docs.insert(alias.lookup_url().to_string(), json!({"subject":alias.resource(), "links":[{"rel":"self","type":"application/activity+json","href":ACTOR}]}));
        let client = FederationClient::new(transport);
        let resolved = client
            .resolve_account("alice@accounts.example.com")
            .await
            .unwrap();
        assert_eq!(resolved.id, ACTOR);
        assert_eq!(resolved.handle, "@alice@accounts.example.com");
        assert!(client
            .verify_public_post_at(&resolved, CODE, issued(), now())
            .await
            .is_ok());
    }
}
