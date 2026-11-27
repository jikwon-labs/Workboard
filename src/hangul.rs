// Korean text written as separate letters (decomposed, "NFD") joined
// back into whole syllables. macOS file names and text typed on a Mac
// often reach the app this way (an Instagram caption, a file dropped from
// Finder), and egui draws each letter on its own: "ㅂㅏㅇㅣ" instead of
// "바이". This is the Unicode Hangul composition rule, nothing else.

use std::borrow::Cow;

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const S_COUNT: u32 = L_COUNT * V_COUNT * T_COUNT;

/// `s` with decomposed Hangul joined into syllables; borrowed unchanged
/// when there is nothing to join.
pub fn compose(s: &str) -> Cow<'_, str> {
    if !s.chars().any(|c| (0x1100..=0x11FF).contains(&(c as u32))) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut last: Option<char> = None;
    for c in s.chars() {
        let cu = c as u32;
        if let Some(p) = last {
            let pu = p as u32;
            // leading consonant + vowel -> syllable
            if (L_BASE..L_BASE + L_COUNT).contains(&pu) && (V_BASE..V_BASE + V_COUNT).contains(&cu)
            {
                let s = S_BASE + ((pu - L_BASE) * V_COUNT + (cu - V_BASE)) * T_COUNT;
                last = char::from_u32(s);
                continue;
            }
            // syllable without a final + final consonant -> syllable
            if (S_BASE..S_BASE + S_COUNT).contains(&pu)
                && (pu - S_BASE) % T_COUNT == 0
                && (T_BASE + 1..T_BASE + T_COUNT).contains(&cu)
            {
                last = char::from_u32(pu + (cu - T_BASE));
                continue;
            }
            out.push(p);
        }
        last = Some(c);
    }
    out.extend(last);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_decomposed_syllables() {
        // "바이브코더 특징" as macOS writes it
        let nfd =
            "\u{1107}\u{1161}\u{110B}\u{1175}\u{1107}\u{1173}\u{110F}\u{1169}\u{1103}\u{1165} \
                   \u{1110}\u{1173}\u{11A8}\u{110C}\u{1175}\u{11BC}";
        assert_eq!(compose(nfd), "바이브코더 특징");
        assert_eq!(
            compose("\u{1112}\u{1161}\u{11AB}\u{1109}\u{1161}\u{11BC}"),
            "한상"
        );
    }

    #[test]
    fn leaves_other_text_alone() {
        assert!(matches!(compose("한상훈 Han 韓 ㅋㅋ"), Cow::Borrowed(_)));
        // a lone final consonant after a syllable that already has one
        assert_eq!(compose("한\u{11A8}"), "한\u{11A8}");
        // a lone leading consonant stays
        assert_eq!(compose("\u{1100}a"), "\u{1100}a");
    }
}
