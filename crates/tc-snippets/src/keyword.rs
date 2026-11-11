//! The keyword policy: how a snippet's trigger keyword is matched.
//!
//! Mirrors upstream `Features/Snippets/Model/SnippetKeywordPolicy.swift`:
//! keywords are normalized (trimmed and lowercased), rejected when they are
//! empty or longer than the buffer can hold, and made unique by the snippet
//! identity they belong to.

use std::collections::HashSet;
use std::time::Duration;

/// How long the buffer waits before it gives up.
///
/// Mirrors upstream's `SnippetKeywordPolicy.timeout`.
pub const TIMEOUT: Duration = Duration::from_secs(15);

/// The longest keyword the buffer can hold.
///
/// Mirrors upstream's `SnippetKeywordPolicy.maximumBufferLength`.
pub const MAXIMUM_BUFFER_LENGTH: usize = 256;

/// A keyword, and the snippet it triggers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword {
    /// The snippet this keyword triggers.
    pub snippet_id: String,
    /// The keyword itself, trimmed and lowercased.
    pub value: String,
    /// How many characters have to be deleted for the keyword to be typed.
    pub deletion_count: usize,
}

impl Keyword {
    /// A keyword for a snippet.
    ///
    /// Mirrors upstream's `Keyword.init(snippetID:value:)`: the value is
    /// trimmed of whitespace and newlines and lowercased, and the deletion
    /// count is the length of the trimmed value.
    pub fn new(snippet_id: impl Into<String>, value: impl AsRef<str>) -> Self {
        let trimmed = value.as_ref().trim_matches(|c: char| c.is_whitespace());
        Self {
            snippet_id: snippet_id.into(),
            value: trimmed.to_lowercase(),
            deletion_count: trimmed.chars().count(),
        }
    }
}

/// A keyword that was matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// The snippet the keyword triggers.
    pub snippet_id: String,
    /// The keyword that matched.
    pub keyword: String,
    /// How many characters have to be deleted for the keyword to be typed.
    pub deletion_count: usize,
}

/// What the user did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// The user typed some text.
    Text(String),
    /// The user pressed backspace.
    DeleteBackward,
    /// The buffer has to be cleared.
    Reset,
    /// The event is not the user's typing, so it is ignored.
    Ignored,
}

/// The keyword buffer and the keywords it matches.
///
/// Mirrors upstream's `SnippetKeywordPolicy`.
#[derive(Debug, Clone, Default)]
pub struct SnippetKeywordPolicy {
    keywords: Vec<Keyword>,
    buffer: String,
    last_input_at: Option<Duration>,
}

impl SnippetKeywordPolicy {
    /// A policy that starts with a set of keywords.
    ///
    /// Mirrors upstream's `init(keywords:)`.
    pub fn new(keywords: Vec<Keyword>) -> Self {
        let mut policy = Self::default();
        policy.update(keywords);
        policy
    }

    /// The keywords the policy matches, longest first.
    pub fn keywords(&self) -> &[Keyword] {
        &self.keywords
    }

    /// The text typed so far.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Replace the keywords the policy matches.
    ///
    /// Mirrors upstream's `update(_:)`: empty keywords and keywords longer
    /// than the buffer are rejected, the rest are sorted longest first, and
    /// duplicates of the same keyword are resolved by the snippet identity.
    pub fn update(&mut self, keywords: Vec<Keyword>) {
        let mut accepted: Vec<Keyword> = keywords
            .into_iter()
            .filter(|keyword| {
                !keyword.value.is_empty() && keyword.deletion_count <= MAXIMUM_BUFFER_LENGTH
            })
            .collect();
        accepted.sort_by(|lhs, rhs| {
            rhs.value
                .chars()
                .count()
                .cmp(&lhs.value.chars().count())
                .then_with(|| lhs.snippet_id.cmp(&rhs.snippet_id))
        });
        self.keywords = accepted;
        self.reset();
    }

    /// Feed the buffer an input and report the keyword it matched.
    ///
    /// Mirrors upstream's `process(_:at:)`.
    pub fn process(&mut self, input: &Input, now: Duration) -> Option<Match> {
        if matches!(input, Input::Ignored) {
            return None;
        }
        if let Some(last_input_at) = self.last_input_at {
            if now.saturating_sub(last_input_at) > TIMEOUT {
                self.reset();
            }
        }

        match input {
            Input::Ignored => return None,
            Input::Reset => {
                self.reset();
                return None;
            }
            Input::DeleteBackward => {
                self.last_input_at = Some(now);
                self.remove_last_character();
                return None;
            }
            Input::Text(text) => {
                self.last_input_at = Some(now);
                self.buffer.push_str(text);
                self.trim_buffer();
            }
        }

        let normalized = self.buffer.to_lowercase();
        let matched = self
            .keywords
            .iter()
            .find(|keyword| normalized.ends_with(&keyword.value))?
            .clone();
        self.reset();
        Some(Match {
            snippet_id: matched.snippet_id,
            keyword: matched.keyword,
            deletion_count: matched.deletion_count,
        })
    }

    /// Clear the buffer.
    ///
    /// Mirrors upstream's `reset()`.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.last_input_at = None;
    }

    /// Work out what an input event means.
    ///
    /// Mirrors upstream's `classifyInput(text:isSynthetic:secureEventInputEnabled:isFlagsChanged:isKeyDown:hasCommandOrControl:isResetKey:isDeleteBackward:)`.
    #[allow(clippy::too_many_arguments)]
    pub fn classify_input(
        text: Option<&str>,
        is_synthetic: bool,
        secure_event_input_enabled: bool,
        is_flags_changed: bool,
        is_key_down: bool,
        has_command_or_control: bool,
        is_reset_key: bool,
        is_delete_backward: bool,
    ) -> Input {
        if is_synthetic {
            return Input::Ignored;
        }
        if secure_event_input_enabled || has_command_or_control || is_reset_key {
            return Input::Reset;
        }
        if is_flags_changed {
            return Input::Ignored;
        }
        if !is_key_down {
            return Input::Ignored;
        }
        if is_delete_backward {
            return Input::DeleteBackward;
        }
        match text {
            Some(text) => Input::Text(text.to_string()),
            None => Input::Reset,
        }
    }

    /// Remove the last character of the buffer.
    fn remove_last_character(&mut self) {
        if let Some(index) = self.buffer.char_indices().next_back() {
            self.buffer.truncate(index.0);
        }
    }

    /// Keep the buffer within its length limit.
    fn trim_buffer(&mut self) {
        let mut characters: Vec<char> = self.buffer.chars().collect();
        if characters.len() > MAXIMUM_BUFFER_LENGTH {
            characters.drain(..characters.len() - MAXIMUM_BUFFER_LENGTH);
            self.buffer = characters.into_iter().collect();
        }
    }
}

/// The keywords of a set of snippets, one per snippet that has one.
///
/// Mirrors what upstream's keyword listener builds for the snippets it watches.
pub fn keywords_for_snippets(snippets: &[(String, Option<String>)]) -> Vec<Keyword> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut keywords = Vec::new();
    for (snippet_id, keyword) in snippets {
        let Some(keyword) = keyword else {
            continue;
        };
        let normalized = keyword.trim().to_lowercase();
        if normalized.is_empty() || !seen.insert(normalized) {
            continue;
        }
        keywords.push(Keyword::new(snippet_id.clone(), keyword));
    }
    keywords
}
