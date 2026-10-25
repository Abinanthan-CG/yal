//! Text transforms: the small conversions a launcher is asked for a dozen
//! times a day.
//!
//! The target's text transform commands have no network and no database; they
//! are pure functions, which is what this module is.

/// A failure while transforming text.
#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("/{0} is not a transform this launcher knows")]
    Unknown(String),

    #[error("the transform could not be done: {0}")]
    Failed(String),
}

/// Every transform, with the words that find it in the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transform {
    pub slug: &'static str,
    pub title: &'static str,
    pub keywords: &'static [&'static str],
}

/// The set of transforms.
pub const TRANSFORMS: &[Transform] = &[
    Transform {
        slug: "upper",
        title: "UPPERCASE",
        keywords: &["uppercase", "upper", "shout", "caps"],
    },
    Transform {
        slug: "lower",
        title: "lowercase",
        keywords: &["lowercase", "lower", "quiet"],
    },
    Transform {
        slug: "title",
        title: "Title Case",
        keywords: &["title", "titlecase", "heading"],
    },
    Transform {
        slug: "sentence",
        title: "Sentence case",
        keywords: &["sentence", "sentencecase"],
    },
    Transform {
        slug: "camel",
        title: "camelCase",
        keywords: &["camel", "camelcase", "code"],
    },
    Transform {
        slug: "snake",
        title: "snake_case",
        keywords: &["snake", "snakecase", "underscore"],
    },
    Transform {
        slug: "kebab",
        title: "kebab-case",
        keywords: &["kebab", "kebabcase", "hyphen", "dash"],
    },
    Transform {
        slug: "pascal",
        title: "PascalCase",
        keywords: &["pascal", "pascalcase"],
    },
    Transform {
        slug: "reverse",
        title: "esrever",
        keywords: &["reverse", "backwards"],
    },
    Transform {
        slug: "trim",
        title: "Trim whitespace",
        keywords: &["trim", "strip", "whitespace", "clean"],
    },
    Transform {
        slug: "words",
        title: "Word count",
        keywords: &["words", "count", "length"],
    },
    Transform {
        slug: "lines",
        title: "Line count",
        keywords: &["lines", "count"],
    },
    Transform {
        slug: "sort-lines",
        title: "Sort lines alphabetically",
        keywords: &["sort", "alphabetical", "lines"],
    },
    Transform {
        slug: "unique-lines",
        title: "Remove duplicate lines",
        keywords: &["unique", "dedupe", "duplicate", "lines"],
    },
    Transform {
        slug: "reverse-lines",
        title: "Reverse line order",
        keywords: &["reverse", "lines", "order"],
    },
    Transform {
        slug: "b64encode",
        title: "Base64 encode",
        keywords: &["base64", "encode", "b64"],
    },
    Transform {
        slug: "b64decode",
        title: "Base64 decode",
        keywords: &["base64", "decode", "b64"],
    },
    Transform {
        slug: "urlencode",
        title: "URL percent-encode",
        keywords: &["url", "encode", "percent"],
    },
    Transform {
        slug: "urldecode",
        title: "URL percent-decode",
        keywords: &["url", "decode", "unpercent"],
    },
    Transform {
        slug: "json-escape",
        title: "JSON string escape",
        keywords: &["json", "escape", "string"],
    },
    Transform {
        slug: "json-unescape",
        title: "JSON string unescape",
        keywords: &["json", "unescape", "string"],
    },
    Transform {
        slug: "hexencode",
        title: "Hex string encode",
        keywords: &["hex", "encode", "bytes"],
    },
    Transform {
        slug: "hexdecode",
        title: "Hex string decode",
        keywords: &["hex", "decode", "bytes"],
    },
    Transform {
        slug: "morseencode",
        title: "Morse code",
        keywords: &["morse", "dots", "beep"],
    },
    Transform {
        slug: "uuid",
        title: "Generate UUID",
        keywords: &["uuid", "guid", "identifier"],
    },
    Transform {
        slug: "lorem",
        title: "Placeholder text",
        keywords: &["lorem", "ipsum", "placeholder", "dummy", "filler"],
    },
];

/// Apply one transform to some text.
///
/// The slug is the short name in [`TRANSFORMS`].
pub fn apply(slug: &str, text: &str) -> Result<String, TextError> {
    let result = match slug {
        "upper" => text.to_uppercase(),
        "lower" => text.to_lowercase(),
        "title" => title_case(text),
        "sentence" => sentence_case(text),
        "camel" => camel_case(text),
        "snake" => join_words(text, Case::Lower, "_"),
        "kebab" => join_words(text, Case::Lower, "-"),
        "pascal" => join_words(text, Case::Upper, ""),
        "reverse" => text.chars().rev().collect(),
        "trim" => text.trim().to_string(),
        "words" => text.split_whitespace().count().to_string(),
        "lines" => text.lines().count().to_string(),
        "sort-lines" => {
            let mut lines: Vec<&str> = text.lines().collect();
            lines.sort_by_key(|line| line.to_lowercase());
            lines.join("\n")
        }
        "unique-lines" => {
            let mut seen = std::collections::BTreeSet::new();
            let mut lines = Vec::new();
            for line in text.lines() {
                if seen.insert(line.to_lowercase()) {
                    lines.push(line);
                }
            }
            lines.join("\n")
        }
        "reverse-lines" => text.lines().rev().collect::<Vec<_>>().join("\n"),
        "b64encode" => base64_encode(text.as_bytes()),
        "b64decode" => base64_decode_ok(text)?,
        "urlencode" => url_encode(text),
        "urldecode" => url_decode_ok(text)?,
        "json-escape" => json_escape(text),
        "json-unescape" => json_unescape(text)?,
        "hexencode" => hex_encode(text.as_bytes()),
        "hexdecode" => hex_decode(text)?,
        "morseencode" => morse_encode(text),
        "uuid" => uuid(),
        "lorem" => {
            let paragraphs = text.trim().parse::<usize>().unwrap_or(1).clamp(1, 20);
            lorem_ipsum(paragraphs)
        }
        other => return Err(TextError::Unknown(other.to_string())),
    };
    Ok(result)
}

/// Whether the first letters of the words are upper- or lower-case.
#[derive(Clone, Copy)]
enum Case {
    Lower,
    Upper,
}

/// Split text into words on spaces, punctuation and case changes.
fn words(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut current = String::new();
    let mut last_was_word = false;

    for ch in text.chars() {
        if ch.is_alphanumeric() {
            let upper = ch.is_uppercase();
            if upper && last_was_word && !current.is_empty() {
                found.push(std::mem::take(&mut current));
            }
            current.push(ch);
            last_was_word = true;
        } else if !current.is_empty() {
            found.push(std::mem::take(&mut current));
            last_was_word = false;
        }
    }
    if !current.is_empty() {
        found.push(current);
    }
    found
}

/// Capitalise every word after the first, leaving the first lower-case.
fn camel_case(text: &str) -> String {
    words(text)
        .iter()
        .enumerate()
        .map(|(index, word)| {
            if index == 0 {
                word.to_lowercase()
            } else {
                lower_then_upper(word)
            }
        })
        .collect()
}

/// Lower-case a word, then raise its first letter.
fn lower_then_upper(word: &str) -> String {
    let mut out = word.to_lowercase();
    if let Some(first) = out.chars().next() {
        let upper: String = first.to_uppercase().collect();
        out = upper + &out[first.len_utf8()..];
    }
    out
}

/// Put the words back together with a separator and the given case.
fn join_words(text: &str, case: Case, separator: &str) -> String {
    words(text)
        .iter()
        .map(|word| match case {
            Case::Lower => word.to_lowercase(),
            Case::Upper => lower_then_upper(word),
        })
        .collect::<Vec<_>>()
        .join(separator)
}

/// Capitalise every word, lower-casing the rest.
fn title_case(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            let mut letters = word.chars();
            match letters.next() {
                Some(first) => first.to_uppercase().collect::<String>()
                    + &letters.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Capitalise the first letter of the text and lower-case the rest.
fn sentence_case(text: &str) -> String {
    let trimmed = text.trim();
    let mut letters = trimmed.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &letters.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// Standard base64.
pub fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

/// Decode base64, reporting laziness as a real error.
fn base64_decode_ok(text: &str) -> Result<String, TextError> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut buffer = 0u32;
    let mut bits = 0u32;
    let mut bytes = Vec::new();

    for (index, ch) in text.chars().enumerate() {
        if ch == '=' || ch.is_whitespace() {
            continue;
        }
        let Some(position) = ALPHABET.iter().position(|&c| c == ch as u8) else {
            return Err(TextError::Failed(format!("'{ch}' is not base64")));
        };
        let _ = index;
        buffer = (buffer << 6) | position as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    String::from_utf8(bytes).map_err(|err| TextError::Failed(err.to_string()))
}

/// Percent-encode for a URL query or path.
pub fn url_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Decode percent-encoded text.
fn url_decode_ok(text: &str) -> Result<String, TextError> {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok());
            match hex {
                Some(byte) => {
                    out.push(byte);
                    index += 3;
                    continue;
                }
                None => {
                    return Err(TextError::Failed(format!(
                        "the escape at {index} is not hexadecimal"
                    )))
                }
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).map_err(|err| TextError::Failed(err.to_string()))
}

/// Escape a string for use inside a JSON document.
pub fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for ch in text.chars() {
        match ch {
            '\"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Unescape a JSON string body back into text.
fn json_unescape(text: &str) -> Result<String, TextError> {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('\"') => out.push('\"'),
            Some('\\') => out.push('\\'),
            Some('/') => out.push('/'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('b') => out.push('\u{0008}'),
            Some('f') => out.push('\u{000C}'),
            Some('u') => {
                let hex: String = (0..4)
                    .filter_map(|_| chars.next())
                    .collect();
                let code = u32::from_str_radix(&hex, 16)
                    .map_err(|_| TextError::Failed("the \\u escape is malformed".into()))?;
                out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
            }
            Some(other) => return Err(TextError::Failed(format!("\\{other} is not an escape"))),
            None => return Err(TextError::Failed("the text ends mid-escape".into())),
        }
    }
    Ok(out)
}

/// Hex-encode bytes.
pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

/// Hex-decode a string.
fn hex_decode(text: &str) -> Result<String, TextError> {
    let cleaned: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    if cleaned.len() % 2 != 0 {
        return Err(TextError::Failed(
            "hexadecimal text must have an even number of digits".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(cleaned.len() / 2);
    let digits: Vec<char> = cleaned.chars().collect();
    for pair in digits.chunks(2) {
        let byte = u8::from_str_radix(&pair.iter().collect::<String>(), 16)
            .map_err(|err| TextError::Failed(err.to_string()))?;
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|err| TextError::Failed(err.to_string()))
}

/// The Morse alphabet.
const MORSE: &[(&str, &str)] = &[
    ("A", ".-"), ("B", "-..."), ("C", "-.-."), ("D", "-.."), ("E", "."),
    ("F", "..-."), ("G", "--."), ("H", "...."), ("I", ".."), ("J", ".---"),
    ("K", "-.-"), ("L", ".-.."), ("M", "--"), ("N", "-."), ("O", "---"),
    ("P", ".--."), ("Q", "--.-"), ("R", ".-."), ("S", "..."), ("T", "-"),
    ("U", "..-"), ("V", "...-"), ("W", ".--"), ("X", "-..-"), ("Y", "-.--"),
    ("Z", "--.."), ("0", "-----"), ("1", ".----"), ("2", "..---"),
    ("3", "...--"), ("4", "....-"), ("5", "....."), ("6", "-...."),
    ("7", "--..."), ("8", "---.."), ("9", "----."),
];

/// Render text as Morse code, one dot or dash per signal.
fn morse_encode(text: &str) -> String {
    let mut out = Vec::new();
    for word in text.to_uppercase().split_whitespace() {
        let mut letters = Vec::new();
        for ch in word.chars() {
            if let Some((_, code)) = MORSE.iter().find(|(letter, _)| *letter == ch.to_string()) {
                letters.push(code.to_string());
            }
        }
        if !letters.is_empty() {
            out.push(letters.join(" "));
        }
    }
    out.join(" / ")
}

/// A version 4 identifier.
///
/// The bytes come from the system source; the version and variant bits are
/// set to the values the specification fixes.
pub fn uuid() -> String {
    use rand::RngCore;

    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);

    // Version 4 lives in the top nibble of the third group; the variant
    // occupies the top two bits of the fourth.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let hex = |chunk: &[u8]| {
        chunk
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };

    format!(
        "{}-{}-{}-{}-{}",
        hex(&bytes[..4]),
        hex(&bytes[4..6]),
        hex(&bytes[6..8]),
        hex(&bytes[8..10]),
        hex(&bytes[10..16]),
    )
}

/// Cryptographic digests of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Digest {
    Sha256,
    Sha1,
    Md5,
}

use sha2::{Digest as _, Sha256};

/// Digest text with one of the hash primitives.
///
/// The weak digests are implemented and labelled: they are offered because
/// checking a known string against an old system is still something people
/// do, and the label tells them what they are looking at.
pub fn hash(digest: Digest, text: &str) -> Result<String, TextError> {
    match digest {
        Digest::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(text.as_bytes());
            Ok(hex_encode_lower(&hasher.finalize()))
        }
        Digest::Sha1 => {
            let mut hasher = sha1::Sha1::new();
            hasher.update(text.as_bytes());
            Ok(hex_encode_lower(&hasher.finalize()))
        }
        Digest::Md5 => {
            let mut hasher = md5::Md5::new();
            hasher.update(text.as_bytes());
            Ok(hex_encode_lower(&hasher.finalize()))
        }
    }
}

/// Hex-encode bytes in lower case, as digests are printed.
fn hex_encode_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A dose of placeholder text.
///
/// The words are the standard set, rewritten rather than copied from a
/// distribution package, for the places a layout needs filling.
pub fn lorem_ipsum(paragraphs: usize) -> String {
    const SENTENCES: &[&str] = &[
        "The quick brown fox jumps over the lazy dog.",
        "Pack my box with five dozen liquor jugs.",
        "How vexingly quick daft zebras jump.",
        "Bright vixens jump dozy fowl quack.",
        "Quick zephyrs blow vexing daft Jim.",
        "Two driven jocks help fax my big quiz.",
        "Five quacking zephyrs jolt my wax bed.",
        "Sphinx of black quartz judge my vow.",
        "Waltz bad nymph for quick jigs vex.",
        "Amazingly few discotheques provide jukeboxes.",
    ];

    if paragraphs == 0 {
        return String::new();
    }
    (0..paragraphs.max(1))
        .map(|paragraph| {
            (0..4)
                .map(|index| SENTENCES[(paragraph * 4 + index) % SENTENCES.len()])
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_transforms() {
        assert_eq!(apply("upper", "mixed Case").unwrap(), "MIXED CASE");
        assert_eq!(apply("lower", "mixed Case").unwrap(), "mixed case");
        assert_eq!(apply("title", "hello world").unwrap(), "Hello World");
        assert_eq!(apply("sentence", "HELLO WORLD").unwrap(), "Hello world");
    }

    #[test]
    fn identifier_transforms() {
        assert_eq!(apply("camel", "hello world").unwrap(), "helloWorld");
        assert_eq!(apply("snake", "Hello World").unwrap(), "hello_world");
        assert_eq!(apply("kebab", "Hello World").unwrap(), "hello-world");
        assert_eq!(apply("pascal", "hello world").unwrap(), "HelloWorld");
        assert_eq!(apply("snake", "HelloWorldTest").unwrap(), "hello_world_test");
    }

    #[test]
    fn line_transforms() {
        assert_eq!(apply("words", "one two three").unwrap(), "3");
        assert_eq!(apply("lines", "one\ntwo").unwrap(), "2");
        assert_eq!(
            apply("sort-lines", "b\na\nc").unwrap(),
            "a\nb\nc"
        );
        assert_eq!(apply("unique-lines", "a\nb\nA\nc").unwrap(), "a\nb\nc");
        assert_eq!(apply("reverse-lines", "a\nb\nc").unwrap(), "c\nb\na");
    }

    #[test]
    fn encodings_round_trip() {
        let text = "Hello, wörld! & <tag>";
        assert_eq!(
            apply("b64decode", &apply("b64encode", text).unwrap()).unwrap(),
            text
        );
        assert_eq!(
            apply("urldecode", &apply("urlencode", text).unwrap()).unwrap(),
            text
        );
        assert_eq!(
            apply("hexdecode", &apply("hexencode", text).unwrap()).unwrap(),
            text
        );
        assert_eq!(
            apply("json-unescape", &apply("json-escape", text).unwrap()).unwrap(),
            text
        );
    }

    #[test]
    fn base64_is_the_standard_vector() {
        // RFC 4648 vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    }

    #[test]
    fn decoding_reports_real_errors() {
        assert!(apply("b64decode", "not base64!").is_err());
        assert!(apply("b64decode", "Zm9v").is_ok());
        assert!(apply("hexdecode", "abc").is_err());
        assert!(apply("urldecode", "%zz").is_err());
    }

    #[test]
    fn percent_encoding_is_the_canonical_set() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn hashes() {
        assert_eq!(
            hash(Digest::Sha256, "abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hash(Digest::Sha256, "").unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // Self-check vectors from the specification documents.
        assert_eq!(
            hash(Digest::Sha1, "abc").unwrap(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hash(Digest::Md5, "abc").unwrap(),
            "900150983cd24fb0d6963f7d28e17f72"
        );
    }

    #[test]
    fn uuids_are_version_four() {
        let first = uuid();
        let second = uuid();
        assert_ne!(first, second);
        assert_eq!(first.len(), 36);
        assert_eq!(first.as_bytes()[14], b'4');
        assert!(matches!(
            first.as_bytes()[19],
            b'8' | b'9' | b'a' | b'b'
        ));
        // The dashes sit where the specification puts them.
        for index in [8usize, 13, 18, 23] {
            assert_eq!(first.as_bytes()[index], b'-');
        }
    }

    #[test]
    fn lorem_makes_the_paragraphs_asked_for() {
        assert_eq!(lorem_ipsum(0), "");
        assert_eq!(lorem_ipsum(2).split("\n\n").count(), 2);
        assert!(lorem_ipsum(1).len() > 100);
    }

    #[test]
    fn morse_knows_its_letters() {
        assert_eq!(apply("morseencode", "SOS").unwrap(), "... --- ...");
        assert_eq!(morse_encode("a"), ".-");
    }

    #[test]
    fn an_unknown_transform_is_reported() {
        assert!(matches!(
            apply("rot13", "text"),
            Err(TextError::Unknown(_))
        ));
    }

    #[test]
    fn every_transform_applies_without_panicking() {
        let text = "The quick brown fox.\nSecond line!";
        for transform in TRANSFORMS {
            // Encoding decoders fail on plain text; that is expected and not
            // a panic.
            let _ = apply(transform.slug, text);
        }
    }
}
