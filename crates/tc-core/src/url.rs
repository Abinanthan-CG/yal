//! A small URL reader for `tinycast://` links.
//!
//! Upstream uses `URLComponents`. This does the same job for the one shape the
//! port needs: a scheme, a host, a path and a query, all percent-decoded.

use crate::deeplink::QueryItem;

/// The parts of a link the port needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    pub scheme: String,
    pub host: String,
    /// The path without its leading slash.
    pub path: String,
    pub query_items: Vec<QueryItem>,
}

impl ParsedUrl {
    /// Read a URL.
    ///
    /// `None` when it has no scheme, or its scheme is not followed by `://`.
    pub fn parse(url: &str) -> Option<ParsedUrl> {
        let url = url.trim();
        let separator = url.find("://")?;
        let scheme = &url[..separator];
        if scheme.is_empty()
            || !scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        {
            return None;
        }
        let rest = &url[separator + 3..];

        let (authority, remainder) = match rest.find(['/', '?', '#']) {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, ""),
        };
        let host = percent_decode(authority);

        let remainder = remainder.split('#').next().unwrap_or("");
        let (path, query) = match remainder.find('?') {
            Some(index) => (&remainder[..index], &remainder[index + 1..]),
            None => (remainder, ""),
        };
        let path = path.strip_prefix('/').unwrap_or(path);
        let path = percent_decode(path);

        Some(ParsedUrl {
            scheme: scheme.to_ascii_lowercase(),
            host,
            path,
            query_items: parse_query(query),
        })
    }
}

/// Split a query string into its items, percent-decoding names and values.
///
/// `+` is a literal plus, not a space, because upstream's `URLComponents` does
/// not treat it as one either.
fn parse_query(query: &str) -> Vec<QueryItem> {
    if query.is_empty() {
        return Vec::new();
    }
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((name, value)) => QueryItem {
                name: percent_decode(name),
                value: Some(percent_decode(value)),
            },
            None => QueryItem {
                name: percent_decode(pair),
                value: None,
            },
        })
        .collect()
}

/// Percent-decode a string, leaving an invalid escape as written.
pub fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                out.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encode a string with RFC 3986's unreserved set, so an encoded value
/// survives any reader of the link intact.
pub fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        let c = *byte as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
            out.push(c);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_link_has_an_empty_host_and_path() {
        let url = ParsedUrl::parse("tinycast://").unwrap();
        assert_eq!(url.scheme, "tinycast");
        assert_eq!(url.host, "");
        assert_eq!(url.path, "");
        assert!(url.query_items.is_empty());
    }

    #[test]
    fn a_host_and_path_are_separated() {
        let url = ParsedUrl::parse("tinycast://settings/general").unwrap();
        assert_eq!(url.host, "settings");
        assert_eq!(url.path, "general");
    }

    #[test]
    fn query_items_are_decoded() {
        let url = ParsedUrl::parse("tinycast://?fallbackText=hello%20world").unwrap();
        assert_eq!(
            url.query_items,
            vec![QueryItem {
                name: "fallbackText".to_string(),
                value: Some("hello world".to_string())
            }]
        );
    }

    #[test]
    fn a_plus_is_a_literal_plus() {
        let url = ParsedUrl::parse("tinycast://?fallbackText=a+b").unwrap();
        assert_eq!(url.query_items[0].value.as_deref(), Some("a+b"));
    }

    #[test]
    fn an_item_without_a_value_has_none() {
        let url = ParsedUrl::parse("tinycast://?flag").unwrap();
        assert_eq!(url.query_items[0].value, None);
    }

    #[test]
    fn encoding_keeps_only_the_unreserved_set() {
        assert_eq!(percent_encode("aZ0-._~"), "aZ0-._~");
        assert_eq!(percent_encode("a b"), "a%20b");
        assert_eq!(percent_encode("a/b"), "a%2Fb");
        assert_eq!(percent_encode("é"), "%C3%A9");
    }

    #[test]
    fn decoding_reverses_encoding() {
        for sample in ["hello world", "a/b", "é", "100%"] {
            assert_eq!(percent_decode(&percent_encode(sample)), sample);
        }
    }

    #[test]
    fn a_url_without_a_scheme_is_rejected() {
        assert!(ParsedUrl::parse("settings/general").is_none());
        assert!(ParsedUrl::parse("://host").is_none());
    }
}
