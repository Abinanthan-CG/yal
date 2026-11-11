//! What a name in another script gets typed as.
//!
//! Ported from upstream's
//! `Tinycast/Features/Launcher/Model/ScriptRomanization.swift`.
//!
//! Upstream uses ICU, which macOS ships and Windows does not. The rules are
//! therefore ported as explicit tables:
//!
//! | Script | Upstream | Here |
//! |---|---|---|
//! | Han | ICU `.mandarinToLatin` | The `pinyin` crate, toneless |
//! | Japanese | the kana only | A kana table |
//! | Cyrillic | an explicit BGN table | The same table |
//! | Hangul | ICU `.toLatin` | The Revised Romanization algorithm |
//! | everything else | ICU `.toLatin` | Greek, and nothing beyond it |
//!
//! A script with no rule is not romanized. The name is still an exact hit when
//! it is typed in its own script, which is what upstream promises too.

use crate::fuzzy::normalized;

/// The scripts worth their own rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Han,
    Japanese,
    Hangul,
    Cyrillic,
    Greek,
    Other,
}

/// The Latin reading of a name, or nothing when the name is already Latin.
///
/// The reading keeps one space between words, which is what lets both `wx`
/// and `weixin` reach `微信` (`wei xin`) through word starts.
pub fn latin(name: &str) -> Option<String> {
    let script = script_of(name)?;
    let reading = match script {
        Script::Han => han_to_latin(name),
        // Only the kana romanize: reading the kanji as Mandarin would be the
        // wrong word.
        Script::Japanese => japanese_to_latin(name),
        Script::Hangul => hangul_to_latin(name),
        Script::Cyrillic => cyrillic_reading(name),
        Script::Greek => greek_to_latin(name),
        Script::Other => return None,
    }?;

    let spaced = reading.split_whitespace().collect::<Vec<_>>().join(" ");
    let folded = normalized(&spaced);
    if folded.is_empty() || folded == normalized(name) {
        None
    } else {
        Some(folded)
    }
}

/// The first script with a rule of its own, kana before Han so a Japanese
/// title routes right.
pub fn script_of(name: &str) -> Option<Script> {
    let mut has_han = false;
    let mut has_hangul = false;
    let mut has_cyrillic = false;
    let mut has_greek = false;
    let mut has_other = false;

    for c in name.chars() {
        let code = c as u32;
        if is_kana(code) {
            return Some(Script::Japanese);
        }
        if is_han(code) {
            has_han = true;
        } else if is_hangul(code) {
            has_hangul = true;
        } else if is_cyrillic(code) {
            has_cyrillic = true;
        } else if is_greek(code) {
            has_greek = true;
        } else if code > 0x24F && c.is_alphabetic() {
            has_other = true;
        }
    }

    if has_han {
        Some(Script::Han)
    } else if has_hangul {
        Some(Script::Hangul)
    } else if has_cyrillic {
        Some(Script::Cyrillic)
    } else if has_greek {
        Some(Script::Greek)
    } else if has_other {
        Some(Script::Other)
    } else {
        None
    }
}

/// The Han reading: a toneless pinyin syllable per character.
fn han_to_latin(name: &str) -> Option<String> {
    use pinyin::ToPinyin;

    let mut tokens: Vec<String> = Vec::new();
    let mut literal = String::new();
    let mut read_any = false;

    for (c, syllable) in name.chars().zip(name.to_pinyin()) {
        match syllable {
            Some(pinyin) => {
                if !literal.is_empty() {
                    tokens.push(std::mem::take(&mut literal));
                }
                tokens.push(pinyin.plain().to_string());
                read_any = true;
            }
            None => literal.push(c),
        }
    }
    if !literal.is_empty() {
        tokens.push(literal);
    }
    // A script the table cannot read is not romanized at all: the name stays
    // an exact hit in its own script.
    if !read_any || tokens.is_empty() {
        return None;
    }
    Some(tokens.join(" "))
}

/// What the reading is currently made of, so a script boundary gets a space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    None,
    Latin,
    Kana,
}

/// The Japanese reading: the kana only, and the kanji dropped.
///
/// Upstream drops the kanji and hands the rest to ICU, which reads the katakana
/// as its hiragana twin and doubles the consonant after a small tsu.
fn japanese_to_latin(name: &str) -> Option<String> {
    let mut out = String::new();
    let mut kind = Kind::None;
    let mut pending_geminate = false;

    for c in name.chars() {
        let code = c as u32;
        if is_han(code) {
            // The kanji are dropped rather than read as Mandarin: it would be
            // a wrong word.
            continue;
        }
        if is_kana(code) {
            if code == SMALL_TSU || code == SMALL_TSU_KATAKANA {
                // The small tsu doubles the next consonant.
                pending_geminate = true;
                continue;
            }
            let Some(reading) = kana_reading(code) else {
                // A combining mark or the prolonged sound mark: dropped.
                continue;
            };
            if kind == Kind::Latin {
                out.push(' ');
            }
            if pending_geminate {
                out.push_str(&geminate(reading));
                pending_geminate = false;
            } else {
                out.push_str(reading);
            }
            kind = Kind::Kana;
            continue;
        }
        if c.is_alphanumeric() {
            if kind == Kind::Kana {
                out.push(' ');
            }
            out.push(c);
            kind = Kind::Latin;
        } else {
            out.push(' ');
            kind = Kind::None;
        }
    }

    if out.trim().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// The reading of a kana, with the katakana read as its hiragana twin.
fn kana_reading(code: u32) -> Option<&'static str> {
    let hiragana = if (0x30A1..=0x30F6).contains(&code) {
        code - 0x60
    } else {
        code
    };
    match hiragana {
        // The small kana outside the table.
        0x3095 => Some("ka"),
        0x3096 => Some("ke"),
        _ => {
            if !(0x3041..=0x308F).contains(&hiragana) {
                return None;
            }
            KANA.get((hiragana - 0x3041) as usize).copied()
        }
    }
}

/// The reading with its first consonant doubled, as a small tsu does.
///
/// The romanizations are not phonemic — `shi`, `chi` and `tsu` are one
/// consonant each — so the onset is looked up rather than guessed from the
/// first letter.
fn geminate(reading: &str) -> String {
    const ONSETS: [&str; 21] = [
        "ch", "sh", "ts", "k", "g", "s", "z", "t", "d", "n", "h", "f", "b", "p", "m", "y", "r",
        "w", "j", "v", "l",
    ];
    for onset in ONSETS {
        if let Some(rest) = reading.strip_prefix(onset) {
            return format!("{onset}{onset}{rest}");
        }
    }
    reading.to_string()
}

/// The Hangul reading: the Revised Romanization.
fn hangul_to_latin(name: &str) -> Option<String> {
    let mut out = String::new();
    for c in name.chars() {
        let code = c as u32;
        if !(0xAC00..=0xD7A3).contains(&code) {
            if c.is_alphanumeric() {
                out.push(c);
            } else {
                out.push(' ');
            }
            continue;
        }
        let syllable = code - 0xAC00;
        let initial = (syllable / 588) as usize;
        let medial = ((syllable % 588) / 28) as usize;
        let final_ = (syllable % 28) as usize;

        out.push_str(HANGUL_INITIAL[initial]);
        out.push_str(HANGUL_MEDIAL[medial]);
        if final_ != 0 {
            out.push_str(HANGUL_FINAL[final_]);
        }
    }
    if out.trim().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// The Cyrillic reading.
///
/// Upstream uses an explicit table because ICU's is scientific: `Яндекс`
/// becomes `Ândeks`, never the `yandex` users type.
fn cyrillic_reading(name: &str) -> Option<String> {
    let mut out = String::new();
    // The table is lower case, so the name is folded down first.
    for c in name.to_lowercase().chars() {
        if let Some(latin) = cyrillic_latin(c) {
            out.push_str(latin);
        } else if c.is_alphanumeric() {
            out.push(c);
        } else {
            out.push(' ');
        }
    }
    if out.trim().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// The Greek reading.
///
/// Upstream hands Greek to ICU's generic transform; Windows has no ICU, so the
/// same letters are spelled out here.
fn greek_to_latin(name: &str) -> Option<String> {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if let Some(latin) = greek_latin(c) {
            out.push_str(latin);
        } else if c.is_alphanumeric() {
            out.push(c);
        } else {
            out.push(' ');
        }
    }
    if out.trim().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// The Cyrillic table, from upstream.
fn cyrillic_latin(c: char) -> Option<&'static str> {
    Some(match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'ґ' => "g",
        'д' => "d",
        'е' => "e",
        'ё' => "yo",
        'є' => "ye",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'і' => "i",
        'ї' => "yi",
        'й' => "y",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ў' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ъ' => "",
        'ы' => "y",
        'ь' => "",
        'э' => "e",
        'ю' => "yu",
        'я' => "ya",
        _ => return None,
    })
}

/// The Greek table.
fn greek_latin(c: char) -> Option<&'static str> {
    Some(match c {
        'α' => "a",
        'β' => "v",
        'γ' => "g",
        'δ' => "d",
        'ε' => "e",
        'ζ' => "z",
        'η' => "i",
        'θ' => "th",
        'ι' => "i",
        'κ' => "k",
        'λ' => "l",
        'μ' => "m",
        'ν' => "n",
        'ξ' => "x",
        'ο' => "o",
        'π' => "p",
        'ρ' => "r",
        'σ' | 'ς' => "s",
        'τ' => "t",
        'υ' => "y",
        'φ' => "f",
        'χ' => "ch",
        'ψ' => "ps",
        'ω' => "o",
        'ά' => "a",
        'έ' => "e",
        'ή' => "i",
        'ί' => "i",
        'ό' => "o",
        'ύ' => "y",
        'ώ' => "o",
        'ϊ' | 'ΐ' => "i",
        'ΰ' => "y",
        _ => return None,
    })
}

/// Whether a character is kana.
fn is_kana(code: u32) -> bool {
    (0x3040..=0x309F).contains(&code)
        || (0x30A0..=0x30FF).contains(&code)
        || (0x31F0..=0x31FF).contains(&code)
}

/// Whether a character is Han.
fn is_han(code: u32) -> bool {
    (0x3400..=0x4DBF).contains(&code)
        || (0x4E00..=0x9FFF).contains(&code)
        || (0xF900..=0xFAFF).contains(&code)
        || (0x20000..=0x2FA1F).contains(&code)
}

/// Whether a character is Hangul.
fn is_hangul(code: u32) -> bool {
    (0x1100..=0x11FF).contains(&code)
        || (0x3130..=0x318F).contains(&code)
        || (0xA960..=0xA97F).contains(&code)
        || (0xAC00..=0xD7A3).contains(&code)
}

/// Whether a character is Cyrillic.
fn is_cyrillic(code: u32) -> bool {
    (0x0400..=0x04FF).contains(&code) || (0x0500..=0x052F).contains(&code)
}

/// Whether a character is Greek.
fn is_greek(code: u32) -> bool {
    (0x0370..=0x03FF).contains(&code) || (0x1F00..=0x1FFF).contains(&code)
}

/// The kana readings, from `あ` at U+3041.
///
/// The order is the Unicode order, so a character's index in the table is its
/// offset from U+3041.
const KANA: [&str; 79] = [
    "a", "a", "i", "i", "u", "u", "e", "e", "o", "o", // 0-9  あいうえお
    "ka", "ga", "ki", "gi", "ku", "gu", "ke", "ge", "ko", "go", // 10-19  かきくけこ
    "sa", "za", "shi", "ji", "su", "zu", "se", "ze", "so", "zo", // 20-29  さしすせそ
    "ta", "da", "chi", "ji", "tsu", "tsu", "zu", "te", "de", "to", // 30-39  たちつてと
    "do", "na", "ni", "nu", "ne", "no", "ha", "ba", "pa", "hi", // 40-49  なにぬねのは
    "bi", "pi", "fu", "bu", "pu", "he", "be", "pe", "ho", "bo", // 50-59  ひふへほ
    "po", "ma", "mi", "mu", "me", "mo", "ya", "yu", "yo",
    "ra", // 60-69  まみむめもやゆよら
    "ri", "ru", "re", "ro", "wa", "wi", "we", "wo", "n", // 70-78  りるれろわゐゑをん
];

/// The small tsu, which doubles the next consonant.
const SMALL_TSU: u32 = 0x3063;

/// The small tsu in katakana, which does the same.
const SMALL_TSU_KATAKANA: u32 = 0x30C3;

/// The Hangul initial consonants, in the Revised Romanization.
const HANGUL_INITIAL: [&str; 19] = [
    "g", "kk", "n", "d", "kk", "r", "m", "b", "pp", "s", "ss", "", "j", "jj", "ch", "k", "t", "p",
    "h",
];

/// The Hangul medial vowels, in the Revised Romanization.
const HANGUL_MEDIAL: [&str; 21] = [
    "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo", "we",
    "wi", "yu", "eu", "ui", "i",
];

/// The Hangul final consonants, in the Revised Romanization.
///
/// The Unicode order is 27 codas plus the empty one; a cluster reads as the
/// consonant it starts with.
const HANGUL_FINAL: [&str; 28] = [
    "", "k", "kk", "k", "n", "n", "n", "t", "l", "lk", "l", "l", "l", "l", "l", "l", "m", "p", "p",
    "t", "t", "ng", "j", "ch", "k", "t", "p", "t",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn han_reads_as_spaced_pinyin() {
        assert_eq!(latin("微信").as_deref(), Some("wei xin"));
    }

    #[test]
    fn a_longer_han_name_keeps_its_syllables() {
        assert_eq!(latin("网易云音乐").as_deref(), Some("wang yi yun yin le"));
    }

    #[test]
    fn mixed_script_keeps_the_latin_word() {
        assert_eq!(latin("Safari浏览器").as_deref(), Some("safari liu lan qi"));
    }

    #[test]
    fn cyrillic_reads_the_way_users_type_it() {
        assert_eq!(latin("Телеграм").as_deref(), Some("telegram"));
    }

    #[test]
    fn a_cyrillic_name_reads_the_way_the_docs_say() {
        // The upstream documentation names this one explicitly.
        assert_eq!(latin("Яндекс").as_deref(), Some("yandeks"));
    }

    #[test]
    fn the_kana_romanize_and_the_kanji_drop() {
        assert_eq!(latin("メモ帳").as_deref(), Some("memo"));
    }

    #[test]
    fn a_small_tsu_doubles_the_next_consonant() {
        assert_eq!(latin("きって").as_deref(), Some("kitte"));
    }

    #[test]
    fn a_name_already_in_latin_is_not_romanized() {
        // Upstream reads a Latin name through the fold, not through a script
        // rule, so there is no reading to hand back.
        assert_eq!(latin("Café Noir"), None);
        assert_eq!(latin("Visual Studio Code"), None);
        assert_eq!(latin("ｃａｆｅ"), None);
        assert_eq!(latin("\u{200E}Safari"), None);
    }

    #[test]
    fn a_name_that_is_already_latin_is_not_romanized() {
        assert_eq!(latin("Safari"), None);
    }

    #[test]
    fn a_script_routes_to_its_own_rule() {
        assert_eq!(script_of("微信"), Some(Script::Han));
        assert_eq!(script_of("メモ"), Some(Script::Japanese));
        assert_eq!(script_of("Телеграм"), Some(Script::Cyrillic));
        assert_eq!(script_of("안녕"), Some(Script::Hangul));
        assert_eq!(script_of("Safari"), None);
    }

    #[test]
    fn hangul_reads_as_revised_romanization() {
        assert_eq!(latin("안녕하세요").as_deref(), Some("annyeonghaseyo"));
    }

    #[test]
    fn greek_reads_as_latin() {
        assert_eq!(latin("Ελληνικά").as_deref(), Some("ellinika"));
    }
}
