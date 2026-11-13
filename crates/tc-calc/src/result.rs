//! The card the calculator shows, as upstream's `CalcResult`.

use crate::formatter::Formatter;
use crate::units::UnitDef;

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

    /// The payload of a measurement, as upstream's `CalcResult.Payload.measurement`.
    ///
    /// CSS lengths copy without a space, so the answer pastes straight into a stylesheet.
    pub fn measurement(value: f64, unit: &UnitDef) -> Self {
        let text = Formatter::copy_text(value);
        let separator = if unit.category == crate::units::UnitCategory::Pixels {
            ""
        } else {
            " "
        };
        Self::Value {
            display: format!("{} {}", Formatter::grouped(&text), unit.symbol),
            copy_text: format!("{text}{separator}{}", unit.symbol),
        }
    }

    /// The payload of a measurement that carries its unit, as upstream's
    /// `CalcResult.Payload.measurement`.
    pub fn measured(value: f64, unit: &crate::measure::Measurement) -> Self {
        let text = Formatter::copy_text(value);
        let separator = if unit.category == crate::units::UnitCategory::Pixels {
            ""
        } else {
            " "
        };
        Self::Value {
            display: format!("{} {}", Formatter::grouped(&text), unit.symbol),
            copy_text: format!("{text}{separator}{}", unit.symbol),
        }
    }

    /// The payload of a percentage, as upstream's `CalcResult.Payload.number(_:suffix:)`.
    ///
    /// The sign sits outside the grouped digits, so `1234.5%` reads as it should.
    pub fn percentage(value: f64) -> Self {
        Self::suffixed(value, "%")
    }

    /// The payload of a number with a sign after it, as upstream's
    /// `CalcResult.Payload.number(_:suffix:)`.
    ///
    /// `5x` is a multiplier and `25%` is a percentage.
    pub fn suffixed(value: f64, suffix: &str) -> Self {
        let text = Formatter::copy_text(value);
        Self::Value {
            display: format!("{}{suffix}", Formatter::grouped(&text)),
            copy_text: format!("{text}{suffix}"),
        }
    }

    /// The payload of a text answer, as upstream's `CalcResult.Payload.value`.
    pub fn text(text: &str) -> Self {
        Self::Value {
            display: text.to_string(),
            copy_text: text.to_string(),
        }
    }

    /// The payload of an amount of money, as upstream's `CalcResult.Payload.money`.
    ///
    /// The code follows the amount, so `80.00 USD` reads the way a bill does, and the
    /// digits are grouped the way they are on one.
    pub fn money(value: f64, code: &str) -> Self {
        let text = Formatter::currency(value);
        Self::Value {
            display: format!("{} {}", Formatter::grouped(&text), code),
            copy_text: format!("{text} {code}"),
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
        Self::with_badges(Formatter::collapsed(query), "Expression", target, payload)
    }

    /// The same card with another expression, as upstream's `CalcEngine.replacingExpression`.
    pub fn with_expression(&self, expression: String) -> Self {
        Self {
            expression,
            ..self.clone()
        }
    }
}
