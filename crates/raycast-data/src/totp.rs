//! Time-based one-time passwords.
//!
//! Implements RFC 6238 over RFC 4226, with the HMAC-SHA1 variant that every
//! authenticator application uses by default.

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;

/// The default number of digits in a code.
pub const DEFAULT_DIGITS: u32 = 6;

/// The default time step in seconds.
pub const DEFAULT_PERIOD: u64 = 30;

/// A TOTP secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpSecret {
    /// The decoded key bytes.
    pub key: Vec<u8>,
    /// The secret as it was entered, for display.
    pub label: String,
}

impl TotpSecret {
    /// Parse a base32 secret.
    ///
    /// Padding is optional and the alphabet is case-insensitive, matching
    /// what authenticator applications accept.
    pub fn parse(secret: &str) -> Option<Self> {
        let key = decode_base32(secret)?;
        if key.is_empty() {
            return None;
        }
        Some(Self {
            key,
            label: secret.to_string(),
        })
    }

    /// The code for a given time.
    pub fn code_at(&self, unix_seconds: u64, digits: u32, period: u64) -> String {
        let counter = unix_seconds / period;
        let code = hotp(&self.key, counter);
        format!(
            "{:0width$}",
            code % 10u32.pow(digits),
            width = digits as usize
        )
    }

    /// The code for a given time, and the seconds until it changes.
    pub fn code_and_remaining(&self, unix_seconds: u64, digits: u32, period: u64) -> (String, u64) {
        let remaining = period - (unix_seconds % period);
        (
            self.code_at(unix_seconds, digits, period),
            remaining,
        )
    }
}

/// The counter-based one-time password.
fn hotp(key: &[u8], counter: u64) -> u32 {
    // HMAC accepts a key of any length.
    let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();

    // Dynamic truncation: the low four bits of the last byte select the
    // offset of the four-byte window.
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3])
}

/// Decode a base32 string into bytes.
fn decode_base32(text: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    let mut buffer = 0u32;
    let mut bits = 0u32;
    let mut bytes = Vec::new();

    for character in text.chars() {
        if character == '=' || character == ' ' || character == '-' {
            continue;
        }
        let upper = character.to_ascii_uppercase() as u8;
        let index = ALPHABET.iter().position(|&c| c == upper)? as u32;
        buffer = (buffer << 5) | index;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            bytes.push(((buffer >> bits) & 0xff) as u8);
        }
    }

    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_base32() {
        // RFC 4648 base32 test vectors.
        assert_eq!(decode_base32("MZXW6YTBOI").unwrap(), b"foobar");
        assert_eq!(decode_base32("MZXW6YTB").unwrap(), b"fooba");
        assert_eq!(decode_base32("MY").unwrap(), b"f");
        assert_eq!(decode_base32("").unwrap(), b"");
    }

    #[test]
    fn base32_ignores_padding_and_case() {
        assert_eq!(
            decode_base32("mzxw6ytboi").unwrap(),
            decode_base32("MZXW6YTBOI").unwrap()
        );
        assert_eq!(
            decode_base32("MZXW6YTBOI======").unwrap(),
            decode_base32("MZXW6YTBOI").unwrap()
        );
    }

    #[test]
    fn matches_the_rfc_6238_test_vectors() {
        // RFC 6238 appendix B, the SHA-1 column, with the twenty-byte
        // secret "12345678901234567890". The published codes are eight
        // digits; the underlying value is taken modulo ten to the eighth.
        let key = b"12345678901234567890";
        let vectors: [(u64, u32); 6] = [
            (59, 94_287_082),
            (1_111_111_109, 7_081_804),
            (1_111_111_111, 14_050_471),
            (1_234_567_890, 89_005_924),
            (2_000_000_000, 69_279_037),
            (20_000_000_000, 65_353_130),
        ];
        for (time, expected) in vectors {
            let code = hotp(key, time / 30) % 100_000_000;
            assert_eq!(code, expected, "TOTP for time {time}");
        }
    }

    #[test]
    fn matches_the_rfc_6238_vectors_through_the_secret_type() {
        let secret = TotpSecret::parse("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        // The RFC's base32 secret for the SHA-1 vectors.
        let (code, _) = secret.code_and_remaining(59, 8, 30);
        assert_eq!(code, "94287082");
    }

    #[test]
    fn a_secret_produces_a_six_digit_code() {
        let secret = TotpSecret::parse("JBSWY3DPEHPK3PXP").unwrap();
        let code = secret.code_at(59, DEFAULT_DIGITS, DEFAULT_PERIOD);
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn reports_the_seconds_remaining() {
        let secret = TotpSecret::parse("JBSWY3DPEHPK3PXP").unwrap();
        let (_, remaining) = secret.code_and_remaining(65, DEFAULT_DIGITS, 30);
        assert_eq!(remaining, 25);
    }

    #[test]
    fn rejects_an_empty_secret() {
        assert!(TotpSecret::parse("").is_none());
    }
}
