//! The expression parser: tokens in, a value out.
//!
//! Mirrors upstream `CalcExpressionParser.swift`. Upstream's parser carries units and
//! currencies through a calculation; both live in this crate now, so a value keeps the
//! unit it was written with and answers in the unit the last one named.

use crate::catalog::Catalog;
use crate::currency::CurrencyDef;
use crate::math::Math;
use crate::measure::{self, Measurement};
use crate::token::{Operator, Token};
use crate::units::{Dimension, UnitCategory};
use crate::value::{ParseError, UnsupportedArea, Value};

/// How tightly a unary sign binds, as upstream's `CalcExpressionParser.unaryBindingPower`.
///
/// A unary minus binds looser than `^` and tighter than `*`.
const UNARY_BINDING_POWER: i32 = 25;

/// How tightly two quantities written side by side bind, as upstream's
/// `CalcExpressionParser.compositeBindingPower`.
///
/// Composite notation binds above multiplication, so `5 feet 3 inches` is one amount
/// rather than five feet times three inches.
const COMPOSITE_BINDING_POWER: i32 = 40;

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
    dimension_count: usize,
}

impl<'a> Parser<'a> {
    /// A parser over `tokens`.
    pub fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            position: 0,
            operation_count: 0,
            dimension_count: 0,
        }
    }

    /// How many operations the parse applied, as upstream's `operationCount`.
    ///
    /// The engine uses it to tell a calculation from a lone literal.
    pub fn operation_count(&self) -> usize {
        self.operation_count
    }

    /// How many units and currencies the parse named, as upstream's `dimensionCount`.
    ///
    /// The engine uses it to tell a lone unit from a calculation that carries one.
    pub fn dimension_count(&self) -> usize {
        self.dimension_count
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
            let Some(binary) = self.peek_binary(&left, catalog) else {
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
            let combined = self.apply(binary.op, left, right, !binary.consumes_token)?;
            if !combined.effective().is_finite() {
                return Err(ParseError::NotACalculation);
            }
            left = combined;
        }
    }

    /// The binary operator that follows, as upstream's
    /// `CalcExpressionParser.peekBinary`.
    ///
    /// Two quantities written side by side make an operator that is not written down:
    /// the `+` between them is invisible, and it binds tighter than a multiplication.
    fn peek_binary(&self, left: &Value, catalog: &dyn Catalog) -> Option<BinaryOp> {
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
                self.composite(left, catalog)
            }
            _ => self.composite(left, catalog),
        }
    }

    /// The invisible `+` between two quantities written side by side, as upstream's
    /// `CalcExpressionParser.peekBinary` reports it.
    ///
    /// `5 feet 3 inches` is one amount rather than a sum, so the operator binds
    /// tighter than a multiplication.
    fn composite(&self, left: &Value, catalog: &dyn Catalog) -> Option<BinaryOp> {
        left.unit?;
        if !self.starts_quantity(catalog) {
            return None;
        }
        Some(BinaryOp {
            op: Operator::Add,
            binding_power: COMPOSITE_BINDING_POWER,
            right_binding_power: COMPOSITE_BINDING_POWER + 1,
            consumes_token: false,
        })
    }
    /// Whether a quantity stands at the cursor, as upstream's
    /// `CalcExpressionParser.startsQuantity`.
    fn starts_quantity(&self, catalog: &dyn Catalog) -> bool {
        match self.current() {
            Some(Token::Number(_))
            | Some(Token::CompactNumber(_))
            | Some(Token::IntLiteral(_, _)) => true,
            Some(Token::Ident(name)) => {
                // A currency followed by an amount starts a quantity; a unit does not,
                // so `1hr 30` waits for the unit that is still being typed.
                catalog.currency(name).is_some() && self.number_at(self.position + 1).is_some()
            }
            _ => false,
        }
    }

    /// Applies a binary operator, as upstream's `CalcExpressionParser.apply`.
    ///
    /// `implicit` says the operator was not written down: the two quantities stood
    /// side by side, or a parenthesis followed an amount. Composite notation is
    /// answered in the leading unit, while a written `+` is answered in the last one.
    fn apply(
        &mut self,
        op: Operator,
        left: Value,
        right: Value,
        implicit: bool,
    ) -> Result<Value, ParseError> {
        match op {
            Operator::Equal
            | Operator::NotEqual
            | Operator::Less
            | Operator::Greater
            | Operator::LessEqual
            | Operator::GreaterEqual => {
                let amount = self.comparable(right, left)?;
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
                    unit: None,
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
            Operator::Add | Operator::Subtract => self.add_or_subtract(op, left, right, implicit),
            Operator::Multiply => self.multiply(left, right),
            Operator::Divide => self.divide(left, right),
            Operator::Power => self.power(left, right),
            _ => Err(ParseError::NotACalculation),
        }
    }

    /// The amount the right value stands for in the left value's own unit, as
    /// upstream's `CalcExpressionParser.comparable`.
    fn comparable(&self, right: Value, left: Value) -> Result<f64, ParseError> {
        match (left.unit, right.unit) {
            (Some(target), Some(source)) => {
                if !target.is_compatible_with(&source) {
                    return Err(ParseError::NotACalculation);
                }
                Ok(source.told_in(right.effective(), &target))
            }
            (None, None) => Ok(right.effective()),
            _ => Err(ParseError::NotACalculation),
        }
    }

    /// Adds or subtracts two values, as upstream's
    /// `CalcExpressionParser.addOrSubtract`.
    ///
    /// The last unit typed decides the unit of the answer; two quantities written side
    /// by side keep the unit of the first.
    fn add_or_subtract(
        &mut self,
        op: Operator,
        left: Value,
        right: Value,
        implicit: bool,
    ) -> Result<Value, ParseError> {
        let direction = if op == Operator::Add { 1.0 } else { -1.0 };
        if right.is_percent {
            let output = left.effective() * (1.0 + direction * right.amount / 100.0);
            return Ok(Value {
                amount: output,
                is_percent: false,
                is_boolean: false,
                currency: left.currency,
                unit: left.unit,
            });
        }
        // A measure and an amount of money are two different things.
        if (left.unit.is_some() && right.currency.is_some())
            || (left.currency.is_some() && right.unit.is_some())
        {
            return Err(ParseError::NotACalculation);
        }
        match (left.unit, right.unit) {
            (Some(lhs), Some(rhs)) => {
                if !lhs.is_compatible_with(&rhs) {
                    return Err(ParseError::NotACalculation);
                }
                if lhs.category == UnitCategory::Temperature && lhs.symbol != rhs.symbol {
                    return Err(ParseError::NotACalculation);
                }
                // Composite notation answers in the leading unit; a written sign in the last.
                if implicit {
                    let converted = rhs.told_in(right.amount, &lhs);
                    return Ok(Value::measurement(left.amount + direction * converted, lhs));
                }
                let converted = lhs.told_in(left.amount, &rhs);
                Ok(Value::measurement(
                    converted + direction * right.amount,
                    rhs,
                ))
            }
            (Some(unit), None) => {
                // A bare amount takes the unit beside it, unless the two stood side by
                // side: then the amount is a unit that is still being typed.
                if implicit {
                    return Err(ParseError::NotACalculation);
                }
                Ok(Value::measurement(
                    left.amount + direction * right.effective(),
                    unit,
                ))
            }
            (None, Some(unit)) => {
                if implicit {
                    return Err(ParseError::NotACalculation);
                }
                Ok(Value::measurement(
                    left.effective() + direction * right.amount,
                    unit,
                ))
            }
            (None, None) => {
                let currency = combine_currency(left.currency, right.currency)?;
                Ok(Value {
                    amount: left.effective() + direction * right.effective(),
                    is_percent: false,
                    is_boolean: false,
                    currency,
                    unit: None,
                })
            }
        }
    }

    /// Multiplies two values, as upstream's `CalcExpressionParser.multiply`.
    fn multiply(&mut self, left: Value, right: Value) -> Result<Value, ParseError> {
        if (left.unit.is_some() && right.currency.is_some())
            || (left.currency.is_some() && right.unit.is_some())
        {
            return Err(ParseError::NotACalculation);
        }
        match (left.unit, right.unit) {
            (Some(_), Some(_)) => self.combine(left, right, false),
            (Some(unit), None) => Ok(Value::measurement(left.amount * right.effective(), unit)),
            (None, Some(unit)) => Ok(Value::measurement(left.effective() * right.amount, unit)),
            (None, None) => {
                let currency = combine_currency(left.currency, right.currency)?;
                Ok(Value {
                    amount: left.effective() * right.effective(),
                    is_percent: false,
                    is_boolean: false,
                    currency,
                    unit: None,
                })
            }
        }
    }

    /// Divides one value by another, as upstream's `CalcExpressionParser.divide`.
    fn divide(&mut self, left: Value, right: Value) -> Result<Value, ParseError> {
        if right.effective() == 0.0 {
            return Err(ParseError::NotACalculation);
        }
        if (left.unit.is_some() && right.currency.is_some())
            || (left.currency.is_some() && right.unit.is_some())
        {
            return Err(ParseError::NotACalculation);
        }
        match (left.unit, right.unit) {
            (Some(lhs), Some(rhs)) => {
                if lhs.is_compatible_with(&rhs) && lhs.category != UnitCategory::Temperature {
                    // Two measures of one kind divide into a plain ratio.
                    let numerator = left.amount * lhs.factor;
                    let denominator = right.amount * rhs.factor;
                    return Ok(Value::scalar(numerator / denominator));
                }
                self.combine(left, right, true)
            }
            (Some(unit), None) => Ok(Value::measurement(
                left.effective() / right.effective(),
                unit,
            )),
            (None, Some(rhs)) => {
                // A plain amount over a unit is the inverse of that unit: `1 / 20ms`
                // is fifty hertz.
                let Some(inverted) = measure::power(&rhs, -1.0) else {
                    return Err(ParseError::NotACalculation);
                };
                let Some(dimension) = inverted.dimension else {
                    return Err(ParseError::NotACalculation);
                };
                let unit = measure::base_unit(&dimension).unwrap_or(inverted);
                let amount = left.effective() / (right.amount * rhs.factor);
                Ok(Value::measurement(amount / unit.factor, unit))
            }
            (None, None) => {
                // An amount of money over an amount of money is a plain ratio.
                if left.currency.is_some() && right.currency.is_some() {
                    combine_currency(left.currency, right.currency)?;
                    return Ok(Value::scalar(left.effective() / right.effective()));
                }
                let currency = combine_currency(left.currency, right.currency)?;
                Ok(Value {
                    amount: left.effective() / right.effective(),
                    is_percent: false,
                    is_boolean: false,
                    currency,
                    unit: None,
                })
            }
        }
    }

    /// Raises a value to a power, as upstream's `CalcExpressionParser.power`.
    fn power(&mut self, left: Value, right: Value) -> Result<Value, ParseError> {
        if right.unit.is_some() || right.currency.is_some() {
            return Err(ParseError::NotACalculation);
        }
        let Some(unit) = left.unit else {
            return Ok(Value::scalar(left.effective().powf(right.effective())));
        };
        let Some(dimension) = unit.dimension else {
            return Err(ParseError::NotACalculation);
        };
        let raised = dimension.raised(right.effective());
        let base = left.amount * unit.factor;
        let amount = base.powf(right.effective());
        let chosen = match measure::base_unit(&raised) {
            Some(base_unit) => base_unit,
            None => match measure::power(&unit, right.effective()) {
                Some(compound) => compound,
                None => return Err(ParseError::NotACalculation),
            },
        };
        Ok(Value::measurement(amount / chosen.factor, chosen))
    }

    /// The product or the quotient of two measures, as upstream's
    /// `CalcExpressionParser.combine`.
    ///
    /// The answer is told in the base unit of the dimension it lands on, so five
    /// metres times four metres is twenty square metres; a rate multiplied by a time
    /// keeps the numerator of the rate instead.
    fn combine(&mut self, left: Value, right: Value, dividing: bool) -> Result<Value, ParseError> {
        let (Some(lhs), Some(rhs)) = (left.unit, right.unit) else {
            return Err(ParseError::NotACalculation);
        };
        let (Some(left_dimension), Some(right_dimension)) = (lhs.dimension, rhs.dimension) else {
            return Err(ParseError::NotACalculation);
        };
        let dimension = left_dimension.adding(right_dimension, if dividing { -1.0 } else { 1.0 });
        let base = if dividing {
            left.amount * lhs.factor / (right.amount * rhs.factor)
        } else {
            left.amount * lhs.factor * right.amount * rhs.factor
        };
        if dimension == Dimension::scalar() {
            return Ok(Value::scalar(base));
        }
        let preferred = if dividing {
            None
        } else {
            measure::product_unit(&lhs, &rhs)
        };
        let unit = match preferred.or_else(|| measure::base_unit(&dimension)) {
            Some(unit) => unit,
            None => match measure::combine(&lhs, &rhs, dividing) {
                Some(compound) => compound,
                None => return Err(ParseError::NotACalculation),
            },
        };
        Ok(Value::measurement(base / unit.factor, unit))
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
                    if value.currency.is_none() && value.unit.is_none() {
                        if let Some(currency) = catalog.currency_definition(name) {
                            value.currency = Some(currency);
                            self.dimension_count += 1;
                            self.position += 1;
                            continue;
                        }
                    }
                    // `10kg` is ten of the unit the name stands for.
                    if value.currency.is_none() && value.unit.is_none() {
                        if let Some(unit) = Measurement::named(catalog, name) {
                            value.unit = Some(unit);
                            self.dimension_count += 1;
                            self.position += 1;
                            continue;
                        }
                    }
                    check_dimension(name, catalog)?;
                    return Ok(value);
                }
                Some(Token::Op(op)) if *op == Operator::Multiply || *op == Operator::Divide => {
                    // A unit beside another unit name makes a compound of the two:
                    // `60 miles per hour` is sixty miles per hour, not a division of
                    // two measures.
                    if !matches!(self.tokens.get(self.position + 1), Some(Token::Ident(_))) {
                        return Ok(value);
                    }
                    let Some(unit) = arithmetic_unit(value) else {
                        return Ok(value);
                    };
                    let Some((factor, end)) = measure::factor(self.tokens, self.position + 1)
                    else {
                        return Ok(value);
                    };
                    // A number after the unit means the unit stands alone.
                    if end != self.tokens.len() && number_value(&self.tokens[end]).is_some() {
                        return Ok(value);
                    }
                    let Some(combined) = measure::combine(&unit, &factor, *op == Operator::Divide)
                    else {
                        return Ok(value);
                    };
                    value.currency = None;
                    value.unit = Some(combined);
                    self.dimension_count += 1;
                    self.position = end;
                }
                Some(Token::Op(Operator::Percent)) => {
                    // A percentage is a plain amount: `50 as % of 200` is a phrase, and
                    // the `as` there is not the unit of a coulomb.
                    if value.is_percent
                        || value.is_boolean
                        || value.unit.is_some()
                        || value.currency.is_some()
                    {
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
                // The sign turns the amount about; the unit stays.
                Ok(Value {
                    amount: -value.effective(),
                    is_percent: false,
                    is_boolean: false,
                    currency: value.currency,
                    unit: value.unit,
                })
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
            return self.root(value, degree);
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
                return self.root(value, degree);
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
            // A function of a measure keeps the unit, or answers in plain numbers.
            if let Some(result) = self.measure_function(name, argument)? {
                return Ok(result);
            }
            let result = Math::apply_single(name, argument.effective())
                .ok_or(ParseError::NotACalculation)?;
            return Ok(Value::scalar(result));
        }
        // A unit right after a division means one of that unit: `100 / s` is a
        // hundred hertz, the way `(s)` is one second.
        if self.is_after_division() {
            if let Some(unit) = Measurement::named(catalog, name) {
                self.dimension_count += 1;
                self.position += 1;
                return Ok(Value::measurement(1.0, unit));
            }
        }
        // A currency before its amount: `$100`.
        if let Some(currency) = catalog.currency_definition(name) {
            if let Some(amount) = self.number_at(self.position + 1) {
                self.dimension_count += 1;
                self.position += 2;
                return Ok(Value::money(amount, currency));
            }
        }
        check_dimension(name, catalog)?;
        Err(ParseError::NotACalculation)
    }

    /// Whether the token before the cursor is a division, as upstream's
    /// `CalcExpressionParser.isAfterDivision`.
    fn is_after_division(&self) -> bool {
        self.position > 0 && self.tokens[self.position - 1] == Token::Op(Operator::Divide)
    }

    /// A function of a measure, as upstream's `CalcExpressionParser.parsePrefix` does.
    ///
    /// A root raises the unit as well, and the functions that keep a scale keep the
    /// unit; the trigonometry of an angle answers in plain numbers.
    fn measure_function(
        &mut self,
        name: &str,
        argument: Value,
    ) -> Result<Option<Value>, ParseError> {
        let Some(unit) = argument.unit else {
            return Ok(None);
        };
        Ok(match name {
            "sqrt" => Some(self.power(argument, Value::scalar(0.5))?),
            "cbrt" => Some(self.root(argument, 3.0)?),
            "abs" | "floor" | "ceil" | "round" | "trunc" => {
                let result =
                    Math::apply_single(name, argument.amount).ok_or(ParseError::NotACalculation)?;
                Some(Value::measurement(result, unit))
            }
            "sin" | "cos" | "tan" | "cot" | "sec" | "csc"
                if unit.category == UnitCategory::Angle =>
            {
                let result = Math::apply_single(name, argument.amount * unit.factor)
                    .ok_or(ParseError::NotACalculation)?;
                Some(Value::scalar(result))
            }
            _ => None,
        })
    }

    /// The root of a value, as upstream's `CalcExpressionParser.root`.
    ///
    /// Odd roots keep the sign, so the cube root of -8m³ is -2 m rather than no answer.
    fn root(&mut self, value: Value, degree: f64) -> Result<Value, ParseError> {
        if degree % 2.0 != 1.0 || value.effective() >= 0.0 {
            return self.power(value, Value::scalar(1.0 / degree));
        }
        let magnitude = Value {
            amount: -value.amount,
            is_percent: false,
            is_boolean: false,
            currency: value.currency,
            unit: value.unit,
        };
        let mut result = self.power(magnitude, Value::scalar(1.0 / degree))?;
        result.amount = -result.amount;
        Ok(result)
    }

    /// Parses a function with an argument list, as upstream's
    /// `CalcExpressionParser.parseFunction`.
    fn parse_function(&mut self, name: &str, catalog: &dyn Catalog) -> Result<Value, ParseError> {
        self.position += 2;
        let mut values: Vec<Value> = Vec::new();
        loop {
            let end = self.argument_end().ok_or(ParseError::NotACalculation)?;
            let value = self.parse_scope(end, catalog)?;
            if value.is_boolean {
                return Err(ParseError::NotACalculation);
            }
            values.push(value);
            self.position = end + 1;
            if self.tokens[end] == Token::Op(Operator::Close) {
                break;
            }
        }
        self.operation_count += 1;
        let first = values[0];
        // The functions of a measurement answer in the unit of their first argument,
        // so `min(1km,999m)` is `0.999 km` rather than `1`.
        let keeps_unit = Math::MEASUREMENTS.contains(&name);
        let mut amounts: Vec<f64> = Vec::with_capacity(values.len());
        for (index, value) in values.iter().enumerate() {
            if name == "round" && index == 1 {
                // The number of digits a rounding keeps is a plain number.
                if value.unit.is_some() || value.currency.is_some() {
                    return Err(ParseError::NotACalculation);
                }
                amounts.push(value.effective());
            } else if keeps_unit {
                amounts.push(self.comparable(*value, first)?);
            } else {
                if value.unit.is_some() || value.currency.is_some() {
                    return Err(ParseError::NotACalculation);
                }
                amounts.push(value.effective());
            }
        }
        let result = Math::evaluate(name, &amounts).ok_or(ParseError::NotACalculation)?;
        match (keeps_unit, first.unit) {
            (true, Some(unit)) => Ok(Value::measurement(result, unit)),
            _ => Ok(Value::scalar(result)),
        }
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

/// Reports a word the catalog knows as a unit or a currency.
///
/// Upstream resolves those names into a dimension and carries them through the
/// calculation; the units live in this crate now, so a unit the catalog cannot
/// define is reported as unsupported rather than answered with a made-up number. A
/// currency the catalog can define is carried; one it can only name is reported as
/// unsupported too.
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

/// The unit a value carries into an arithmetic operation, as upstream's
/// `CalcExpressionParser.arithmeticUnit`.
///
/// A currency is a unit of its own dimension, so money can be a compound with a
/// time: `50 usd per hour` is `50 USD/hr`.
fn arithmetic_unit(value: Value) -> Option<Measurement> {
    match (value.unit, value.currency) {
        (Some(unit), _) => Some(unit),
        (None, Some(currency)) => Some(measure::of_currency(currency)),
        (None, None) => None,
    }
}

/// The number a token carries, as upstream's `CalcQuantity.numberValue`.
fn number_value(token: &Token) -> Option<f64> {
    match token {
        Token::Number(value) | Token::CompactNumber(value) => Some(*value),
        Token::IntLiteral(value, _) => Some(*value as f64),
        _ => None,
    }
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
