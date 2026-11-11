//! What a quicklink points at, worked out from its shape alone.
//!
//! Ported from upstream's
//! `Tinycast/Features/Quicklinks/Model/QuicklinkDestination.swift`.
//!
//! A user writes what they mean: a path, a web page, a share, a link into
//! another application, or a bare domain they wrote down from memory. The
//! shape of the text says which it is, and nothing else is asked.

use url::Url;

/// What a resolved link points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuicklinkDestination {
    /// A web page.
    Web(Url),
    /// A mountable share: `smb://`, `afp://`, `nfs://`, `ftp://`, `sftp://`,
    /// `ftps://`.
    Network(Url),
    /// Any other registered scheme, such as `mailto:`.
    Deeplink(Url),
    /// An absolute local path, tilde already expanded.
    Path(String),
}

impl QuicklinkDestination {
    /// The schemes that name a mountable share.
    const NETWORK_SCHEMES: [&'static str; 6] = ["smb", "afp", "nfs", "ftp", "sftp", "ftps"];

    /// The symbol the launcher shows for it.
    pub fn symbol(&self) -> &'static str {
        match self {
            QuicklinkDestination::Web(_) => "globe",
            QuicklinkDestination::Network(_) => "externaldrive.connected.to.line.below",
            QuicklinkDestination::Deeplink(_) => "arrow.up.forward.app",
            QuicklinkDestination::Path(_) => "folder",
        }
    }

    /// The text the launcher shows for it.
    pub fn display_text(&self) -> String {
        match self {
            QuicklinkDestination::Web(url)
            | QuicklinkDestination::Network(url)
            | QuicklinkDestination::Deeplink(url) => url.to_string(),
            QuicklinkDestination::Path(path) => path.clone(),
        }
    }

    /// What the link points at.
    ///
    /// `home_directory` is passed in rather than read, so that a test never
    /// depends on the machine it runs on.
    pub fn detect(link: &str, home_directory: &str) -> Option<QuicklinkDestination> {
        let trimmed = link.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Some(path) = absolute_path(trimmed, home_directory) {
            return Some(QuicklinkDestination::Path(path));
        }
        if let Some(scheme) = scheme_of(trimmed) {
            let url = url_from(trimmed)?;
            // A file: URL names a local path, so it resolves to one rather
            // than to a deeplink.
            if scheme == "file" {
                return Some(QuicklinkDestination::Path(file_url_path(&url)?));
            }
            if scheme == "http" || scheme == "https" {
                return Some(QuicklinkDestination::Web(url));
            }
            return Some(if Self::NETWORK_SCHEMES.contains(&scheme.as_str()) {
                QuicklinkDestination::Network(url)
            } else {
                QuicklinkDestination::Deeplink(url)
            });
        }
        // A bare host is how people write a website down, so it reads as https.
        if !looks_like_bare_host(trimmed) {
            return None;
        }
        let url = url_from(&format!("https://{trimmed}"))?;
        Some(QuicklinkDestination::Web(url))
    }

    /// Whether substituted values need percent-encoding, decided before the
    /// placeholders resolve.
    pub fn uses_url_encoding(link: &str, home_directory: &str) -> bool {
        absolute_path(link.trim(), home_directory).is_none()
    }

    /// True while a placeholder remains; the validation accepts it and the
    /// open path reports it.
    pub fn contains_placeholder(link: &str) -> bool {
        let Some(opening) = link.find('{') else {
            return false;
        };
        link[opening + 1..].contains('}')
    }
}

/// An absolute local path, with the tilde expanded.
fn absolute_path(value: &str, home_directory: &str) -> Option<String> {
    // A Windows drive letter, such as `C:\Users`.
    if is_drive_path(value) {
        return Some(value.to_string());
    }
    // A UNC share, such as `\\server\share`.
    if value.starts_with(r"\\") {
        return Some(value.to_string());
    }
    // A root, on either system's spelling.
    if value.starts_with('/') || value.starts_with('\\') {
        return Some(value.to_string());
    }
    if !value.starts_with('~') {
        return None;
    }
    let home = home_directory.trim_end_matches(['/', '\\']);
    if value == "~" {
        return Some(home.to_string());
    }
    if value.starts_with("~/") || value.starts_with("~\\") {
        return Some(format!("{home}{}", &value[1..]));
    }
    None
}

/// A Windows drive letter, such as `C:\Users` or `c:/Users`.
fn is_drive_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// The scheme of a link, if it has one.
///
/// A single letter is not a scheme: it is a Windows drive letter. Neither is
/// a name and a port. Both are commoner than what they shadow.
fn scheme_of(value: &str) -> Option<String> {
    let colon = value.find(':')?;
    let candidate = &value[..colon];
    let mut characters = candidate.chars();
    let first = characters.next()?;
    if !first.is_alphabetic() {
        return None;
    }
    if !characters.all(|c| c.is_alphanumeric() || c == '+' || c == '-') {
        return None;
    }
    if candidate.chars().count() <= 1 {
        return None;
    }
    let remainder = &value[colon + 1..];
    if remainder.is_empty() || remainder.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(candidate.to_lowercase())
}

/// A URL, with a space rescued: re-encoding would corrupt an existing `%xx`.
fn url_from(value: &str) -> Option<Url> {
    Url::parse(value)
        .ok()
        .or_else(|| Url::parse(&value.replace(' ', "%20")).ok())
}

/// The local path a `file:` URL names.
///
/// The `url` crate's own conversion is per-platform, and a `file:` URL means
/// different things on the two systems, so the path is read here instead: a
/// host is a UNC share, and a leading `/C:` is a drive.
fn file_url_path(url: &Url) -> Option<String> {
    let path = percent_decode(url.path());
    let bytes = path.as_bytes();
    if bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
        return Some(path[1..].to_string());
    }
    match url.host_str() {
        Some(host) if !host.is_empty() => Some(format!(r"\\{host}{path}")),
        _ => Some(path),
    }
}

/// The percent-decoded text of a URL path.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
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

/// A bare host is a name with at least two labels and a letter-led last one.
fn looks_like_bare_host(value: &str) -> bool {
    if value.chars().any(char::is_whitespace) {
        return false;
    }
    let mut host = value.split(['/', '?', '#']).next().unwrap_or("");
    if let Some(colon) = host.find(':') {
        let port = &host[colon + 1..];
        if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        host = &host[..colon];
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 || labels.iter().any(|label| label.is_empty()) {
        return false;
    }
    // A letter-led final label keeps version numbers and decimals out.
    let Some(last) = labels.last() else {
        return false;
    };
    if last.len() < 2 || !last.chars().next().is_some_and(char::is_alphabetic) {
        return false;
    }
    last.chars().all(char::is_alphabetic)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The home directory the harness uses, so nothing depends on the machine.
    const HOME: &str = "/Users/tinycast-harness";

    fn detect(link: &str) -> Option<QuicklinkDestination> {
        QuicklinkDestination::detect(link, HOME)
    }

    fn kind(destination: &Option<QuicklinkDestination>) -> Option<&'static str> {
        destination.as_ref().map(|destination| match destination {
            QuicklinkDestination::Web(_) => "web",
            QuicklinkDestination::Network(_) => "network",
            QuicklinkDestination::Deeplink(_) => "deeplink",
            QuicklinkDestination::Path(_) => "path",
        })
    }

    #[test]
    fn https_is_a_website() {
        assert_eq!(kind(&detect("https://example.com")), Some("web"));
    }

    #[test]
    fn http_is_a_website() {
        assert_eq!(kind(&detect("http://example.com/a")), Some("web"));
    }

    #[test]
    fn a_bare_host_gains_https_keeping_its_path_and_query() {
        let destination = detect("example.com/search?q=1").expect("a website");
        let QuicklinkDestination::Web(url) = destination else {
            panic!("not a website");
        };
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("example.com"));
        assert_eq!(url.path(), "/search");
        assert_eq!(url.query(), Some("q=1"));
    }

    #[test]
    fn a_multi_label_host_is_still_a_website() {
        assert_eq!(kind(&detect("sub.example.co.uk")), Some("web"));
    }

    #[test]
    fn a_port_does_not_stop_a_bare_host_being_a_website() {
        let destination = detect("example.com:8080/x").expect("a website");
        let QuicklinkDestination::Web(url) = destination else {
            panic!("not a website");
        };
        assert_eq!(url.port(), Some(8080));
        assert_eq!(url.path(), "/x");
    }

    #[test]
    fn an_unknown_scheme_is_a_deeplink() {
        assert_eq!(kind(&detect("spotify://track/1")), Some("deeplink"));
    }

    #[test]
    fn a_deeplink_keeps_its_query() {
        let destination = detect("shortcuts://run-shortcut?name=Focus").expect("a deeplink");
        let QuicklinkDestination::Deeplink(url) = destination else {
            panic!("not a deeplink");
        };
        assert_eq!(url.query(), Some("name=Focus"));
    }

    #[test]
    fn a_schemeless_authority_scheme_is_still_a_deeplink() {
        assert_eq!(
            kind(&detect("mailto:someone@example.com")),
            Some("deeplink")
        );
    }

    #[test]
    fn a_share_is_a_network_path() {
        for link in [
            "smb://server/share",
            "afp://server/share",
            "nfs://server/share",
            "ftp://server/share",
            "sftp://server/share",
            "ftps://server/share",
        ] {
            assert_eq!(kind(&detect(link)), Some("network"), "{link}");
        }
    }

    #[test]
    fn a_literal_space_is_rescued_rather_than_rejected() {
        let destination = detect("http://example.com/a b").expect("a website");
        let QuicklinkDestination::Web(url) = destination else {
            panic!("not a website");
        };
        assert_eq!(url.path(), "/a%20b");
    }

    #[test]
    fn an_already_encoded_value_is_not_encoded_twice() {
        let destination = detect("https://example.com/a%20b").expect("a website");
        let QuicklinkDestination::Web(url) = destination else {
            panic!("not a website");
        };
        assert_eq!(url.path(), "/a%20b");
    }

    #[test]
    fn an_empty_link_resolves_to_nothing() {
        assert_eq!(detect(""), None);
    }

    #[test]
    fn a_whitespace_only_link_resolves_to_nothing() {
        assert_eq!(detect("   "), None);
    }

    #[test]
    fn prose_is_not_a_destination() {
        assert_eq!(detect("not a url"), None);
    }

    #[test]
    fn a_bare_leftover_placeholder_is_not_a_destination() {
        assert_eq!(detect("{argument}"), None);
    }

    #[test]
    fn a_decimal_is_not_a_host() {
        assert_eq!(detect("1.5"), None);
    }

    #[test]
    fn a_drive_letter_is_not_a_scheme() {
        // On Windows it is a path, which is the same answer for the same
        // reason: one letter and a colon never make a scheme.
        assert_eq!(kind(&detect("C:/Users/x")), Some("path"));
    }

    #[test]
    fn an_absolute_path_is_a_path() {
        assert_eq!(kind(&detect("/tmp")), Some("path"));
    }

    #[test]
    fn a_tilde_expands_against_the_injected_home() {
        let destination = detect("~/Downloads").expect("a path");
        let QuicklinkDestination::Path(path) = destination else {
            panic!("not a path");
        };
        assert_eq!(path, format!("{HOME}/Downloads"));
    }

    #[test]
    fn a_bare_tilde_is_the_home_directory() {
        let destination = detect("~").expect("a path");
        let QuicklinkDestination::Path(path) = destination else {
            panic!("not a path");
        };
        assert_eq!(path, HOME);
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_before_detection() {
        let destination = detect("  ~/Projects/Tinycast  ").expect("a path");
        let QuicklinkDestination::Path(path) = destination else {
            panic!("not a path");
        };
        assert_eq!(path, format!("{HOME}/Projects/Tinycast"));
    }

    #[test]
    fn a_file_url_resolves_to_the_path_it_names_not_to_a_deeplink() {
        let destination = detect("file:///tmp/notes.md").expect("a path");
        let QuicklinkDestination::Path(path) = destination else {
            panic!("not a path");
        };
        assert!(path.ends_with("notes.md"), "{path}");
    }

    #[test]
    fn a_file_url_names_a_drive_without_its_leading_slash() {
        let destination = detect("file:///C:/Users/x/notes.md").expect("a path");
        let QuicklinkDestination::Path(path) = destination else {
            panic!("not a path");
        };
        assert_eq!(path, "C:/Users/x/notes.md");
    }

    #[test]
    fn a_file_url_with_a_host_names_a_share() {
        let destination = detect("file://server/share/notes.md").expect("a path");
        let QuicklinkDestination::Path(path) = destination else {
            panic!("not a path");
        };
        assert_eq!(path, r"\\server/share/notes.md");
    }

    #[test]
    fn a_trailing_slash_on_the_home_directory_does_not_double_up() {
        let destination =
            QuicklinkDestination::detect("~/Downloads", "/Users/other/").expect("a path");
        let QuicklinkDestination::Path(path) = destination else {
            panic!("not a path");
        };
        assert_eq!(path, "/Users/other/Downloads");
    }

    #[test]
    fn a_unc_share_is_a_path() {
        assert_eq!(kind(&detect(r"\\server\share")), Some("path"));
    }

    #[test]
    fn values_going_into_a_url_are_encoded() {
        assert!(QuicklinkDestination::uses_url_encoding(
            "https://x.com/?q={argument}",
            HOME
        ));
        assert!(QuicklinkDestination::uses_url_encoding(
            "spotify://search/{argument}",
            HOME
        ));
    }

    #[test]
    fn values_going_into_a_path_are_not_encoded() {
        assert!(!QuicklinkDestination::uses_url_encoding(
            "~/Notes/{date}.md",
            HOME
        ));
        assert!(!QuicklinkDestination::uses_url_encoding(
            "/tmp/{argument}",
            HOME
        ));
    }

    #[test]
    fn a_token_is_a_placeholder() {
        assert!(QuicklinkDestination::contains_placeholder(
            "https://x.com/?q={argument}"
        ));
    }

    #[test]
    fn a_plain_link_has_no_placeholder() {
        assert!(!QuicklinkDestination::contains_placeholder(
            "https://x.com/?q=1"
        ));
    }

    #[test]
    fn an_unclosed_brace_is_not_a_placeholder() {
        assert!(!QuicklinkDestination::contains_placeholder(
            "https://x.com/{unterminated"
        ));
    }

    #[test]
    fn a_link_has_a_symbol_that_matches_its_kind() {
        assert_eq!(
            detect("https://example.com").map(|d| d.symbol()),
            Some("globe")
        );
        assert_eq!(
            detect("smb://server/share").map(|d| d.symbol()),
            Some("externaldrive.connected.to.line.below")
        );
        assert_eq!(
            detect("mailto:someone@example.com").map(|d| d.symbol()),
            Some("arrow.up.forward.app")
        );
        assert_eq!(detect("/tmp").map(|d| d.symbol()), Some("folder"));
    }

    #[test]
    fn the_text_shown_is_the_link_itself() {
        let destination = detect("https://example.com/a").expect("a website");
        assert_eq!(destination.display_text(), "https://example.com/a");
    }
}
