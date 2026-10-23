//! Optional pure preparation of a public ActivityStreams Create/Note.
//! This module does not sign, send, persist, or deduplicate activities.
use crate::transport::safe_url;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{json, Value};
use std::{collections::HashSet, fmt};
use url::Url;

pub const PUBLIC: &str = "https://www.w3.org/ns/activitystreams#Public";
pub const MAX_NOTE_TEXT_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationError {
    UnsafeUrl,
    NonCanonicalUrl,
    DifferentOrigin,
    DuplicateIds,
    EmptyText,
    TextTooLarge,
    Serialization,
}

impl fmt::Display for PublicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsafeUrl => "publication URL is unsafe",
            Self::NonCanonicalUrl => {
                "publication URL must be canonical and have no query or fragment"
            }
            Self::DifferentOrigin => "publication URLs must share one origin",
            Self::DuplicateIds => "publication URLs must be distinct",
            Self::EmptyText => "publication text must not be empty",
            Self::TextTooLarge => "publication text exceeds the configured limit",
            Self::Serialization => "could not serialize publication activity",
        })
    }
}

impl std::error::Error for PublicationError {}

/// Immutable prepared JSON and its serialized representation.
/// IDs are caller-owned stable identifiers; this value does not prevent replay.
#[derive(Clone, Debug)]
pub struct PreparedActivity {
    document: Value,
    bytes: Vec<u8>,
    activity_id: String,
    actor_id: String,
}

impl PreparedActivity {
    /// Prepare a public `Create` containing a plain-text `Note`.
    ///
    /// All four URLs must be canonical HTTPS URLs with no query or fragment,
    /// use the same origin, and be distinct. `activity_id` and `object_id`
    /// should be stable across retries performed by the caller.
    pub fn public_note(
        activity_id: &str,
        object_id: &str,
        actor_id: &str,
        followers_url: &str,
        text: &str,
        published_utc: DateTime<Utc>,
    ) -> Result<Self, PublicationError> {
        if text.is_empty() {
            return Err(PublicationError::EmptyText);
        }
        if text.len() > MAX_NOTE_TEXT_BYTES {
            return Err(PublicationError::TextTooLarge);
        }

        let urls = [activity_id, object_id, actor_id, followers_url]
            .map(canonical_id_url)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let origin = urls[0].origin().ascii_serialization();
        if urls
            .iter()
            .any(|url| url.origin().ascii_serialization() != origin)
        {
            return Err(PublicationError::DifferentOrigin);
        }
        let unique: HashSet<_> = urls.iter().map(Url::as_str).collect();
        if unique.len() != urls.len() {
            return Err(PublicationError::DuplicateIds);
        }

        let published = published_utc.to_rfc3339_opts(SecondsFormat::AutoSi, true);
        let escaped = escape_html_text(text);
        let document = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": activity_id,
            "type": "Create",
            "actor": actor_id,
            "to": [PUBLIC],
            "cc": [followers_url],
            "published": published,
            "object": {
                "id": object_id,
                "type": "Note",
                "attributedTo": actor_id,
                "to": [PUBLIC],
                "cc": [followers_url],
                "published": published,
                "content": escaped,
                "source": {
                    "mediaType": "text/plain",
                    "content": text
                }
            }
        });
        let bytes = serde_json::to_vec(&document).map_err(|_| PublicationError::Serialization)?;
        Ok(Self {
            document,
            bytes,
            activity_id: activity_id.to_owned(),
            actor_id: actor_id.to_owned(),
        })
    }

    /// Borrow the prepared JSON without exposing mutable state.
    pub fn json(&self) -> &Value {
        &self.document
    }

    /// Borrow the exact compact JSON bytes prepared at construction time.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return a copy of the serialized body for a transport adapter.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn activity_id(&self) -> &str {
        &self.activity_id
    }

    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }
}

fn canonical_id_url(input: &str) -> Result<Url, PublicationError> {
    let url = safe_url(input).map_err(|_| PublicationError::UnsafeUrl)?;
    if url.as_str() != input || url.query().is_some() || url.fragment().is_some() {
        return Err(PublicationError::NonCanonicalUrl);
    }
    Ok(url)
}

fn escape_html_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        escaped.push_str(match character {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            '\'' => "&#39;",
            _ => {
                escaped.push(character);
                continue;
            }
        });
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTIVITY: &str = "https://social.example.org/activities/1";
    const OBJECT: &str = "https://social.example.org/notes/1";
    const ACTOR: &str = "https://social.example.org/users/bot";
    const FOLLOWERS: &str = "https://social.example.org/users/bot/followers";

    fn prepared(text: &str) -> PreparedActivity {
        PreparedActivity::public_note(
            ACTIVITY,
            OBJECT,
            ACTOR,
            FOLLOWERS,
            text,
            DateTime::parse_from_rfc3339("2026-10-23T01:02:03Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .unwrap()
    }

    #[test]
    fn stable_inputs_produce_stable_bytes_and_timestamp() {
        let first = prepared("hello");
        let second = prepared("hello");
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert_eq!(first.as_bytes(), serde_json::to_vec(first.json()).unwrap());
        assert_eq!(first.to_bytes(), first.as_bytes());
        assert_eq!(first.activity_id(), ACTIVITY);
        assert_eq!(first.actor_id(), ACTOR);
        assert_eq!(first.json()["published"], "2026-10-23T01:02:03Z");
        assert_eq!(
            first.json()["object"]["published"],
            first.json()["published"]
        );
    }

    #[test]
    fn escapes_markup_and_script_payloads_as_text() {
        let activity = prepared("<script>alert('x')</script> & \"hello\"");
        let content = activity.json()["object"]["content"].as_str().unwrap();
        assert_eq!(
            content,
            "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt; &amp; &quot;hello&quot;"
        );
        assert_eq!(
            activity.json()["object"]["source"]["mediaType"],
            "text/plain"
        );
        assert_eq!(
            activity.json()["object"]["source"]["content"],
            "<script>alert('x')</script> & \"hello\""
        );
        assert!(!content.contains("<script>"));
    }

    #[test]
    fn actor_and_public_recipients_match_on_create_and_note() {
        let activity = prepared("hello");
        let note = &activity.json()["object"];
        assert_eq!(activity.json()["actor"], note["attributedTo"]);
        assert_eq!(activity.json()["to"], note["to"]);
        assert_eq!(activity.json()["cc"], note["cc"]);
        assert_eq!(activity.json()["to"][0], PUBLIC);
        assert_eq!(activity.json()["cc"][0], FOLLOWERS);
    }

    #[test]
    fn rejects_unsafe_mixed_origin_and_ambiguous_ids() {
        let at = DateTime::parse_from_rfc3339("2026-10-23T01:02:03Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(matches!(
            PreparedActivity::public_note(
                "http://social.example.org/a",
                OBJECT,
                ACTOR,
                FOLLOWERS,
                "x",
                at
            ),
            Err(PublicationError::UnsafeUrl)
        ));
        assert!(matches!(
            PreparedActivity::public_note(
                ACTIVITY,
                OBJECT,
                ACTOR,
                "https://elsewhere.example.org/followers",
                "x",
                at
            ),
            Err(PublicationError::DifferentOrigin)
        ));
        assert!(matches!(
            PreparedActivity::public_note(
                ACTIVITY,
                OBJECT,
                ACTOR,
                "https://social.example.org/users/bot",
                "x",
                at
            ),
            Err(PublicationError::DuplicateIds)
        ));
        assert!(matches!(
            PreparedActivity::public_note(
                "https://social.example.org/activities/1?x=1",
                OBJECT,
                ACTOR,
                FOLLOWERS,
                "x",
                at
            ),
            Err(PublicationError::NonCanonicalUrl)
        ));
    }

    #[test]
    fn rejects_empty_and_oversized_plain_text() {
        let at = DateTime::parse_from_rfc3339("2026-10-23T01:02:03Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(matches!(
            PreparedActivity::public_note(ACTIVITY, OBJECT, ACTOR, FOLLOWERS, "", at),
            Err(PublicationError::EmptyText)
        ));
        let oversized = "x".repeat(MAX_NOTE_TEXT_BYTES + 1);
        assert!(matches!(
            PreparedActivity::public_note(ACTIVITY, OBJECT, ACTOR, FOLLOWERS, &oversized, at),
            Err(PublicationError::TextTooLarge)
        ));
    }
}
