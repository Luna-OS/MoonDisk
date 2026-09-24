//! Validates the typed confirmation phrase required before a critical
//! (delete/format) operation executes. See `docs/safety-model.md` §8.
//!
//! Compared after Unicode NFC normalization and trimming, since the same
//! visible phrase can arrive with different underlying codepoints
//! depending on input method (composed vs. decomposed "Ö").

/// English fallback; German is the app's default UI language and default
/// phrase (`docs/safety-model.md` §8, decision D4).
pub const PHRASE_DE: &str = "LÖSCHEN";
pub const PHRASE_EN: &str = "DELETE";

pub fn expected_phrase(language: &str) -> &'static str {
    match language {
        "en" => PHRASE_EN,
        _ => PHRASE_DE,
    }
}

pub fn phrase_matches(input: &str, language: &str) -> bool {
    normalize(input) == normalize(expected_phrase(language))
}

fn normalize(s: &str) -> String {
    nfc_compose(s.trim())
}

// A small, dependency-free NFC composer covering exactly the one
// decomposition MoonDisk's confirmation phrase needs (Ö = O + combining
// diaeresis U+0308). A full Unicode normalization crate would be
// justified once more of the UI needs it; until then this keeps the
// dependency list minimal while still doing the composition correctly.
fn nfc_compose(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if chars.peek() == Some(&'\u{0308}') {
            let composed = match c {
                'O' => Some('Ö'),
                'o' => Some('ö'),
                'U' => Some('Ü'),
                'u' => Some('ü'),
                'A' => Some('Ä'),
                'a' => Some('ä'),
                _ => None,
            };
            if let Some(composed) = composed {
                out.push(composed);
                chars.next(); // consume the combining mark
                continue;
            }
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exact_phrase() {
        assert!(phrase_matches("LÖSCHEN", "de"));
        assert!(phrase_matches("DELETE", "en"));
    }

    #[test]
    fn accepts_decomposed_umlaut() {
        // "O" + combining diaeresis (U+0308), as some input methods emit.
        let decomposed = "L\u{4F}\u{308}SCHEN";
        assert!(phrase_matches(decomposed, "de"));
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert!(phrase_matches("  LÖSCHEN  ", "de"));
    }

    #[test]
    fn rejects_wrong_phrase() {
        assert!(!phrase_matches("LOESCHEN", "de"));
        assert!(!phrase_matches("delete", "en")); // case-sensitive by design
    }

    #[test]
    fn language_selects_the_right_expected_phrase() {
        assert!(!phrase_matches("DELETE", "de"));
        assert!(!phrase_matches("LÖSCHEN", "en"));
    }
}
