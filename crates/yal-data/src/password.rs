//! Password generation.
//!
//! Generation is driven by a set of toggles rather than a fixed recipe, so
//! the caller decides which character classes are in play and how long the
//! result is. Every generated password is guaranteed to contain at least one
//! character from each enabled class.

use rand::Rng;

/// The character classes a password can draw from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordOptions {
    /// Total length.
    pub length: usize,
    /// Include lowercase letters.
    pub lowercase: bool,
    /// Include uppercase letters.
    pub uppercase: bool,
    /// Include digits.
    pub digits: bool,
    /// Include punctuation.
    pub symbols: bool,
    /// Exclude characters that are easy to confuse with one another.
    pub avoid_ambiguous: bool,
}

impl Default for PasswordOptions {
    fn default() -> Self {
        Self {
            length: 20,
            lowercase: true,
            uppercase: true,
            digits: true,
            symbols: true,
            avoid_ambiguous: false,
        }
    }
}

/// The lowercase alphabet.
pub const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
/// The uppercase alphabet.
pub const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// The digit alphabet.
pub const DIGITS: &str = "0123456789";
/// The symbol alphabet.
pub const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.?/";

/// Characters excluded when `avoid_ambiguous` is set.
const AMBIGUOUS: &[char] = &['0', 'O', 'o', 'l', 'I', 'i', '1'];

/// The alphabets in play for a set of options.
pub fn alphabets(options: &PasswordOptions) -> Vec<&'static str> {
    let mut sets: Vec<&'static str> = Vec::new();
    if options.lowercase {
        sets.push(LOWERCASE);
    }
    if options.uppercase {
        sets.push(UPPERCASE);
    }
    if options.digits {
        sets.push(DIGITS);
    }
    if options.symbols {
        sets.push(SYMBOLS);
    }
    sets
}

/// Generate a password.
///
/// Returns `None` when no character class is enabled, because there is no
/// password to generate in that case.
pub fn generate(options: &PasswordOptions) -> Option<String> {
    let sets = alphabets(options);
    if sets.is_empty() {
        return None;
    }

    let filter = |set: &str| -> String {
        if options.avoid_ambiguous {
            set.chars().filter(|c| !AMBIGUOUS.contains(c)).collect()
        } else {
            set.to_string()
        }
    };

    let filtered: Vec<String> = sets.iter().map(|set| filter(set)).collect();
    let usable: Vec<&String> = filtered.iter().filter(|set| !set.is_empty()).collect();
    if usable.is_empty() {
        return None;
    }

    let mut rng = rand::rng();
    let length = options.length.max(usable.len());

    // Seed one character from every enabled class so the result satisfies
    // the class requirement, then fill the rest from the combined alphabet.
    let mut characters: Vec<char> = usable
        .iter()
        .map(|set| {
            let chars: Vec<char> = set.chars().collect();
            chars[rng.random_range(0..chars.len())]
        })
        .collect();

    let combined: Vec<char> = usable.iter().flat_map(|set| set.chars()).collect();
    while characters.len() < length {
        characters.push(combined[rng.random_range(0..combined.len())]);
    }
    characters.truncate(length);

    // Shuffle so the seeded characters are not always at the front.
    for index in (1..characters.len()).rev() {
        let swap = rng.random_range(0..=index);
        characters.swap(index, swap);
    }

    Some(characters.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_the_requested_length() {
        let options = PasswordOptions {
            length: 24,
            ..Default::default()
        };
        let password = generate(&options).unwrap();
        assert_eq!(password.chars().count(), 24);
    }

    #[test]
    fn includes_every_enabled_class() {
        let options = PasswordOptions {
            length: 12,
            ..Default::default()
        };
        let password = generate(&options).unwrap();
        assert!(password.chars().any(|c| c.is_ascii_lowercase()));
        assert!(password.chars().any(|c| c.is_ascii_uppercase()));
        assert!(password.chars().any(|c| c.is_ascii_digit()));
        assert!(password
            .chars()
            .any(|c| SYMBOLS.contains(c)));
    }

    #[test]
    fn excludes_disabled_classes() {
        let options = PasswordOptions {
            length: 16,
            lowercase: false,
            uppercase: false,
            digits: true,
            symbols: false,
            avoid_ambiguous: false,
        };
        let password = generate(&options).unwrap();
        assert!(password.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn avoids_ambiguous_characters_when_asked() {
        let options = PasswordOptions {
            length: 64,
            avoid_ambiguous: true,
            ..Default::default()
        };
        let password = generate(&options).unwrap();
        assert!(!password.chars().any(|c| AMBIGUOUS.contains(&c)));
    }

    #[test]
    fn no_classes_means_no_password() {
        let options = PasswordOptions {
            length: 10,
            lowercase: false,
            uppercase: false,
            digits: false,
            symbols: false,
            avoid_ambiguous: false,
        };
        assert!(generate(&options).is_none());
    }

    #[test]
    fn successive_passwords_differ() {
        let options = PasswordOptions::default();
        let first = generate(&options).unwrap();
        let second = generate(&options).unwrap();
        assert_ne!(first, second);
    }
}
