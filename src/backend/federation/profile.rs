//! Read-only AP profile media, following the MIT Phoenix actor parser.
//! Untrusted URLs are data, never browser image sources or identity proof.
use serde_json::Value;

pub const MAX_EMOJIS: usize = 10;

#[derive(Clone, Debug, Default)]
pub struct ActorMedia {
    /// None means absent; Some invalid URL is a failed image, not deletion.
    pub avatar: Option<String>,
    pub emojis: Vec<(String, String)>,
}

fn image_url(value: &Value) -> Option<String> {
    value.as_str().or_else(|| value["url"].as_str()).map(|s| {
        // Reject oversized URLs later without retaining remote megabytes.
        if s.len() > 2048 {
            String::new()
        } else {
            s.to_owned()
        }
    })
}

pub fn parse(actor: &Value) -> ActorMedia {
    let avatar = if actor["icon"].is_null() {
        None
    } else {
        Some(image_url(&actor["icon"]).unwrap_or_default())
    };
    let mut emojis = Vec::new();
    if let Some(tags) = actor["tag"].as_array() {
        for tag in tags {
            if tag["type"] != "Emoji" {
                continue;
            }
            let Some(name) = tag["name"]
                .as_str()
                .and_then(|s| s.strip_prefix(':'))
                .and_then(|s| s.strip_suffix(':'))
            else {
                continue;
            };
            if !crate::backend::media::emoji_name(name) || emojis.iter().any(|(n, _)| n == name) {
                continue;
            }
            emojis.push((name.to_owned(), image_url(&tag["icon"]).unwrap_or_default()));
            if emojis.len() == MAX_EMOJIS {
                break;
            }
        }
    }
    ActorMedia { avatar, emojis }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_exact_shortcodes_and_failed_urls_do_not_become_deletions() {
        let mut tags = vec![
            serde_json::json!({"type":"Emoji","name":":wave:","icon":{"url":"https://cdn.example.org/wave.gif"}}),
        ];
        tags.push(tags[0].clone());
        tags.push(serde_json::json!({"type":"Emoji","name":":../secret:","icon":{"url":"x"}}));
        for n in 0..20 {
            tags.push(
                serde_json::json!({"type":"Emoji","name":format!(":e{n}:"),"icon":{"url":"x"}}),
            );
        }
        let media = parse(&serde_json::json!({"icon":{},"tag":tags}));
        assert_eq!(media.avatar, Some(String::new()));
        assert_eq!(media.emojis.len(), 10);
        assert_eq!(media.emojis[0].0, "wave");
        assert!(parse(&serde_json::json!({})).avatar.is_none());
    }
}
