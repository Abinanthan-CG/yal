//! Stable identifiers and the UUID form upstream uses for stored items.

/// Whether a string is a UUID in the canonical 8-4-4-4-12 hexadecimal form.
///
/// Upstream's `UUID(uuidString:)` is strict about the shape and tolerant about
/// case, so a link naming a chat must use that shape exactly.
pub fn is_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        match index {
            8 | 13 | 18 | 23 => {
                if *byte != b'-' {
                    return false;
                }
            }
            _ => {
                if !byte.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

/// A UUID, normalised to the lowercase form upstream stores.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid(String);

impl Uuid {
    /// Parse a UUID, rejecting anything that is not in the canonical shape.
    pub fn parse(value: &str) -> Option<Uuid> {
        if is_uuid(value) {
            Some(Uuid(value.to_ascii_lowercase()))
        } else {
            None
        }
    }

    /// The lowercase form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Uuid {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_canonical_uuid_is_accepted() {
        assert!(is_uuid("3f2504e0-4f89-11d3-9a0c-0305e82c3301"));
        assert!(is_uuid("3F2504E0-4F89-11D3-9A0C-0305E82C3301"));
    }

    #[test]
    fn a_malformed_uuid_is_rejected() {
        assert!(!is_uuid(""));
        assert!(!is_uuid("3f2504e0-4f89-11d3-9a0c-0305e82c330"));
        assert!(!is_uuid("3f2504e04f8911d39a0c0305e82c3301"));
        assert!(!is_uuid("3f2504e0-4f89-11d3-9a0c-0305e82c330g"));
        assert!(!is_uuid("3f2504e0_4f89_11d3_9a0c_0305e82c3301"));
    }

    #[test]
    fn a_uuid_is_stored_lowercase() {
        let id = Uuid::parse("3F2504E0-4F89-11D3-9A0C-0305E82C3301").unwrap();
        assert_eq!(id.as_str(), "3f2504e0-4f89-11d3-9a0c-0305e82c3301");
    }
}
