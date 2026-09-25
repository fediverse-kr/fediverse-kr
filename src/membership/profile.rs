use crate::media_ui::{ImageMark, emoji_source};
use dioxus::prelude::*;

#[derive(Debug, PartialEq)]
enum Segment {
    Text(String),
    Emoji(String),
}
fn segments(name: &str, emojis: &[String]) -> Vec<Segment> {
    let mut rest = name;
    let mut result = vec![];
    while let Some(start) = rest.find(':') {
        if start > 0 {
            result.push(Segment::Text(rest[..start].into()));
        }
        rest = &rest[start..];
        if let Some(end) = rest[1..].find(':').map(|i| i + 1) {
            let name = &rest[1..end];
            if emojis.iter().any(|e| e == name) {
                result.push(Segment::Emoji(name.into()));
                rest = &rest[end + 1..];
                continue;
            }
        }
        result.push(Segment::Text(":".into()));
        rest = &rest[1..];
    }
    if !rest.is_empty() {
        result.push(Segment::Text(rest.into()));
    }
    result
}
#[component]
pub fn PrivateIdentity(name: String, media: super::ProfileMedia) -> Element {
    rsx! {span{class:"profile-identity",
     if media.avatar_available{ImageMark{key:"avatar-{media.revision}",source:Some(format!("/api/member/avatar?v={}",media.revision)),fallback:name.chars().next().unwrap_or('·').to_string(),class:"profile-avatar"}}
     span{class:"profile-summary",strong{for segment in segments(&name,&media.emojis){match segment{
      Segment::Text(text)=>rsx!{"{text}"},
      Segment::Emoji(name)=>rsx!{ImageMark{key:"{name}-{media.revision}",source:emoji_source(&name,media.revision),fallback:format!(":{name}:"),class:"profile-emoji",size:20,decorative:false}},
     }}}span{class:"profile-caption","로그인되어 있어요."}}
    }}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emoji_names_are_exact_and_unknown_markup_stays_plain_text() {
        assert_eq!(
            segments("슈나 :wave: 안녕", &["wave".into()]),
            vec![
                Segment::Text("슈나 ".into()),
                Segment::Emoji("wave".into()),
                Segment::Text(" 안녕".into())
            ]
        );
        let raw = "<img> :missing: & 👩‍💻";
        let out = segments(raw, &[])
            .into_iter()
            .map(|s| match s {
                Segment::Text(t) => t,
                _ => panic!(),
            })
            .collect::<String>();
        assert_eq!(out, raw);
    }

    #[test]
    fn legacy_shortcodes_with_path_or_unicode_characters_remain_exact() {
        let name = "blob-cat.@/한국어";
        assert_eq!(
            segments(&format!(":{name}:"), &[name.into()]),
            vec![Segment::Emoji(name.into())]
        );
    }
}
