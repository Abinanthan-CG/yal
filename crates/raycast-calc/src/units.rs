//! Unit definitions and conversion.
//!
//! Units are grouped into dimensions. A conversion is only possible within a
//! dimension, so the table itself enforces the "incompatible units" rule
//! rather than leaving it to arithmetic on raw factors.

use std::collections::HashMap;

/// The physical dimension a unit belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    Length,
    Mass,
    Time,
    Temperature,
    Volume,
    Data,
    Speed,
    Area,
    Energy,
    Pressure,
}

impl Dimension {
    /// The base unit of this dimension.
    pub fn base(self) -> &'static str {
        match self {
            Dimension::Length => "m",
            Dimension::Mass => "kg",
            Dimension::Time => "s",
            Dimension::Temperature => "c",
            Dimension::Volume => "l",
            Dimension::Data => "byte",
            Dimension::Speed => "m/s",
            Dimension::Area => "m2",
            Dimension::Energy => "j",
            Dimension::Pressure => "pa",
        }
    }
}

/// A unit definition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitDef {
    /// The canonical identifier.
    pub id: &'static str,
    /// Alternative spellings accepted from the user.
    pub aliases: &'static [&'static str],
    /// The dimension this unit belongs to.
    pub dimension: Dimension,
    /// Multiplier that converts a value in this unit to the dimension's base
    /// unit. Ignored for temperature, which uses `offset` instead.
    pub to_base: f64,
    /// Additive offset applied during temperature conversion.
    pub offset: f64,
    /// Whether this unit measures a time *duration* rather than an instant.
    pub is_duration: bool,
}

const fn unit(
    id: &'static str,
    aliases: &'static [&'static str],
    dimension: Dimension,
    to_base: f64,
) -> UnitDef {
    UnitDef {
        id,
        aliases,
        dimension,
        to_base,
        offset: 0.0,
        is_duration: false,
    }
}

const fn temperature(
    id: &'static str,
    aliases: &'static [&'static str],
    to_base: f64,
    offset: f64,
) -> UnitDef {
    UnitDef {
        id,
        aliases,
        dimension: Dimension::Temperature,
        to_base,
        offset,
        is_duration: false,
    }
}

/// Every unit the calculator understands.
pub const UNITS: &[UnitDef] = &[
    // Length (base: metre)
    unit("m", &["meter", "meters", "metre", "metres"], Dimension::Length, 1.0),
    unit("km", &["kilometer", "kilometers", "kilometre", "kilometres"], Dimension::Length, 1000.0),
    unit("cm", &["centimeter", "centimeters"], Dimension::Length, 0.01),
    unit("mm", &["millimeter", "millimeters"], Dimension::Length, 0.001),
    unit("mi", &["mile", "miles"], Dimension::Length, 1609.344),
    unit("yd", &["yard", "yards"], Dimension::Length, 0.9144),
    unit("ft", &["foot", "feet"], Dimension::Length, 0.3048),
    unit("in", &["inch", "inches"], Dimension::Length, 0.0254),
    unit("nmi", &["nauticalmile"], Dimension::Length, 1852.0),
    // Mass (base: kilogram)
    unit("kg", &["kilogram", "kilograms"], Dimension::Mass, 1.0),
    unit("g", &["gram", "grams"], Dimension::Mass, 0.001),
    unit("mg", &["milligram", "milligrams"], Dimension::Mass, 0.000_001),
    unit("t", &["tonne", "tonnes", "metricton"], Dimension::Mass, 1000.0),
    unit("lb", &["pound", "pounds"], Dimension::Mass, 0.453_592_37),
    unit("oz", &["ounce", "ounces"], Dimension::Mass, 0.028_349_523_125),
    // Time (base: second)
    unit(
        "s",
        &["second", "seconds", "sec", "secs"],
        Dimension::Time,
        1.0,
    ),
    unit(
        "min",
        &["minute", "minutes", "mins"],
        Dimension::Time,
        60.0,
    ),
    unit(
        "h",
        &["hour", "hours", "hr", "hrs"],
        Dimension::Time,
        3600.0,
    ),
    unit(
        "d",
        &["day", "days"],
        Dimension::Time,
        86_400.0,
    ),
    unit(
        "wk",
        &["week", "weeks"],
        Dimension::Time,
        604_800.0,
    ),
    unit(
        "mo",
        &["month", "months"],
        Dimension::Time,
        2_629_800.0,
    ),
    unit(
        "yr",
        &["year", "years"],
        Dimension::Time,
        31_557_600.0,
    ),
    // Temperature (base: celsius)
    temperature("c", &["celsius", "centigrade", "degc"], 1.0, 0.0),
    temperature("f", &["fahrenheit", "degf"], 5.0 / 9.0, -32.0 * 5.0 / 9.0),
    temperature("k", &["kelvin", "degk"], 1.0, -273.15),
    // Volume (base: litre)
    unit("l", &["liter", "liters", "litre", "litres"], Dimension::Volume, 1.0),
    unit("ml", &["milliliter", "milliliters", "millilitre"], Dimension::Volume, 0.001),
    unit("gal", &["gallon", "gallons"], Dimension::Volume, 3.785_411_784),
    unit("floz", &["fluidounce", "fluidounces"], Dimension::Volume, 0.029_573_529_562_5),
    unit("pt", &["pint", "pints"], Dimension::Volume, 0.473_176_473),
    // Data (base: byte)
    unit("b", &["byte", "bytes"], Dimension::Data, 1.0),
    unit("kb", &["kilobyte", "kilobytes"], Dimension::Data, 1000.0),
    unit("mb", &["megabyte", "megabytes"], Dimension::Data, 1_000_000.0),
    unit("gb", &["gigabyte", "gigabytes"], Dimension::Data, 1_000_000_000.0),
    unit("tb", &["terabyte", "terabytes"], Dimension::Data, 1_000_000_000_000.0),
    unit("kib", &["kibibyte", "kibibytes"], Dimension::Data, 1024.0),
    unit("mib", &["mebibyte", "mebibytes"], Dimension::Data, 1_048_576.0),
    unit("gib", &["gibibyte", "gibibytes"], Dimension::Data, 1_073_741_824.0),
    // Speed (base: metre per second)
    unit("mps", &["meterspersecond"], Dimension::Speed, 1.0),
    unit("kph", &["kmh", "kilometerperhour"], Dimension::Speed, 1000.0 / 3600.0),
    unit("mph", &["mileperhour"], Dimension::Speed, 1609.344 / 3600.0),
    unit("knot", &["knots"], Dimension::Speed, 1852.0 / 3600.0),
    // Area (base: square metre)
    unit("m2", &["sqm", "squaremeter", "squaremeters"], Dimension::Area, 1.0),
    unit("cm2", &["sqcm"], Dimension::Area, 0.0001),
    unit("km2", &["sqkm"], Dimension::Area, 1_000_000.0),
    unit("ha", &["hectare", "hectares"], Dimension::Area, 10_000.0),
    unit("acre", &["acres"], Dimension::Area, 4_046.856_422_4),
    // Energy (base: joule)
    unit("j", &["joule", "joules"], Dimension::Energy, 1.0),
    unit("kj", &["kilojoule", "kilojoules"], Dimension::Energy, 1000.0),
    unit("cal", &["calorie", "calories"], Dimension::Energy, 4.184),
    unit("kcal", &["kilocalorie", "kilocalories"], Dimension::Energy, 4184.0),
    unit("wh", &["wattHour"], Dimension::Energy, 3600.0),
    unit("kwh", &["kilowattHour", "kilowatthour"], Dimension::Energy, 3_600_000.0),
    // Pressure (base: pascal)
    unit("pa", &["pascal", "pascals"], Dimension::Pressure, 1.0),
    unit("kpa", &["kilopascal"], Dimension::Pressure, 1000.0),
    unit("bar", &["bars"], Dimension::Pressure, 100_000.0),
    unit("atm", &["atmosphere", "atmospheres"], Dimension::Pressure, 101_325.0),
    unit(
        "psi",
        &["poundsperinch"],
        Dimension::Pressure,
        6_894.757_293_168,
    ),
];

/// A registry of units, resolved by canonical id or alias.
#[derive(Debug, Clone)]
pub struct UnitRegistry {
    by_id: HashMap<&'static str, &'static UnitDef>,
    by_alias: HashMap<String, &'static UnitDef>,
}

impl Default for UnitRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl UnitRegistry {
    /// Build the registry from the built-in table.
    pub fn new() -> Self {
        let mut by_id = HashMap::new();
        let mut by_alias = HashMap::new();
        for def in UNITS {
            by_id.insert(def.id, def);
            by_alias.insert(def.id.to_string(), def);
            for alias in def.aliases {
                by_alias.insert((*alias).to_string(), def);
            }
        }
        Self { by_id, by_alias }
    }

    /// Resolve a unit by canonical id or alias, case-insensitively.
    pub fn resolve(&self, name: &str) -> Option<&'static UnitDef> {
        let lowered = name.to_lowercase();
        self.by_alias.get(&lowered).copied()
    }

    /// Look up a unit by its canonical id.
    pub fn by_id(&self, id: &str) -> Option<&'static UnitDef> {
        self.by_id.get(id).copied()
    }
}

/// Convert an amount in `from` to the base unit of its dimension.
pub fn to_base(amount: f64, def: &UnitDef) -> f64 {
    match def.dimension {
        // Temperature base is celsius: value * factor + offset.
        Dimension::Temperature => amount * def.to_base + def.offset,
        _ => amount * def.to_base,
    }
}

/// Convert an amount from the base unit of its dimension into `def`.
pub fn from_base(base_amount: f64, def: &UnitDef) -> f64 {
    match def.dimension {
        Dimension::Temperature => (base_amount - def.offset) / def.to_base,
        _ => base_amount / def.to_base,
    }
}

/// Convert an amount between two units of the same dimension.
pub fn convert(amount: f64, from: &UnitDef, to: &UnitDef) -> Option<f64> {
    if from.dimension != to.dimension {
        return None;
    }
    Some(from_base(to_base(amount, from), to))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_aliases_case_insensitively() {
        let registry = UnitRegistry::new();
        assert_eq!(registry.resolve("KM").map(|u| u.id), Some("km"));
        assert_eq!(registry.resolve("miles").map(|u| u.id), Some("mi"));
        assert!(registry.resolve("nonsense").is_none());
    }

    #[test]
    fn converts_length() {
        let registry = UnitRegistry::new();
        let km = registry.resolve("km").unwrap();
        let mi = registry.resolve("mi").unwrap();
        let miles = convert(1.0, km, mi).unwrap();
        assert!((miles - 0.621_371_192_237_334).abs() < 1e-9);
    }

    #[test]
    fn converts_temperature_with_offset() {
        let registry = UnitRegistry::new();
        let c = registry.resolve("c").unwrap();
        let f = registry.resolve("f").unwrap();
        let fahrenheit = convert(100.0, c, f).unwrap();
        assert!((fahrenheit - 212.0).abs() < 1e-9);
        let back = convert(fahrenheit, f, c).unwrap();
        assert!((back - 100.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_cross_dimension_conversion() {
        let registry = UnitRegistry::new();
        let kg = registry.resolve("kg").unwrap();
        let km = registry.resolve("km").unwrap();
        assert!(convert(1.0, kg, km).is_none());
    }

    #[test]
    fn kelvin_converts_to_celsius() {
        let registry = UnitRegistry::new();
        let k = registry.resolve("k").unwrap();
        let c = registry.resolve("c").unwrap();
        let celsius = convert(273.15, k, c).unwrap();
        assert!((celsius - 0.0).abs() < 1e-9);
    }
}
