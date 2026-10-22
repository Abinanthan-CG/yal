//! The command sources.
//!
//! Each source knows how to turn the host state it owns into candidate items.
//! Sources never rank; they only produce. The engine ranks across all of
//! them so that a snippet and an application compete on equal terms.

use chrono::Utc;
use yal_calc::{Value, evaluate, format as format_value};
use yal_core::action::Action;
use yal_core::command::CommandKind;
use yal_core::icon::Icon;
use yal_core::item::Item;
use yal_data::emoji::Emoji;
use yal_index::Discovered;
use yal_search::Candidate;
use yal_store::Store;

/// The kind of thing a result came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An installed application.
    Application,
    /// A saved text snippet.
    Snippet,
    /// A saved link.
    Quicklink,
    /// A saved bookmark.
    Bookmark,
    /// A clipboard history entry.
    Clipboard,
    /// The result of evaluating the query as an expression.
    Calculator,
    /// An emoji.
    Emoji,
}

impl Source {
    /// The command this source belongs to.
    pub fn command_kind(self) -> CommandKind {
        match self {
            Source::Application => CommandKind::Applications,
            Source::Snippet => CommandKind::Snippets,
            Source::Quicklink => CommandKind::Quicklinks,
            Source::Bookmark => CommandKind::Bookmarks,
            Source::Clipboard => CommandKind::Clipboard,
            Source::Calculator => CommandKind::Calculator,
            Source::Emoji => CommandKind::Emoji,
        }
    }
}

/// Build candidates for installed applications.
pub fn applications(discovered: &[Discovered]) -> Vec<Candidate> {
    discovered
        .iter()
        .map(|application| {
            let mut item = Item::new(application.name.clone())
                .with_id(application.id.clone())
                .with_subtitle(application.path.display().to_string())
                .with_action(Action::open_path(application.path.clone()));
            if let Some(icon) = icon_for(&application.path) {
                item = item.with_icon(icon);
            }
            Candidate::new(item, CommandKind::Applications)
        })
        .collect()
}

/// Build candidates for snippets.
pub fn snippets(store: &Store) -> yal_store::StoreResult<Vec<Candidate>> {
    Ok(store
        .snippets()?
        .into_iter()
        .map(|snippet| {
            let item = Item::new(snippet.name.clone())
                .with_id(format!("snippet:{}", snippet.id))
                .with_subtitle(snippet.expansion.clone())
                .with_keyword(snippet.keyword.clone())
                .with_action(Action::copy(snippet.expansion.clone()));
            Candidate::new(item, CommandKind::Snippets)
        })
        .collect())
}

/// Build candidates for quicklinks.
pub fn quicklinks(store: &Store) -> yal_store::StoreResult<Vec<Candidate>> {
    Ok(store
        .quicklinks()?
        .into_iter()
        .map(|quicklink| {
            let item = Item::new(quicklink.name.clone())
                .with_id(format!("quicklink:{}", quicklink.id))
                .with_subtitle(quicklink.url.clone())
                .with_action(Action::open_url(quicklink.url.clone()));
            Candidate::new(item, CommandKind::Quicklinks)
        })
        .collect())
}

/// Build candidates for bookmarks.
pub fn bookmarks(store: &Store) -> yal_store::StoreResult<Vec<Candidate>> {
    Ok(store
        .bookmarks()?
        .into_iter()
        .map(|bookmark| {
            let item = Item::new(bookmark.name.clone())
                .with_id(format!("bookmark:{}", bookmark.id))
                .with_subtitle(bookmark.url.clone())
                .with_action(Action::open_url(bookmark.url.clone()));
            Candidate::new(item, CommandKind::Bookmarks)
        })
        .collect())
}

/// Build candidates for the clipboard history.
pub fn clipboard(store: &Store, limit: usize) -> yal_store::StoreResult<Vec<Candidate>> {
    Ok(store
        .clipboard(limit)?
        .into_iter()
        .map(|entry| {
            let item = Item::new(first_line(&entry.text))
                .with_id(format!("clipboard:{}", entry.id))
                .with_subtitle(entry.text.clone())
                .with_action(Action::copy(entry.text.clone()));
            Candidate::new(item, CommandKind::Clipboard)
        })
        .collect())
}

/// Build the calculator candidate, if the query evaluates to a value.
pub fn calculator(query: &str) -> Option<Candidate> {
    let value = evaluate(query).ok()?;
    if !is_interesting(&value) {
        return None;
    }
    let text = format_value(&value);
    let item = Item::new(text.clone())
        .with_id("calculator:result".to_string())
        .with_subtitle(format!("{query} = {text}"))
        .with_action(Action::copy(text));
    Some(Candidate::new(item, CommandKind::Calculator))
}

/// Build candidates for emoji.
pub fn emoji(query: &str, limit: usize) -> Vec<Candidate> {
    yal_data::emoji::search(query, limit)
        .into_iter()
        .map(|Emoji { character, name, keywords }| {
            let mut item = Item::new(character.clone())
                .with_id(format!("emoji:{}", name))
                .with_subtitle(name.clone())
                .with_action(Action::copy(character.clone()));
            for keyword in keywords {
                item = item.with_keyword(keyword);
            }
            Candidate::new(item, CommandKind::Emoji)
        })
        .collect()
}

/// Build the QR code candidate for a query.
///
/// The rendered PNG travels in the item payload so the palette can draw it
/// without touching the filesystem.
pub fn qr_code(query: &str) -> Option<Candidate> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let code = yal_data::qr::generate(trimmed, 6).ok()?;
    let encoded = base64_encode(&code.png);
    let item = Item::new(format!("QR code — {trimmed}"))
        .with_id("qr:result".to_string())
        .with_subtitle(format!("{}×{} pixels", code.width, code.height))
        .with_action(Action::copy(trimmed.to_string()))
        .with_payload(serde_json::json!({
            "kind": "qr",
            "png_base64": encoded,
            "width": code.width,
            "height": code.height,
        }));
    Some(Candidate::new(item, CommandKind::QrCode))
}

/// Build the one-time password candidate for a query.
///
/// The query is a base32 secret. The subtitle carries the seconds remaining
/// before the code changes.
pub fn totp(query: &str) -> Option<Candidate> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let secret = yal_data::totp::TotpSecret::parse(trimmed)?;
    let now = chrono::Utc::now().timestamp() as u64;
    let (code, remaining) = secret.code_and_remaining(
        now,
        yal_data::totp::DEFAULT_DIGITS,
        yal_data::totp::DEFAULT_PERIOD,
    );
    let item = Item::new(code.clone())
        .with_id("totp:result".to_string())
        .with_subtitle(format!("{remaining}s remaining"))
        .with_action(Action::copy(code))
        .with_payload(serde_json::json!({
            "kind": "totp",
            "seconds_remaining": remaining,
        }));
    Some(Candidate::new(item, CommandKind::Passwords))
}

/// Build the regular expression candidate for a query.
///
/// The pattern is the query; the subject is the text to match against.
pub fn regex(query: &str, subject: &str) -> Option<Candidate> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let result = yal_data::regex::match_pattern(trimmed, subject);
    let summary = yal_data::regex::summarise(&result);
    let mut item = Item::new(summary.clone())
        .with_id("regex:result".to_string())
        .with_subtitle(trimmed.to_string());
    if let Some(first) = result.matches.first() {
        item = item.with_action(Action::copy(first.text.clone()));
    }
    Some(Candidate::new(item, CommandKind::Regex))
}

/// Build the web search candidate for a query.
pub fn web_search(query: &str) -> Option<Candidate> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let url = format!("https://duckduckgo.com/?q={}", url_encode(trimmed));
    let item = Item::new(format!("Search the web for “{trimmed}”"))
        .with_id("websearch:result".to_string())
        .with_subtitle(url.clone())
        .with_action(Action::open_url(url));
    Some(Candidate::new(item, CommandKind::WebSearch))
}

/// Build candidates for the open windows.
pub fn windows(list: &[yal_system::Window]) -> Vec<Candidate> {
    list.iter()
        .map(|window| {
            let mut item = Item::new(window.title.clone())
                .with_id(format!("window:{}", window.handle))
                .with_subtitle(
                    window
                        .process_name
                        .clone()
                        .unwrap_or_else(|| format!("pid {}", window.process_id)),
                )
                .with_action(Action::open_path(std::path::PathBuf::from(
                    format!("window:{}", window.handle),
                )));
            if let Some(name) = &window.process_name {
                item = item.with_keyword(name.clone());
            }
            Candidate::new(item, CommandKind::Windows)
        })
        .collect()
}

/// Build candidates for files found on disk.
pub fn files(found: &[yal_index::FoundFile]) -> Vec<Candidate> {
    found
        .iter()
        .map(|file| {
            let item = Item::new(file.name.clone())
                .with_id(format!("file:{}", file.path.display()))
                .with_subtitle(file.directory.display().to_string())
                .with_action(Action::open_path(file.path.clone()));
            Candidate::new(item, CommandKind::Files)
        })
        .collect()
}

/// Build the snippet expansion candidate when the query is exactly a snippet
/// keyword.
///
/// The expansion is offered as its own result so it can be chosen without
/// leaving the palette.
pub fn snippet_expansion(
    store: &Store,
    query: &str,
) -> yal_store::StoreResult<Option<Candidate>> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(None);
    }
    for snippet in store.snippets()? {
        if snippet.keyword.to_lowercase() == needle {
            let item = Item::new(snippet.expansion.clone())
                .with_id(format!("snippet-expansion:{}", snippet.id))
                .with_subtitle(format!("{} — snippet", snippet.name))
                .with_action(Action::copy(snippet.expansion.clone()));
            return Ok(Some(Candidate::new(item, CommandKind::Snippets)));
        }
    }
    Ok(None)
}

/// Percent-encode a string for use in a URL.
fn url_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// Standard base64 encoding, used to carry the QR image in a payload.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        encoded.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        encoded.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

/// Whether a calculator value is worth showing.
///
/// A bare `0` is almost never what the user meant to compute, and `2` is more
/// likely to be an application search than an arithmetic result.
fn is_interesting(value: &Value) -> bool {
    match value {
        Value::Scalar(number) => number.abs() >= 1.0,
        _ => true,
    }
}

/// The first line of a text, truncated for display.
fn first_line(text: &str) -> String {
    const MAX: usize = 120;
    let line = text.lines().next().unwrap_or(text);
    if line.chars().count() <= MAX {
        return line.to_string();
    }
    let mut truncated: String = line.chars().take(MAX).collect();
    truncated.push('…');
    truncated
}

/// The icon for an application, derived from its executable path.
fn icon_for(path: &std::path::Path) -> Option<Icon> {
    let extension = path.extension()?.to_str()?;
    if extension.eq_ignore_ascii_case("exe") {
        Some(Icon::symbol("app"))
    } else {
        None
    }
}

/// The current time, used when no other clock is available.
pub fn now() -> chrono::DateTime<Utc> {
    Utc::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line_truncates_long_text() {
        let long = "x".repeat(500);
        assert_eq!(first_line(&long).chars().count(), 121);
        assert_eq!(first_line("a\nb"), "a");
    }

    #[test]
    fn calculator_ignores_trivial_values() {
        assert!(calculator("0").is_none());
        assert!(calculator("1 + 1").is_some());
    }

    #[test]
    fn calculator_reports_the_expression() {
        let candidate = calculator("2 + 2").unwrap();
        assert_eq!(candidate.item.title, "4");
        assert_eq!(candidate.item.subtitle.as_deref(), Some("2 + 2 = 4"));
    }
}
