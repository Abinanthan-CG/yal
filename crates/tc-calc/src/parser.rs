//! The expression parser: tokens in, a value out.
//!
//! Mirrors upstream `CalcExpressionParser.swift`. Upstream's parser also carries units
//! and currencies through a calculation; both live outside this crate, so a name the
//! catalog knows is reported as unsupported instead of being guessed at.

use crate::catalog::Catalog;
use crate::currency::CurrencyDef;
use crate::math::Math;
use crate::token::{Operator, Token};
use crate::value::{ParseError, UnsupportedArea, Value};

/// How tightly a unary sign binds, as upstream's `CalcExpressionParser.unaryBindingPower`.
///
/// A unary minus binds looser than `^` and tighter than `*`.
const UNARY_BINDING_POWER: i32 = 25;

/// A binary operator the parser found, as upstream's `CalcExpressionParser.BinaryOp`.
struct BinaryOp {
    /// The operator itself.
    op: Operator,
    /// How tightly it binds.
    binding_power: i32,
    /// The binding power of the operand on its right.
    right_binding_power: i32,
    /// Whether the operator's own token is consumed before the right operand is parsed.
    consumes_token: bool,
}

/// The parser of a tokenized query, as upstream's `CalcExpressionParser`.
pub struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
    operation_count: usize,
}

impl<'a> Parser<'a> {
    /// A parser over `tokens`.
    pub fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            position: 0,
            operation_count: 0,
        }
    }

    /// How many operations the parse applied, as upstream's `operationCount`.
    ///
    /// The engine uses it to tell a calculation from a lone literal.
    pub fn operation_count(&self) -> usize {
        self.operation_count
    }

    /// Parses the tokens into a value, as upstream's `CalcExpressionParser.parse`.
    ///
    /// Upstream answers `nil` when the tokens are not a whole calculation, which is
    /// [`ParseError::NotACalculation`] here.
    pub fn parse(&mut self, catalog: &dyn Catalog) -> Result<Value, ParseError> {
        let value = self.parse_expression(0, catalog)?;
        if self.position != self.tokens.len() {
            return Err(ParseError::NotACalculation);
        }
        if !value.effective().is_finite() {
            return Err(ParseError::NotACalculation);
        }
        Ok(value)
    }

    /// The token the parser is looking at.
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    /// The number a token carries, as upstream's `CalcQuantity.numberValue`.
    fn number_at(&self, index: usize) -> Option<f64> {
        self.tokens.get(index).and_then(Token::number_value)
    }

    /// Parses an expression, as upstream's `CalcExpressionParser.parseExpression`.
    fn parse_expression(
        &mut self,
        min_binding_power: i32,
        catalog: &dyn Catalog,
    ) -> Result<Value, ParseError> {
        let mut left = self.parse_operand(catalog)?;
        if !left.effective().is_finite() {
            return Err(ParseError::NotACalculation);
        }
        loop {
            let Some(binary) = self.peek_binary() else {
                return Ok(left);
            };
            if binary.binding_power < min_binding_power {
                return Ok(left);
            }
            if binary.consumes_token {
                self.position += 1;
            }
            self.operation_count += 1;
            let right = self.parse_expression(binary.right_binding_power, catalog)?;
            let combined = self.apply(binary.op, left, right)?;
            if !combined.effective().is_finite() {
                return Err(ParseError::NotACalculation);
            }
            left = combined;
        }
    }

    /// The binary operator that follows, as upstream's
    /// `CalcExpressionParser.peekBinary`.
    fn peek_binary(&self) -> Option<BinaryOp> {
        match self.current()? {
            Token::Op(op) => {
                if let Some(power) = op.binding_power() {
                    let extra = i32::from(*op != Operator::Power);
                    return Some(BinaryOp {
                        op: *op,
                        binding_power: power,
                        right_binding_power: power + extra,
                        consumes_token: true,
                    });
                }
                // A parenthesis multiplies what stands before it: `4(2+3)`.
                if *op == Operator::Open {
                    return Some(BinaryOp {
                        op: Operator::Multiply,
                        binding_power: 20,
                        right_binding_power: 21,
                        consumes_token: false,
                    });
                }
                None
            }
            Token::Ident(name) => {
                let named = match name.as_str() {
                    "mod" => Some(BinaryOp {
                        op: Operator::Percent,
                        binding_power: 20,
                        right_binding_power: 21,
                        consumes_token: true,
                    }),
                    "power" => Some(BinaryOp {
                        op: Operator::Power,
                        binding_power: 30,
                        right_binding_power: 30,
                        consumes_token: true,
                    }),
                    "xor" => Some(BinaryOp {
                        op: Operator::BitXor,
                        binding_power: 6,
                        right_binding_power: 7,
                        consumes_token: true,
                    }),
                    "of" => Some(BinaryOp {
                        op: Operator::Multiply,
                        binding_power: 20,
                        right_binding_power: 21,
                        consumes_token: true,
                    }),
                    _ => None,
                };
                if named.is_some() {
                    return named;
                }
                let follows_root = (name == "square" || name == "cube")
                    && self.tokens.get(self.position + 1)
                        == Some(&Token::Ident("root".to_string()));
                if Math::constant(name).is_some() || Math::is_function(name) || follows_root {
                    return Some(BinaryOp {
                        op: Operator::Multiply,
                        binding_power: 20,
                        right_binding_power: 21,
                        consumes_token: false,
                    });
                }
                None
            }
            _ => None,
        }
    }

    /// Applies a binary operator, as upstream's `CalcExpressionParser.apply`.
    fn apply(&mut self, op: Operator, left: Value, right: Value) -> Result<Value, ParseError> {
        match op {
            Operator::Equal
            | Operator::NotEqual
            | Operator::Less
            | Operator::Greater
            | Operator::LessEqual
            | Operator::GreaterEqual => {
                let amount = right.effective();
                let left_amount = left.effective();
                let result = match op {
                    Operator::Equal => left_amount == amount,
                    Operator::NotEqual => left_amount != amount,
                    Operator::Less => left_amount < amount,
                    Operator::Greater => left_amount > amount,
                    Operator::LessEqual => left_amount <= amount,
                    _ => left_amount >= amount,
                };
                Ok(Value {
                    amount: if result { 1.0 } else { 0.0 },
                    is_percent: false,
                    is_boolean: true,
                    currency: None,
                })
            }
            Operator::Percent => Ok(Value::scalar(left.effective() % right.effective())),
            Operator::BitAnd
            | Operator::BitOr
            | Operator::BitXor
            | Operator::ShiftLeft
            | Operator::ShiftRight => {
                let result = Math::bitwise(op, left.effective(), right.effective())
                    .ok_or(ParseError::NotACalculation)?;
                Ok(Value::scalar(result))
            }
            Operator::Add | Operator::Subtract => {
                let direction = if op == Operator::Add { 1.0 } else { -1.0 };
                if right.is_percent {
                    let output = left.effective() * (1.0 + direction * right.amount / 100.0);
                    return Ok(Value {
                        amount: output,
                        is_percent: false,
                        is_boolean: false,
                        currency: left.currency,
                    });
                }
                let currency = combine_currency(left.currency, right.currency)?;
                Ok(Value {
                    amount: left.effective() + direction * right.effective(),
                    is_percent: false,
                    is_boolean: false,
                    currency,
                })
            }
            Operator::Multiply => {
                let currency = combine_currency(left.currency, right.currency)?;
                Ok(Value {
                    amount: left.effective() * right.effective(),
                    is_percent: false,
                    is_boolean: false,
                    currency,
                })
            }
            Operator::Divide => {
                if right.effective() == 0.0 {
                    return Err(ParseError::NotACalculation);
                }
                let currency = combine_currency(left.currency, right.currency)?;
                Ok(Value {
                    amount: left.effective() / right.effective(),
                    is_percent: false,
                    is_boolean: false,
                    currency,
                })
            }
            Operator::Power => Ok(Value::scalar(left.effective().powf(right.effective()))),
            _ => Err(ParseError::NotACalculation),
        }
    }

    /// Parses an operand with its postfixes, as upstream's
    /// `CalcExpressionParser.parseOperand`.
    fn parse_operand(&mut self, catalog: &dyn Catalog) -> Result<Value, ParseError> {
        let mut value = self.parse_prefix(catalog)?;
        loop {
            match self.current() {
                Some(Token::Ident(name)) => {
                    if value.is_percent || value.is_boolean {
                        return Ok(value);
                    }
                    // `100 usd` is money in US dollars.
                    if value.currency.is_none() {
                        if let Some(currency) = catalog.currency_definition(name) {
                            value.currency = Some(currency);
                            self.position += 1;
                            continue;
                        }
                    }
                    check_dimension(name, catalog)?;
                    return Ok(value);
                }
                Some(Token::Op(Operator::Percent)) => {
                    if value.is_percent || value.is_boolean {
                        return Err(ParseError::NotACalculation);
                    }
                    value.is_percent = true;
                    self.position += 1;
                }
                Some(Token::Op(Operator::Factorial)) => {
                    if value.is_percent || value.is_boolean {
                        return Err(ParseError::NotACalculation);
                    }
                    let factorial =
                        Math::factorial(value.amount).ok_or(ParseError::NotACalculation)?;
                    value.amount = factorial;
                    self.position += 1;
                }
                _ => return Ok(value),
            }
        }
    }

    /// Parses a prefix, as upstream's `CalcExpressionParser.parsePrefix`.
    fn parse_prefix(&mut self, catalog: &dyn Catalog) -> Result<Value, ParseError> {
        let current = self.current().cloned().ok_or(ParseError::NotACalculation)?;
        match current {
            Token::Number(value) | Token::CompactNumber(value) => {
                self.position += 1;
                Ok(Value::scalar(value))
            }
            Token::IntLiteral(value, _) => {
                self.position += 1;
                Ok(Value::scalar(value as f64))
            }
            Token::Op(Operator::BitNot) => {
                self.position += 1;
                let value = self.parse_expression(UNARY_BINDING_POWER, catalog)?;
                if value.is_boolean {
                    return Err(ParseError::NotACalculation);
                }
                let result = Math::bitwise(Operator::BitNot, value.effective(), 0.0)
                    .ok_or(ParseError::NotACalculation)?;
                Ok(Value::scalar(result))
            }
            Token::Op(Operator::Subtract) => {
                self.position += 1;
                let value = self.parse_expression(UNARY_BINDING_POWER, catalog)?;
                if value.is_boolean {
                    return Err(ParseError::NotACalculation);
                }
                Ok(Value::scalar(-value.effective()))
            }
            Token::Op(Operator::Add) => {
                self.position += 1;
                let value = self.parse_expression(UNARY_BINDING_POWER, catalog)?;
                if value.is_boolean {
                    return Err(ParseError::NotACalculation);
                }
                Ok(value)
            }
            Token::Op(Operator::Open) => self.parse_grouped(catalog),
            Token::Ident(name) => self.parse_named(&name, catalog),
            _ => Err(ParseError::NotACalculation),
        }
    }

    /// Parses a word, as upstream's `CalcExpressionParser.parsePrefix` does for an
    /// identifier.
    fn parse_named(&mut self, name: &str, catalog: &dyn Catalog) -> Result<Value, ParseError> {
        let follows_root =
            self.tokens.get(self.position + 1) == Some(&Token::Ident("root".to_string()));
        if (name == "square" || name == "cube") && follows_root {
            self.position += 2;
            if self.current() == Some(&Token::Ident("of".to_string())) {
                self.position += 1;
            }
            let value = self.parse_operand(catalog)?;
            if value.is_boolean {
                return Err(ParseError::NotACalculation);
            }
            self.operation_count += 1;
            let degree = if name == "square" { 2.0 } else { 3.0 };
            return Ok(root(value, degree));
        }
        if name == "root" {
            let degree = self.number_at(self.position + 1);
            let follows_of =
                self.tokens.get(self.position + 2) == Some(&Token::Ident("of".to_string()));
            if let Some(degree) = degree.filter(|_| follows_of) {
                self.position += 3;
                if degree < 2.0 || degree.round() != degree {
                    return Err(ParseError::NotACalculation);
                }
                let value = self.parse_operand(catalog)?;
                if value.is_boolean {
                    return Err(ParseError::NotACalculation);
                }
                self.operation_count += 1;
                return Ok(root(value, degree));
            }
        }
        if let Some(constant) = Math::constant(name) {
            self.position += 1;
            return Ok(Value::scalar(constant));
        }
        let opens_an_argument_list =
            self.tokens.get(self.position + 1) == Some(&Token::Op(Operator::Open));
        if Math::MULTIPLE_ARGUMENTS.contains(&name) && opens_an_argument_list {
            return self.parse_function(name, catalog);
        }
        if Math::is_single_argument(name) {
            self.position += 1;
            let argument = self.parse_operand(catalog)?;
            if argument.is_boolean {
                return Err(ParseError::NotACalculation);
            }
            self.operation_count += 1;
            if name == "log" && self.current() == Some(&Token::Ident("base".to_string())) {
                self.position += 1;
                let base = self.parse_operand(catalog)?;
                if base.is_boolean {
                    return Err(ParseError::NotACalculation);
                }
                let result = Math::evaluate("log", &[argument.effective(), base.effective()])
                    .ok_or(ParseError::NotACalculation)?;
                return Ok(Value::scalar(result));
            }
            let result = Math::apply_single(name, argument.effective())
                .ok_or(ParseError::NotACalculation)?;
            return Ok(Value::scalar(result));
        }
        // A currency before its amount: `$100`.
        if let Some(currency) = catalog.currency_definition(name) {
            if let Some(amount) = self.number_at(self.position + 1) {
                self.position += 2;
                return Ok(Value::money(amount, currency));
            }
        }
        check_dimension(name, catalog)?;
        Err(ParseError::NotACalculation)
    }

    /// Parses a function with an argument list, as upstream's
    /// `CalcExpressionParser.parseFunction`.
    fn parse_function(&mut self, name: &str, catalog: &dyn Catalog) -> Result<Value, ParseError> {
        self.position += 2;
        let mut amounts: Vec<f64> = Vec::new();
        loop {
            let end = self.argument_end().ok_or(ParseError::NotACalculation)?;
            let value = self.parse_scope(end, catalog)?;
            if value.is_boolean {
                return Err(ParseError::NotACalculation);
            }
            amounts.push(value.effective());
            self.position = end + 1;
            if self.tokens[end] == Token::Op(Operator::Close) {
                break;
            }
        }
        self.operation_count += 1;
        let result = Math::evaluate(name, &amounts).ok_or(ParseError::NotACalculation)?;
        Ok(Value::scalar(result))
    }

    /// Parses a bracketed group, as upstream's `CalcExpressionParser.parseGrouped`.
    fn parse_grouped(&mut self, catalog: &dyn Catalog) -> Result<Value, ParseError> {
        let close = self
            .matching_parenthesis()
            .ok_or(ParseError::NotACalculation)?;
        self.position += 1;
        let value = self.parse_scope(close, catalog)?;
        self.position = close + 1;
        Ok(value)
    }

    /// Parses the scope that ends at `end`, as upstream's
    /// `CalcExpressionParser.parseScope`.
    fn parse_scope(&mut self, end: usize, catalog: &dyn Catalog) -> Result<Value, ParseError> {
        let value = self.parse_scoped_value(end, catalog)?;
        self.position = end;
        Ok(value)
    }

    /// Parses what stands inside a scope, as upstream's
    /// `CalcExpressionParser.parseScopedValue`.
    fn parse_scoped_value(
        &mut self,
        end: usize,
        catalog: &dyn Catalog,
    ) -> Result<Value, ParseError> {
        if end == self.position + 1 {
            if let Some(Token::Ident(name)) = self.tokens.get(self.position) {
                check_dimension(name, catalog)?;
            }
        }
        let value = self.parse_expression(0, catalog)?;
        if self.position != end {
            return Err(ParseError::NotACalculation);
        }
        Ok(value)
    }

    /// The end of the current argument, as upstream's
    /// `CalcExpressionParser.argumentEnd`.
    fn argument_end(&self) -> Option<usize> {
        let mut depth = 0i32;
        for index in self.position..self.tokens.len() {
            match self.tokens[index] {
                Token::Op(Operator::Open) => depth += 1,
                Token::Op(Operator::Close) => {
                    if depth == 0 {
                        return Some(index);
                    }
                    depth -= 1;
                }
                Token::Comma if depth == 0 => return Some(index),
                _ => {}
            }
        }
        None
    }

    /// The parenthesis that closes the current group, as upstream's
    /// `CalcExpressionParser.matchingParenthesis`.
    fn matching_parenthesis(&self) -> Option<usize> {
        if self.current() != Some(&Token::Op(Operator::Open)) {
            return None;
        }
        let mut depth = 0i32;
        for index in self.position..self.tokens.len() {
            if self.tokens[index] == Token::Op(Operator::Open) {
                depth += 1;
            }
            if self.tokens[index] == Token::Op(Operator::Close) {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
        }
        None
    }
}

/// The root of a value, as upstream's `CalcExpressionParser.root`.
///
/// Odd roots keep the sign, so the cube root of -8 is -2 rather than no answer.
fn root(value: Value, degree: f64) -> Value {
    if degree % 2.0 != 1.0 || value.effective() >= 0.0 {
        return Value::scalar(value.effective().powf(1.0 / degree));
    }
    Value::scalar(-(-value.effective()).powf(1.0 / degree))
}

/// Reports a word the catalog knows as a unit or a currency.
///
/// Upstream resolves those names into a dimension and carries them through the
/// calculation; the units live outside this crate, so a unit is reported as
/// unsupported rather than answered with a made-up number. A currency the catalog
/// can define is carried; one it can only name is reported as unsupported too.
fn check_dimension(name: &str, catalog: &dyn Catalog) -> Result<(), ParseError> {
    let lowered = name.to_lowercase();
    if catalog.unit(name).is_some() || catalog.unit(&lowered).is_some() {
        return Err(ParseError::Unsupported(UnsupportedArea::units(
            name.to_string(),
        )));
    }
    if catalog.currency_definition(name).is_some()
        || catalog.currency_definition(&lowered).is_some()
    {
        return Ok(());
    }
    if catalog.currency(name).is_some() || catalog.currency(&lowered).is_some() {
        return Err(ParseError::Unsupported(UnsupportedArea::currency(
            name.to_string(),
        )));
    }
    Ok(())
}

/// The currency a calculation of two values keeps, as upstream's
/// `CalcExpressionParser.apply` decides.
///
/// A currency beside a plain amount stays in the currency, and two amounts of the
/// same currency stay in it too. Two *different* currencies need the rate feed,
/// which the port does not have, so the calculation is refused rather than answered
/// with a made-up number.
fn combine_currency(
    left: Option<CurrencyDef>,
    right: Option<CurrencyDef>,
) -> Result<Option<CurrencyDef>, ParseError> {
    match (left, right) {
        (Some(first), Some(second)) if first.code != second.code => {
            Err(ParseError::NotACalculation)
        }
        (Some(first), _) => Ok(Some(first)),
        (None, second) => Ok(second),
    }
}
