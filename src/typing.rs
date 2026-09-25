use dioxus::prelude::*;
use unicode_segmentation::UnicodeSegmentation;

// Unicode Hangul syllables: AC00 + (initial * 21 + vowel) * 28 + final.
// https://www.unicode.org/reports/tr15/#Hangul
// This is a syllable-by-syllable typing preview, not an input method editor.
fn syllable_frames(syllable: char) -> Vec<char> {
    let index = syllable as u32 - 0xac00;
    let initial = index / 588;
    let vowel = index % 588 / 28;
    let final_part = index % 28;
    let initials = [
        'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ',
        'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
    ];
    let compose = |v, t| char::from_u32(0xac00 + (initial * 21 + v) * 28 + t).unwrap();
    let mut frames = vec![initials[initial as usize]];
    let vowel_start = match vowel {
        9..=11 => Some(8),   // ㅗ → ㅘ / ㅙ / ㅚ
        14..=16 => Some(13), // ㅜ → ㅝ / ㅞ / ㅟ
        19 => Some(18),      // ㅡ → ㅢ
        _ => None,
    };
    if let Some(v) = vowel_start {
        frames.push(compose(v, 0));
    }
    frames.push(compose(vowel, 0));
    let final_start = match final_part {
        3 => Some(1),      // ㄱ → ㄳ
        5 | 6 => Some(4),  // ㄴ → ㄵ / ㄶ
        9..=15 => Some(8), // ㄹ → ㄺ / ㄻ / ㄼ / ㄽ / ㄾ / ㄿ / ㅀ
        18 => Some(17),    // ㅂ → ㅄ
        _ => None,
    };
    if let Some(t) = final_start {
        frames.push(compose(vowel, t));
    }
    if final_part != 0 {
        frames.push(syllable);
    }
    frames
}

fn typing_frames(message: &str) -> Vec<String> {
    let mut frames = vec![String::new()];
    let mut committed = String::new();
    // Keep emoji, combining marks and other non-Hangul graphemes intact.
    for grapheme in message.graphemes(true) {
        let first = grapheme.chars().next().unwrap();
        if grapheme.chars().count() == 1 && ('가'..='힣').contains(&first) {
            for partial in syllable_frames(first) {
                frames.push(format!("{committed}{partial}"));
            }
        } else {
            frames.push(format!("{committed}{grapheme}"));
        }
        committed.push_str(grapheme);
    }
    frames
}

pub fn typed_text(message: &str, elapsed: u16, budget: u16) -> String {
    let frames = typing_frames(message);
    let steps = frames.len() - 1;
    // Longer text gets more time, but always finishes before the submit click.
    let duration = steps.max(27).min(usize::from(budget.max(1)));
    let elapsed = usize::from(elapsed.saturating_sub(5)).min(duration);
    frames[steps * elapsed / duration].clone()
}

#[component]
pub fn TypingPreview(
    text: String,
    class: String,
    label: String,
    placeholder: String,
    caret: bool,
) -> Element {
    let last = text
        .grapheme_indices(true)
        .last()
        .map_or(0, |(index, _)| index);
    let (prefix, tail) = text.split_at(last);
    rsx! {
        div { class: "{class} typing-preview", role: "textbox", aria_label: label,
            aria_readonly: "true", aria_multiline: "true", tabindex: "-1", "data-empty": text.is_empty(),
            span { class: "typing-text", "{prefix}" span { class: "typing-tail", "{tail}"
                if caret { span { class: "social-caret", aria_hidden: "true" } }
            } }
            if text.is_empty() { span { class: "typing-placeholder", aria_hidden: "true", "{placeholder}" } }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn korean_syllables_build_into_composed_characters() {
        assert_eq!(
            typing_frames("안녕"),
            ["", "ㅇ", "아", "안", "안ㄴ", "안녀", "안녕"]
        );
        assert_eq!(typing_frames("과"), ["", "ㄱ", "고", "과"]);
        assert_eq!(typing_frames("값"), ["", "ㄱ", "가", "갑", "값"]);
        assert_eq!(
            typing_frames("괜찮"),
            ["", "ㄱ", "고", "괘", "괜", "괜ㅊ", "괜차", "괜찬", "괜찮"]
        );
        assert_eq!(typing_frames("쐈"), ["", "ㅆ", "쏘", "쏴", "쐈"]);
    }

    #[test]
    fn every_modern_hangul_syllable_finishes_exactly() {
        for codepoint in 0xac00..=0xd7a3 {
            let syllable = char::from_u32(codepoint).unwrap();
            let frames = syllable_frames(syllable);
            assert_eq!(frames.last(), Some(&syllable));
            assert!(frames[1..].iter().all(|ch| ('가'..='힣').contains(ch)));
        }
    }

    #[test]
    fn non_hangul_graphemes_are_not_split() {
        assert_eq!(
            typing_frames("A 👩‍💻🇰🇷e\u{301}!"),
            [
                "",
                "A",
                "A ",
                "A 👩‍💻",
                "A 👩‍💻🇰🇷",
                "A 👩‍💻🇰🇷e\u{301}",
                "A 👩‍💻🇰🇷e\u{301}!"
            ]
        );
        assert_eq!(typing_frames("한\n글").last().unwrap(), "한\n글");
    }

    #[test]
    fn clock_supports_pause_rewind_and_submit_deadlines() {
        for message in [
            "",
            "안녕하세요 처음이에요",
            "늘 지나치던 골목에서 고양이를 만났어요. 오늘은 잠깐 멈춰 보기로 했습니다.",
        ] {
            for budget in [29, 42] {
                assert_eq!(typed_text(message, 0, budget), "");
                assert_eq!(typed_text(message, 5, budget), "");
                assert_eq!(typed_text(message, budget + 5, budget), message);
                assert_eq!(typed_text(message, 500, budget), message);
                assert_eq!(
                    typed_text(message, 15, budget),
                    typed_text(message, 15, budget)
                );
            }
        }
        assert_eq!(typed_text("안", 14, 27), "ㅇ");
        assert_eq!(typed_text("안", 23, 27), "아");
        assert_eq!(typed_text("안", 32, 27), "안");
    }
}
