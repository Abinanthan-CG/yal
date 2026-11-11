//! The card the calculator shows, as upstream's `CalcResult`.

use crate::formatter::Formatter;

/// What a card shows, as upstream's `CalcResult.Payload`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    /// A value: the grouped display and the ungrouped copy text.
    Value {
        /// The text the card shows.
        display: String,
        /// The text that lands on the pasteboard.
        copy_text: String,
    },
    /// A friendly error, only for a clear conversion attempt.
    Error {
        /// The message the card shows.
        message: String,
    },
}

impl Payload {
    /// The payload of a plain number, as upstream's `CalcResult.Payload.number`.
    pub fn number(value: f64) -> Self {
        let text = Formatter::copy_text(value);
        Self::Value {
            display: Formatter::grouped(&text),
            copy_text: text,
        }
    }
}

/// The card the calculator shows, as upstream's `CalcResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcResult {
    /// The normalized echo of what was evaluated.
    pub expression: String,
    /// The word-name pill beneath the left side.
    pub source_badge: Option<String>,
    /// The word-name pill beneath the right side.
    pub target_badge: Option<String>,
    /// The answer itself.
    pub payload: Payload,
    /// Whether the copy text, typed back as a query, is this answer.
    pub can_chain: bool,
    /// Whether the answer is money.
    pub is_currency: bool,
}

impl CalcResult {
    /// A card that shows a value.
    pub fn new(expression: String, payload: Payload) -> Self {
        Self {
            expression,
            source_badge: None,
            target_badge: None,
            payload,
            can_chain: true,
            is_currency: false,
        }
    }

    /// A card with the two word-name pills.
    pub fn with_badges(
        expression: String,
        source_badge: &str,
        target_badge: &str,
        payload: Payload,
    ) -> Self {
        Self {
            expression,
            source_badge: Some(source_badge.to_string()),
            target_badge: Some(target_badge.to_string()),
            payload,
            can_chain: true,
            is_currency: false,
        }
    }

    /// The card of a phrase, as upstream's `CalcResult.phrase`.
    ///
    /// The query is echoed with its spacing collapsed and badged by what it answers.
    pub fn phrase(query: &str, payload: Payload, target: &str) -> Self {
        Self::with_badges(Formatter::expression(query), "Expression", target, payload)
    }

    /// The same card with another expression, as upstream's `CalcEngine.replacingExpression`.
    pub fn with_expression(&self, expression: String) -> Self {
        Self {
            expression,
            ..self.clone()
        }
    }
}
