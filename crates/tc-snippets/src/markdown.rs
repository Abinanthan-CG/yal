//! The Markdown file format a snippet is stored in.
//!
//! Mirrors upstream `Features/Snippets/Model/SnippetMarkdownSerializer.swift`:
//! a small YAML-ish front matter block, then the snippet's body, with the
//! body starting immediately after the closing delimiter's line break.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::snippet::Snippet;

/// Why a Markdown file could not be read as a snippet.
///
/// Mirrors upstream's `SnippetMarkdownSerializer.ParseError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// The file that failed to parse.
    pub file: PathBuf,
    /// The one-based line the failure was found on.
    pub line: usize,
    /// What was wrong with that line.
    pub reason: String,
}

impl fmt::Display for ParseError {
    /// Upstream's `errorDescription`: `<path>:<line>: <reason>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.file.display(), self.line, self.reason)
    }
}

impl std::error::Error for ParseError {}

/// Reads and writes the Markdown form of a snippet.
///
/// Mirrors upstream `SnippetMarkdownSerializer`.
pub struct SnippetMarkdownSerializer;

impl SnippetMarkdownSerializer {
    /// Read a snippet out of a Markdown file's content.
    ///
    /// Mirrors upstream's `parse(content:fileURL:)`. A file that does not
    /// start with an exact `---` line is all body, and the name falls back to
    /// the file name.
    pub fn parse(content: &str, file: &Path) -> Result<Snippet, ParseError> {
        let lines = source_lines(content);
        let Some(first) = lines.first() else {
            return Ok(Snippet::new(default_name(file), content.to_string()));
        };
        if first.text != "---" {
            return Ok(Snippet::new(default_name(file), content.to_string()));
        }

        let Some(closing_index) = lines
            .iter()
            .skip(1)
            .position(|line| line.text == "---")
            .map(|index| index + 1)
        else {
            return Err(parse_error(
                file,
                1,
                "Missing closing frontmatter delimiter".to_string(),
            ));
        };

        let mut name: Option<String> = None;
        let mut keyword: Option<String> = None;
        let mut is_enabled = true;
        let mut shows_confirmation = false;
        let mut seen_keys: Vec<String> = Vec::new();

        for (line_index, line) in lines.iter().enumerate().take(closing_index).skip(1) {
            let line_number = line_index + 1;
            if line
                .text
                .trim_matches(|c: char| c.is_whitespace())
                .is_empty()
            {
                continue;
            }
            let Some(separator) = line.text.find(':') else {
                return Err(parse_error(
                    file,
                    line_number,
                    "Expected a key and value separated by ':'".to_string(),
                ));
            };

            let raw_key = line.text[..separator].trim_matches(char::is_whitespace);
            let raw_value = line.text[separator + 1..].trim_matches(char::is_whitespace);
            let Some(key) = canonical_key(raw_key) else {
                return Err(parse_error(
                    file,
                    line_number,
                    format!("Unsupported frontmatter key '{raw_key}'"),
                ));
            };
            if seen_keys.iter().any(|seen| seen == &key) {
                return Err(parse_error(
                    file,
                    line_number,
                    format!("Duplicate frontmatter key '{key}'"),
                ));
            }
            seen_keys.push(key.clone());

            match key.as_str() {
                "name" => {
                    let decoded = decode_scalar(raw_value)
                        .map_err(|reason| parse_error(file, line_number, reason))?;
                    // A blank name reads as absent, so the filename fallback still yields a label.
                    name = if decoded.trim_matches(|c: char| c.is_whitespace()).is_empty() {
                        None
                    } else {
                        Some(decoded)
                    };
                }
                "keyword" => {
                    keyword = Some(
                        decode_scalar(raw_value)
                            .map_err(|reason| parse_error(file, line_number, reason))?,
                    );
                }
                "enabled" => {
                    is_enabled = decode_boolean(raw_value)
                        .map_err(|reason| parse_error(file, line_number, reason))?;
                }
                "show_confirmation" => {
                    shows_confirmation = decode_boolean(raw_value)
                        .map_err(|reason| parse_error(file, line_number, reason))?;
                }
                _ => unreachable!("canonical keys are exhaustively handled"),
            }
        }

        let body_start = lines[closing_index].end_including_terminator;
        Ok(Snippet::with_options(
            name.unwrap_or_else(|| default_name(file)),
            content[body_start..].to_string(),
            keyword,
            is_enabled,
            shows_confirmation,
        ))
    }

    /// Write a snippet out as the content of its Markdown file.
    ///
    /// Mirrors upstream's `serialize(_:)`: the canonical key order is
    /// `name`, `keyword`, `enabled`, `show_confirmation`.
    pub fn serialize(snippet: &Snippet) -> String {
        let mut lines = vec![
            "---".to_string(),
            format!("name: {}", encode_scalar(&snippet.name)),
        ];
        if let Some(keyword) = &snippet.keyword {
            lines.push(format!("keyword: {}", encode_scalar(keyword)));
        }
        lines.push(format!("enabled: {}", snippet.is_enabled));
        lines.push(format!("show_confirmation: {}", snippet.shows_confirmation));
        lines.push("---".to_string());
        lines.join("\n") + "\n" + &snippet.text
    }

    /// The file name a snippet of this name is stored under.
    ///
    /// Mirrors upstream's `slug(for:)`: the name lowercased, split on
    /// non-alphanumeric characters and joined with `-`.
    pub fn slug(name: &str) -> String {
        let mut slug = String::new();
        let mut pending_separator = false;
        for character in name.to_lowercase().chars() {
            if character.is_alphanumeric() {
                if pending_separator && !slug.is_empty() {
                    slug.push('-');
                }
                pending_separator = false;
                slug.push(character);
            } else {
                pending_separator = true;
            }
        }
        if slug.is_empty() {
            slug.push_str("snippet");
        }
        slug
    }
}

/// One line of a source file, and where it ends including its terminator.
struct SourceLine {
    /// The line's text, without its terminator.
    text: String,
    /// The byte offset just past the line's terminator.
    end_including_terminator: usize,
}

/// Split content into lines, keeping the byte offset past each terminator.
///
/// Mirrors upstream's `sourceLines(in:)`: `\n`, `\r` and `\r\n` all end a
/// line, and `\r\n` counts as one terminator.
fn source_lines(content: &str) -> Vec<SourceLine> {
    let bytes = content.as_bytes();
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        let character = bytes[index];
        if character != b'\n' && character != b'\r' {
            index += 1;
            continue;
        }

        let mut terminator_end = index + 1;
        if character == b'\r' && terminator_end < bytes.len() && bytes[terminator_end] == b'\n' {
            terminator_end += 1;
        }
        lines.push(SourceLine {
            text: content[line_start..index].to_string(),
            end_including_terminator: terminator_end,
        });
        line_start = terminator_end;
        index = terminator_end;
    }

    if line_start < content.len() || lines.is_empty() {
        lines.push(SourceLine {
            text: content[line_start..].to_string(),
            end_including_terminator: content.len(),
        });
    }
    lines
}

/// The front matter key a raw key names, if upstream recognizes it.
fn canonical_key(raw_key: &str) -> Option<String> {
    let key = raw_key.to_lowercase();
    match key.as_str() {
        "name" | "keyword" | "enabled" | "show_confirmation" => Some(key),
        _ => None,
    }
}

/// Decode a quoted front matter string.
///
/// Mirrors upstream's `decodeScalar(_:fileURL:line:)`: only `"`, `\\`, `\n`,
/// `\r` and `\t` may be escaped, and nothing may follow the closing quote.
fn decode_scalar(value: &str) -> Result<String, String> {
    let mut characters = value.chars();
    if characters.next() != Some('"') {
        return Err("String values must use double quotes".to_string());
    }

    let mut decoded = String::new();
    let mut iterator = characters;
    while let Some(character) = iterator.next() {
        match character {
            '"' => {
                if iterator.clone().all(|c| c == ' ' || c == '\t') {
                    return Ok(decoded);
                }
                return Err("Unexpected text after quoted value".to_string());
            }
            '\\' => {
                let Some(escaped) = iterator.next() else {
                    return Err("Unterminated escape sequence".to_string());
                };
                match escaped {
                    '\\' => decoded.push('\\'),
                    '"' => decoded.push('"'),
                    'n' => decoded.push('\n'),
                    'r' => decoded.push('\r'),
                    't' => decoded.push('\t'),
                    _ => return Err("Unsupported escape sequence".to_string()),
                }
            }
            '\n' | '\r' | '\t' => {
                return Err("Control characters must be escaped".to_string());
            }
            other => decoded.push(other),
        }
    }
    Err("Unterminated quoted value".to_string())
}

/// Decode a strict boolean front matter value.
fn decode_boolean(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("Boolean values must be exactly 'true' or 'false'".to_string()),
    }
}

/// Encode a string as a quoted front matter scalar.
///
/// Mirrors upstream's `encodeScalar(_:)`.
fn encode_scalar(value: &str) -> String {
    let mut encoded = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => encoded.push_str("\\\\"),
            '"' => encoded.push_str("\\\""),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            other => encoded.push(other),
        }
    }
    encoded.push('"');
    encoded
}

/// The name a snippet gets when its file carries no `name`.
///
/// Mirrors upstream's `defaultName(for:)`: the file name without its
/// extension, with `-` turned into spaces and each word capitalized.
fn default_name(file: &Path) -> String {
    let stem = file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    stem.replace('-', " ")
        .split_whitespace()
        .map(capitalize_word)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Capitalize one word the way upstream's `capitalized(with:)` does.
fn capitalize_word(word: &str) -> String {
    let mut characters = word.chars();
    match characters.next() {
        None => String::new(),
        Some(first) => {
            let mut capitalized: String = first.to_uppercase().collect();
            capitalized.push_str(&characters.as_str().to_lowercase());
            capitalized
        }
    }
}

/// Build a parse error for a file and line.
fn parse_error(file: &Path, line: usize, reason: String) -> ParseError {
    ParseError {
        file: file.to_path_buf(),
        line,
        reason,
    }
}
