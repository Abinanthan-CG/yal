//! The seam to the unit and currency catalogs.
//!
//! Upstream's calculator core reaches into `CalcUnits` and `CalcCurrency` for a handful
//! of decisions: whether a word names a unit, whether a word names a currency, and
//! which currency a sign such as `€` stands for. Those catalogs are not part of this
//! crate, so the core asks a [`Catalog`] instead.
//!
//! A catalog implementation is a real, working lookup - the crate ships [`NameCatalog`],
//! which serves any table of names - and it is what lets the core tell a unit apart from
//! an unknown word. A name the catalog knows is reported as unsupported rather than
//! guessed at.

use std::collections::BTreeMap;

/// What the calculator core needs to know about a unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitInfo {
    /// The spelling the catalog is indexed by, such as `kg`.
    pub id: String,
    /// The symbol the unit is written with, such as `kg`.
    pub symbol: String,
    /// The human-readable name, such as `Kilogram`.
    pub name: String,
    /// Whether the unit measures temperature.
    ///
    /// The core needs this to keep a spaced `k` a Kelvin and an attached `k` a thousands
    /// suffix, exactly as upstream's `CalcTokenizer.isTemperatureConversion` does.
    pub is_temperature: bool,
}

/// What the calculator core needs to know about a currency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrencyInfo {
    /// The ISO code, such as `USD`.
    pub code: String,
    /// The human-readable name, such as `US Dollar`.
    pub name: String,
}

/// The units and currencies the calculator core knows about.
///
/// Implementations answer a lookup; the core never assumes a name is unknown when the
/// catalog knows it, so that a unit or a currency is reported as unsupported instead of
/// being silently treated as a misspelled word.
pub trait Catalog {
    /// The unit filed under `name`, if the catalog knows it.
    fn unit(&self, name: &str) -> Option<UnitInfo>;

    /// The currency filed under `name` - its code or an alias - if the catalog knows it.
    fn currency(&self, name: &str) -> Option<CurrencyInfo>;

    /// The currency code a sign such as `€` stands for, if the catalog knows it.
    fn currency_sign(&self, sign: char) -> Option<String>;
}

/// A catalog served from plain tables of names.
///
/// This is the implementation the core is meant to be driven with: the units and
/// currencies crates fill it in, and every lookup it answers is a real one. An empty
/// catalog knows nothing, so every unit and currency query reads as an unknown word.
#[derive(Debug, Clone, Default)]
pub struct NameCatalog {
    units: BTreeMap<String, UnitInfo>,
    currencies: BTreeMap<String, CurrencyInfo>,
    signs: BTreeMap<char, String>,
}

impl NameCatalog {
    /// A catalog that knows no units and no currencies.
    pub fn new() -> Self {
        Self::default()
    }

    /// Files a unit under `id`, as upstream's unit catalog indexes its names.
    #[must_use]
    pub fn unit(mut self, id: &str, symbol: &str, name: &str) -> Self {
        self.units.insert(
            id.to_string(),
            UnitInfo {
                id: id.to_string(),
                symbol: symbol.to_string(),
                name: name.to_string(),
                is_temperature: false,
            },
        );
        self
    }

    /// Files a temperature unit under `id`.
    ///
    /// The core keeps a `k` a Kelvin when the rest of the query converts into a
    /// temperature, and that check needs the category.
    #[must_use]
    pub fn temperature_unit(mut self, id: &str, symbol: &str, name: &str) -> Self {
        self.units.insert(
            id.to_string(),
            UnitInfo {
                id: id.to_string(),
                symbol: symbol.to_string(),
                name: name.to_string(),
                is_temperature: true,
            },
        );
        self
    }

    /// Files a currency under its ISO `code`.
    #[must_use]
    pub fn currency(mut self, code: &str, name: &str) -> Self {
        self.currencies.insert(
            code.to_string(),
            CurrencyInfo {
                code: code.to_string(),
                name: name.to_string(),
            },
        );
        self
    }

    /// Files a currency under an `alias` that stands for the same currency.
    #[must_use]
    pub fn currency_alias(mut self, alias: &str, code: &str, name: &str) -> Self {
        self.currencies.insert(
            alias.to_string(),
            CurrencyInfo {
                code: code.to_string(),
                name: name.to_string(),
            },
        );
        self
    }

    /// Files the currency a sign stands for.
    #[must_use]
    pub fn currency_sign(mut self, sign: char, code: &str) -> Self {
        self.signs.insert(sign, code.to_string());
        self
    }
}

impl Catalog for NameCatalog {
    fn unit(&self, name: &str) -> Option<UnitInfo> {
        self.units.get(name).cloned()
    }

    fn currency(&self, name: &str) -> Option<CurrencyInfo> {
        self.currencies.get(name).cloned()
    }

    fn currency_sign(&self, sign: char) -> Option<String> {
        self.signs.get(&sign).cloned()
    }
}
