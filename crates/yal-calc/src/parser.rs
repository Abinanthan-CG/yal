//! Tokenizer and parser for calculator expressions.
//!
//! The grammar is a conventional precedence-climbing expression language
//! extended with two domain features: quantities (`10 km`), and the `in`
//! operator that converts between units, currencies and time zones
//! (`10 km in miles`).

use chrono::{DateTime, Utc};

use crate::units::UnitDef;

/// A lexical token.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    /// An identifier or keyword, lowercased.
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Comma,
    /// The `in` conversion operator.
    In,
    End,
}

/// A tokenizer failure.
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub position: usize,
}

/// Tokenize an expression.
pub fn lex(input: &str) -> Result<Vec<Token>, LexError> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];

        if ch.is_whitespace() {
            index += 1;
            continue;
        }

        match ch {
            '+' => {
                tokens.push(Token::Plus);
                index += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                index += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                index += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                index += 1;
            }
            '%' => {
                tokens.push(Token::Percent);
                index += 1;
            }
            '^' => {
                tokens.push(Token::Caret);
                index += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                index += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                index += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                index += 1;
            }
            _ => {
                if ch.is_ascii_digit() || (ch == '.' && index + 1 < chars.len() && chars[index + 1].is_ascii_digit())
                {
                    let start = index;
                    let mut radix = 10u32;

                    // Hexadecimal, binary and octal literals.
                    if ch == '0' && index + 1 < chars.len() {
                        let marker = chars[index + 1].to_ascii_lowercase();
                        match marker {
                            'x' => {
                                radix = 16;
                                index += 2;
                            }
                            'b' => {
                                radix = 2;
                                index += 2;
                            }
                            'o' => {
                                radix = 8;
                                index += 2;
                            }
                            _ => {}
                        }
                    }

                    let digits_start = index;
                    while index < chars.len()
                        && (chars[index].is_alphanumeric() || chars[index] == '_')
                    {
                        index += 1;
                    }

                    let mut literal: String = chars[digits_start..index]
                        .iter()
                        .filter(|c| **c != '_')
                        .collect();
                    literal = literal.trim_end_matches(|c: char| c.is_ascii_alphabetic() && radix == 10).to_string();

                    // A decimal point or exponent only applies to base ten.
                    if radix == 10 {
                        while index < chars.len()
                            && (chars[index].is_ascii_digit()
                                || chars[index] == '.'
                                || chars[index] == 'e'
                                || chars[index] == 'E'
                                || ((chars[index] == '+' || chars[index] == '-')
                                    && matches!(chars[index - 1], 'e' | 'E')))
                        {
                            if chars[index] == '.' {
                                // Only one decimal point is allowed.
                                if literal.contains('.') {
                                    break;
                                }
                                literal.push('.');
                            } else if chars[index] == 'e' || chars[index] == 'E' {
                                literal.push('e');
                            } else {
                                literal.push(chars[index]);
                            }
                            index += 1;
                        }
                    }

                    let value = if radix == 10 {
                        literal.parse::<f64>().map_err(|_| LexError {
                            message: format!("`{literal}` is not a number"),
                            position: start,
                        })?
                    } else {
                        let parsed = u64::from_str_radix(&literal, radix).map_err(|_| LexError {
                            message: format!("`{literal}` is not a base-{radix} number"),
                            position: start,
                        })?;
                        parsed as f64
                    };

                    tokens.push(Token::Number(value));
                } else if ch.is_alphabetic() || ch == '_' {
                    let start = index;
                    while index < chars.len()
                        && (chars[index].is_alphanumeric() || chars[index] == '_')
                    {
                        index += 1;
                    }
                    let word: String = chars[start..index].iter().collect();
                    let lowered = word.to_lowercase();
                    if lowered == "in" {
                        tokens.push(Token::In);
                    } else {
                        tokens.push(Token::Ident(lowered));
                    }
                } else {
                    return Err(LexError {
                        message: format!("unexpected character `{ch}`"),
                        position: index,
                    });
                }
            }
        }
    }

    tokens.push(Token::End);
    Ok(tokens)
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Plus,
}

/// The parsed expression tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    /// A resolved instant, such as `now` or `today`.
    Instant(DateTime<Utc>),
    /// A number carrying a unit, such as `10 km`.
    Quantity { amount: f64, unit: &'static UnitDef },
    /// An amount of currency, such as `100 usd`.
    Currency { amount: f64, code: &'static str },
    /// A named constant or unknown identifier.
    Ident(String),
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// A function call.
    Call { name: String, args: Vec<Expr> },
    /// `lhs in rhs` — a conversion.
    Convert { lhs: Box<Expr>, rhs: Box<Expr> },
    /// A trailing percent sign, which divides by one hundred.
    Percent(Box<Expr>),
}

/// A parse failure.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

/// Parse an expression into a tree.
pub fn parse(input: &str) -> Result<Expr, ParseError> {
    let tokens = lex(input).map_err(|err| ParseError {
        message: err.message,
        position: err.position,
    })?;
    let mut parser = Parser { tokens, index: 0 };
    let expr = parser.parse_expr()?;
    if parser.peek() != &Token::End {
        return Err(ParseError {
            message: "unexpected trailing input".into(),
            position: 0,
        });
    }
    Ok(expr)
}

/// A recursive-descent parser.
struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn next(&mut self) -> Token {
        let token = self.tokens[self.index].clone();
        if self.index + 1 < self.tokens.len() {
            self.index += 1;
        }
        token
    }

    /// The entry point: a full expression.
    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_term()?;
        loop {
            match self.peek() {
                Token::Plus => {
                    self.next();
                    let rhs = self.parse_term()?;
                    lhs = Expr::Binary {
                        op: BinOp::Add,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                Token::Minus => {
                    self.next();
                    let rhs = self.parse_term()?;
                    lhs = Expr::Binary {
                        op: BinOp::Sub,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                Token::In => {
                    self.next();
                    let rhs = self.parse_term()?;
                    lhs = Expr::Convert {
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                _ => return Ok(lhs),
            }
        }
    }

    /// Multiplicative precedence.
    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;
        loop {
            match self.peek() {
                Token::Star => {
                    self.next();
                    let rhs = self.parse_power()?;
                    lhs = Expr::Binary {
                        op: BinOp::Mul,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                Token::Slash => {
                    self.next();
                    let rhs = self.parse_power()?;
                    lhs = Expr::Binary {
                        op: BinOp::Div,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                Token::Percent => {
                    self.next();
                    let rhs = self.parse_power()?;
                    lhs = Expr::Binary {
                        op: BinOp::Rem,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                _ => return Ok(lhs),
            }
        }
    }

    /// Unary prefix operators.
    ///
    /// A leading minus binds looser than exponentiation, so `-2^2` parses as
    /// `-(2^2)` rather than `(-2)^2`. This matches ordinary mathematical
    /// convention and avoids the surprise of `-2^2` evaluating to `4`.
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Token::Minus => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                })
            }
            Token::Plus => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Plus,
                    expr: Box::new(expr),
                })
            }
            _ => self.parse_power(),
        }
    }

    /// Exponentiation is right-associative: `2^3^2` is `2^(3^2)`.
    fn parse_power(&mut self) -> Result<Expr, ParseError> {
        let base = self.parse_postfix()?;
        if self.peek() == &Token::Caret {
            self.next();
            let exponent = self.parse_unary()?;
            return Ok(Expr::Binary {
                op: BinOp::Pow,
                lhs: Box::new(base),
                rhs: Box::new(exponent),
            });
        }
        Ok(base)
    }

    /// A trailing `%` turns a value into a percentage.
    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_primary()?;
        if self.peek() == &Token::Percent {
            self.next();
            return Ok(Expr::Percent(Box::new(expr)));
        }
        Ok(expr)
    }

    /// Primary expressions: literals, identifiers, calls and parentheses.
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.next() {
            Token::Number(value) => {
                // A number directly followed by a unit or currency name is a
                // quantity rather than a bare number.
                if let Token::Ident(name) = self.peek().clone() {
                    if let Some(unit) = crate::units::UnitRegistry::new().resolve(&name) {
                        self.next();
                        return Ok(Expr::Quantity { amount: value, unit });
                    }
                    if let Some(currency) =
                        crate::currency::CurrencyRegistry::new().resolve(&name)
                    {
                        self.next();
                        return Ok(Expr::Currency {
                            amount: value,
                            code: currency.code,
                        });
                    }
                }
                Ok(Expr::Number(value))
            }
            Token::Ident(name) => {
                if self.peek() == &Token::LParen {
                    self.next();
                    let mut args = Vec::new();
                    if self.peek() != &Token::RParen {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.peek() == &Token::Comma {
                                self.next();
                                continue;
                            }
                            break;
                        }
                    }
                    if self.next() != Token::RParen {
                        return Err(ParseError {
                            message: "expected `)`".into(),
                            position: 0,
                        });
                    }
                    return Ok(Expr::Call { name, args });
                }
                Ok(Expr::Ident(name))
            }
            Token::LParen => {
                let expr = self.parse_expr()?;
                if self.next() != Token::RParen {
                    return Err(ParseError {
                        message: "expected `)`".into(),
                        position: 0,
                    });
                }
                Ok(expr)
            }
            Token::End => Err(ParseError {
                message: "unexpected end of expression".into(),
                position: 0,
            }),
            other => Err(ParseError {
                message: format!("unexpected token {other:?}"),
                position: 0,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_numbers_and_operators() {
        let tokens = lex("1 + 2.5 * 3").unwrap();
        assert_eq!(tokens[0], Token::Number(1.0));
        assert_eq!(tokens[1], Token::Plus);
        assert_eq!(tokens[2], Token::Number(2.5));
        assert_eq!(tokens[3], Token::Star);
        assert_eq!(tokens[4], Token::Number(3.0));
        assert_eq!(tokens[5], Token::End);
    }

    #[test]
    fn lexes_hex_binary_and_octal() {
        assert_eq!(lex("0xff").unwrap()[0], Token::Number(255.0));
        assert_eq!(lex("0b1010").unwrap()[0], Token::Number(10.0));
        assert_eq!(lex("0o17").unwrap()[0], Token::Number(15.0));
    }

    #[test]
    fn lexes_exponents() {
        assert_eq!(lex("1e3").unwrap()[0], Token::Number(1000.0));
        assert_eq!(lex("1.5e-2").unwrap()[0], Token::Number(0.015));
    }

    #[test]
    fn recognises_the_in_operator() {
        assert_eq!(lex("km in mi").unwrap()[1], Token::In);
    }

    #[test]
    fn rejects_unknown_characters() {
        assert!(lex("1 $ 2").is_err());
    }

    #[test]
    fn parses_unit_quantities() {
        let expr = parse("10 km").unwrap();
        match expr {
            Expr::Quantity { amount, unit } => {
                assert_eq!(amount, 10.0);
                assert_eq!(unit.id, "km");
            }
            other => panic!("expected quantity, got {other:?}"),
        }
    }

    #[test]
    fn parses_conversion() {
        let expr = parse("10 km in miles").unwrap();
        assert!(matches!(expr, Expr::Convert { .. }));
    }

    #[test]
    fn exponentiation_is_right_associative() {
        let expr = parse("2 ^ 3 ^ 2").unwrap();
        match expr {
            Expr::Binary {
                op: BinOp::Pow,
                rhs,
                ..
            } => assert!(matches!(*rhs, Expr::Binary { op: BinOp::Pow, .. })),
            other => panic!("expected nested power, got {other:?}"),
        }
    }

    #[test]
    fn unary_minus_binds_looser_than_power() {
        let expr = parse("-2^2").unwrap();
        match expr {
            Expr::Unary { expr, .. } => assert!(matches!(*expr, Expr::Binary { op: BinOp::Pow, .. })),
            other => panic!("expected unary, got {other:?}"),
        }
    }

    #[test]
    fn parses_function_calls() {
        let expr = parse("max(1, 2)").unwrap();
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "max");
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected call, got {other:?}"),
        }
    }
}
