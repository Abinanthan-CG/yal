//! The token vocabulary of the calculator.
//!
//! Mirrors upstream `CalcToken.swift`, `CalcOperator.swift` and `CalcNumberBase.swift`.

/// The numeric base a literal or a conversion target is written in.
///
/// Mirrors upstream `CalcNumberBase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberBase {
    /// Base 2, written with the `0b` prefix.
    Binary,
    /// Base 8, written with the `0o` prefix.
    Octal,
    /// Base 10, the default.
    Decimal,
    /// Base 16, written with the `0x` prefix.
    Hexadecimal,
}

impl NumberBase {
    /// Parses the base names upstream's `CalcNumberBase.init?(name:)` accepts.
    ///
    /// Upstream answers `nil` for every other spelling.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bin" | "binary" => Some(Self::Binary),
            "oct" | "octal" => Some(Self::Octal),
            "dec" | "decimal" => Some(Self::Decimal),
            "hex" | "hexadecimal" => Some(Self::Hexadecimal),
            _ => None,
        }
    }

    /// The base a radix stands for, as upstream's `CalcNumberBase(rawValue:)`.
    ///
    /// Upstream answers `nil` for a radix that is not one of the four bases.
    pub fn from_radix(radix: u32) -> Option<Self> {
        match radix {
            2 => Some(Self::Binary),
            8 => Some(Self::Octal),
            10 => Some(Self::Decimal),
            16 => Some(Self::Hexadecimal),
            _ => None,
        }
    }

    /// The radix, as upstream's `CalcNumberBase.rawValue`.
    pub fn radix(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }

    /// The literal prefix, as upstream's `CalcNumberBase.prefix`.
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Binary => "0b",
            Self::Octal => "0o",
            Self::Decimal => "",
            Self::Hexadecimal => "0x",
        }
    }

    /// The badge upstream shows for this base, as upstream's `CalcNumberBase.name`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Binary => "Binary",
            Self::Octal => "Octal",
            Self::Decimal => "Decimal",
            Self::Hexadecimal => "Hexadecimal",
        }
    }
}

/// An operator of the calculator, mirroring upstream `CalcOperator`.
///
/// Upstream declares the operators as `Character`-backed, so every one of them has a
/// glyph and a spelling. The glyph is the raw character; the spelling is what the
/// card's expression column shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `*`
    Multiply,
    /// `/`
    Divide,
    /// `^`
    Power,
    /// `!`
    Factorial,
    /// `%`
    Percent,
    /// `(`
    Open,
    /// `)`
    Close,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `~`
    BitNot,
    /// `⊻`
    BitXor,
    /// `«`
    ShiftLeft,
    /// `»`
    ShiftRight,
    /// `≡`
    Equal,
    /// `≠`
    NotEqual,
    /// `<`
    Less,
    /// `>`
    Greater,
    /// `≤`
    LessEqual,
    /// `≥`
    GreaterEqual,
}

impl Operator {
    /// Every operator with its glyph, in the order upstream lists them.
    pub const ALL: [Operator; 21] = [
        Operator::Add,
        Operator::Subtract,
        Operator::Multiply,
        Operator::Divide,
        Operator::Power,
        Operator::Factorial,
        Operator::Percent,
        Operator::Open,
        Operator::Close,
        Operator::BitAnd,
        Operator::BitOr,
        Operator::BitNot,
        Operator::BitXor,
        Operator::ShiftLeft,
        Operator::ShiftRight,
        Operator::Equal,
        Operator::NotEqual,
        Operator::Less,
        Operator::Greater,
        Operator::LessEqual,
        Operator::GreaterEqual,
    ];

    /// The operator a glyph stands for, as upstream's `CalcOperator(rawValue:)`.
    ///
    /// Upstream answers `nil` for a character that is not an operator.
    pub fn from_char(glyph: char) -> Option<Self> {
        Some(match glyph {
            '+' => Operator::Add,
            '-' => Operator::Subtract,
            '*' => Operator::Multiply,
            '/' => Operator::Divide,
            '^' => Operator::Power,
            '!' => Operator::Factorial,
            '%' => Operator::Percent,
            '(' => Operator::Open,
            ')' => Operator::Close,
            '&' => Operator::BitAnd,
            '|' => Operator::BitOr,
            '~' => Operator::BitNot,
            '⊻' => Operator::BitXor,
            '«' => Operator::ShiftLeft,
            '»' => Operator::ShiftRight,
            '≡' => Operator::Equal,
            '≠' => Operator::NotEqual,
            '<' => Operator::Less,
            '>' => Operator::Greater,
            '≤' => Operator::LessEqual,
            '≥' => Operator::GreaterEqual,
            _ => return None,
        })
    }

    /// The glyph the operator is typed as, as upstream's `CalcOperator.rawValue`.
    pub fn glyph(self) -> char {
        match self {
            Operator::Add => '+',
            Operator::Subtract => '-',
            Operator::Multiply => '*',
            Operator::Divide => '/',
            Operator::Power => '^',
            Operator::Factorial => '!',
            Operator::Percent => '%',
            Operator::Open => '(',
            Operator::Close => ')',
            Operator::BitAnd => '&',
            Operator::BitOr => '|',
            Operator::BitNot => '~',
            Operator::BitXor => '⊻',
            Operator::ShiftLeft => '«',
            Operator::ShiftRight => '»',
            Operator::Equal => '≡',
            Operator::NotEqual => '≠',
            Operator::Less => '<',
            Operator::Greater => '>',
            Operator::LessEqual => '≤',
            Operator::GreaterEqual => '≥',
        }
    }

    /// How tightly the operator binds, as upstream's `CalcOperator.bindingPower`.
    ///
    /// Upstream answers `nil` for the operators that are not binary: the parenthesis,
    /// the factorial and the percent sign.
    pub fn binding_power(self) -> Option<i32> {
        Some(match self {
            Operator::Equal
            | Operator::NotEqual
            | Operator::Less
            | Operator::Greater
            | Operator::LessEqual
            | Operator::GreaterEqual => 2,
            Operator::BitOr => 5,
            Operator::BitXor => 6,
            Operator::BitAnd => 7,
            Operator::ShiftLeft | Operator::ShiftRight => 8,
            Operator::Add | Operator::Subtract => 10,
            Operator::Multiply | Operator::Divide => 20,
            Operator::Power => 30,
            Operator::Open
            | Operator::Close
            | Operator::Factorial
            | Operator::Percent
            | Operator::BitNot => return None,
        })
    }

    /// The spelling the card shows, as upstream's `CalcOperator.text`.
    pub fn text(self) -> String {
        match self {
            Operator::Equal => "==".to_string(),
            Operator::ShiftLeft => "<<".to_string(),
            Operator::ShiftRight => ">>".to_string(),
            Operator::BitXor => "xor".to_string(),
            other => other.glyph().to_string(),
        }
    }
}

/// A piece of a tokenized query, mirroring upstream `CalcToken`.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A plain number, such as `12.5`.
    Number(f64),
    /// A shorthand literal such as `10k`, `1e5` or `2 million`, kept distinct so that a
    /// lone one still earns a card.
    CompactNumber(f64),
    /// A radix-prefixed integer literal (`0xff`, `0b1010`, `0o777`), kept exact for base
    /// conversion.
    IntLiteral(u64, NumberBase),
    /// A word: a function, a constant, a unit or a currency.
    Ident(String),
    /// An operator.
    Op(Operator),
    /// `->` or `→`.
    Arrow,
    /// `,`, the argument separator.
    Comma,
}

impl Token {
    /// The number a token carries, as upstream's `CalcQuantity.numberValue`.
    ///
    /// Upstream reads plain and shorthand numbers; every other token has no value.
    pub fn number_value(&self) -> Option<f64> {
        match self {
            Token::Number(value) | Token::CompactNumber(value) => Some(*value),
            _ => None,
        }
    }
}

/// The runs a pattern leaves open, as upstream's `[CalcToken].operands(of:)`.
///
/// The pattern is a list of tokens and holes; every hole collects the tokens between
/// the fixed parts around it. Upstream answers `nil` unless the whole query matches the
/// pattern.
pub fn operands_of(tokens: &[Token], pattern: &[Option<Token>]) -> Option<Vec<Vec<Token>>> {
    let mut operands: Vec<Vec<Token>> = Vec::new();
    let mut cursor = 0usize;
    for (index, part) in pattern.iter().enumerate() {
        if let Some(fixed) = part {
            if cursor >= tokens.len() || &tokens[cursor] != fixed {
                return None;
            }
            cursor += 1;
            continue;
        }
        let mut end = tokens.len();
        if index + 1 < pattern.len() {
            let next: &Token = pattern[index + 1].as_ref()?;
            let found = if cursor < tokens.len() {
                tokens
                    .iter()
                    .skip(cursor + 1)
                    .position(|token| token == next)
                    .map(|offset| offset + cursor + 1)
            } else {
                None
            };
            end = found?;
        }
        if end <= cursor {
            return None;
        }
        operands.push(tokens[cursor..end].to_vec());
        cursor = end;
    }
    if cursor == tokens.len() {
        Some(operands)
    } else {
        None
    }
}
