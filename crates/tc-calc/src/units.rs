//! The unit catalog and the conversions between units.
//!
//! Mirrors upstream `CalcUnits.swift`, `CalcUnitCatalog.swift` and `CalcDimension.swift`.
//!
//! A unit is an affine map onto its base: `base = value * factor + offset`. Only the
//! temperatures need the offset; everything else is a plain scale.
//!
//! The catalog is a table of names, so a word the table knows is a unit and a word it
//! does not know is not. That is what lets the tokenizer tell `10kg` (a measurement) from
//! `10kg` (a search) and `5k` (five thousand) from `5 K` (five kelvin).

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::catalog::{Catalog, CurrencyInfo, UnitInfo};
use crate::formatter::Formatter;
use crate::result::{CalcResult, Payload};
use crate::token::Token;

/// What a unit measures, as upstream's `UnitCategory`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitCategory {
    /// Length.
    Length,
    /// Weight.
    Weight,
    /// Temperature.
    Temperature,
    /// Time.
    Time,
    /// Area.
    Area,
    /// Volume.
    Volume,
    /// Digital storage.
    DigitalStorage,
    /// Angle.
    Angle,
    /// Speed.
    Speed,
    /// Pressure.
    Pressure,
    /// Data transfer rate.
    DataRate,
    /// Acceleration.
    Acceleration,
    /// Force.
    Force,
    /// Energy.
    Energy,
    /// Power.
    Power,
    /// Frequency.
    Frequency,
    /// Electric current.
    ElectricCurrent,
    /// Voltage.
    Voltage,
    /// Resistance.
    Resistance,
    /// Electric charge.
    ElectricCharge,
    /// Volume flow rate.
    VolumeFlow,
    /// Capacitance.
    Capacitance,
    /// Inductance.
    Inductance,
    /// Conductance.
    Conductance,
    /// Magnetic flux.
    MagneticFlux,
    /// Magnetic flux density.
    MagneticFluxDensity,
    /// Torque.
    Torque,
    /// Amount of substance.
    Substance,
    /// Fuel economy.
    FuelEconomy,
    /// Fuel consumption.
    FuelConsumption,
    /// Pixels.
    Pixels,
    /// Pixel area.
    PixelArea,
    /// Pixel density.
    PixelDensity,
    /// A unit made of other units.
    Compound,
}

impl UnitCategory {
    /// The name the card shows, as upstream's `UnitCategory.displayName`.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Length => "Length",
            Self::Weight => "Weight",
            Self::Temperature => "Temperature",
            Self::Time => "Time",
            Self::Area => "Area",
            Self::Volume => "Volume",
            Self::DigitalStorage => "Digital Storage",
            Self::Angle => "Angle",
            Self::Speed => "Speed",
            Self::Pressure => "Pressure",
            Self::DataRate => "Data Transfer Rate",
            Self::Acceleration => "Acceleration",
            Self::Force => "Force",
            Self::Energy => "Energy",
            Self::Power => "Power",
            Self::Frequency => "Frequency",
            Self::ElectricCurrent => "Electric Current",
            Self::Voltage => "Voltage",
            Self::Resistance => "Resistance",
            Self::ElectricCharge => "Electric Charge",
            Self::VolumeFlow => "Volume Flow Rate",
            Self::Capacitance => "Capacitance",
            Self::Inductance => "Inductance",
            Self::Conductance => "Conductance",
            Self::MagneticFlux => "Magnetic Flux",
            Self::MagneticFluxDensity => "Magnetic Flux Density",
            Self::Torque => "Torque",
            Self::Substance => "Amount of Substance",
            Self::FuelEconomy => "Fuel Economy",
            Self::FuelConsumption => "Fuel Consumption",
            Self::Pixels => "Pixels",
            Self::PixelArea => "Pixel Area",
            Self::PixelDensity => "Pixel Density",
            Self::Compound => "Compound Units",
        }
    }

    /// The dimension the category measures, as upstream's `UnitCategory.dimension`.
    ///
    /// The temperature, the angle and the compound units have none of their own.
    pub fn dimension(self) -> Option<Dimension> {
        Some(match self {
            Self::Length => Dimension::new(1.0, 0.0, 0.0),
            Self::Weight => Dimension::new(0.0, 1.0, 0.0),
            Self::Time => Dimension::new(0.0, 0.0, 1.0),
            Self::Area => Dimension::new(2.0, 0.0, 0.0),
            Self::Volume => Dimension::new(3.0, 0.0, 0.0),
            Self::DigitalStorage => Dimension::data(1.0),
            Self::Speed => Dimension::new(1.0, 0.0, -1.0),
            Self::Pressure => Dimension::new(-1.0, 1.0, -2.0),
            Self::DataRate => Dimension::data(1.0).with_time(-1.0),
            Self::Acceleration => Dimension::new(1.0, 0.0, -2.0),
            Self::Force => Dimension::new(1.0, 1.0, -2.0),
            Self::Energy => Dimension::new(2.0, 1.0, -2.0),
            Self::Power => Dimension::new(2.0, 1.0, -3.0),
            Self::Frequency => Dimension::new(0.0, 0.0, -1.0),
            Self::ElectricCurrent => Dimension::current(1.0),
            Self::Voltage => Dimension::new(2.0, 1.0, -3.0).with_current(-1.0),
            Self::Resistance => Dimension::new(2.0, 1.0, -3.0).with_current(-2.0),
            Self::ElectricCharge => Dimension::new(0.0, 0.0, 1.0).with_current(1.0),
            Self::VolumeFlow => Dimension::new(3.0, 0.0, -1.0),
            Self::Capacitance => Dimension::new(-2.0, -1.0, 4.0).with_current(2.0),
            Self::Inductance => Dimension::new(2.0, 1.0, -2.0).with_current(-2.0),
            Self::Conductance => Dimension::new(-2.0, 0.0, 3.0).with_current(2.0),
            Self::MagneticFlux => Dimension::new(2.0, 1.0, -2.0).with_current(-1.0),
            Self::MagneticFluxDensity => Dimension::new(0.0, 1.0, -2.0).with_current(-1.0),
            Self::Torque => Dimension::new(2.0, 1.0, -2.0),
            Self::Substance => Dimension::substance(1.0),
            Self::FuelEconomy => Dimension::new(-2.0, 0.0, 0.0),
            Self::FuelConsumption => Dimension::new(2.0, 0.0, 0.0),
            Self::Pixels => Dimension::pixels(1.0),
            Self::PixelArea => Dimension::pixels(2.0),
            Self::PixelDensity => Dimension::new(-1.0, 0.0, 0.0).with_pixels(1.0),
            Self::Temperature | Self::Angle | Self::Compound => return None,
        })
    }

    /// The key the catalog's records use for this category, as upstream's
    /// `UnitCategory(rawValue:)`.
    fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "length" => Self::Length,
            "weight" => Self::Weight,
            "temperature" => Self::Temperature,
            "time" => Self::Time,
            "area" => Self::Area,
            "volume" => Self::Volume,
            "digitalStorage" => Self::DigitalStorage,
            "angle" => Self::Angle,
            "speed" => Self::Speed,
            "pressure" => Self::Pressure,
            "dataRate" => Self::DataRate,
            "acceleration" => Self::Acceleration,
            "force" => Self::Force,
            "energy" => Self::Energy,
            "power" => Self::Power,
            "frequency" => Self::Frequency,
            "electricCurrent" => Self::ElectricCurrent,
            "voltage" => Self::Voltage,
            "resistance" => Self::Resistance,
            "electricCharge" => Self::ElectricCharge,
            "volumeFlow" => Self::VolumeFlow,
            "capacitance" => Self::Capacitance,
            "inductance" => Self::Inductance,
            "conductance" => Self::Conductance,
            "magneticFlux" => Self::MagneticFlux,
            "magneticFluxDensity" => Self::MagneticFluxDensity,
            "torque" => Self::Torque,
            "substance" => Self::Substance,
            "fuelEconomy" => Self::FuelEconomy,
            "fuelConsumption" => Self::FuelConsumption,
            "pixels" => Self::Pixels,
            "pixelArea" => Self::PixelArea,
            "pixelDensity" => Self::PixelDensity,
            "compound" => Self::Compound,
            _ => return None,
        })
    }
}

/// What a quantity is measured in, as upstream's `CalcDimension`.
///
/// The exponents say how much of each base quantity a unit carries: a metre is one
/// length, a square metre is two, a kilometre per hour is one length and one time
/// backwards.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dimension {
    /// The exponent of the length.
    pub length: f64,
    /// The exponent of the mass.
    pub mass: f64,
    /// The exponent of the time.
    pub time: f64,
    /// The exponent of the data size.
    pub data: f64,
    /// The exponent of the electric current.
    pub electric_current: f64,
    /// The exponent of the pixels.
    pub pixels: f64,
    /// The exponent of the currency.
    pub currency: f64,
    /// The exponent of the substance.
    pub substance: f64,
}

impl Dimension {
    /// A dimension of length, mass and time.
    pub const fn new(length: f64, mass: f64, time: f64) -> Self {
        Self {
            length,
            mass,
            time,
            data: 0.0,
            electric_current: 0.0,
            pixels: 0.0,
            currency: 0.0,
            substance: 0.0,
        }
    }

    /// A dimension of data size alone.
    const fn data(data: f64) -> Self {
        Self {
            data,
            ..Self::new(0.0, 0.0, 0.0)
        }
    }

    /// A dimension of electric current alone.
    const fn current(electric_current: f64) -> Self {
        Self {
            electric_current,
            ..Self::new(0.0, 0.0, 0.0)
        }
    }

    /// A dimension of pixels alone.
    const fn pixels(pixels: f64) -> Self {
        Self {
            pixels,
            ..Self::new(0.0, 0.0, 0.0)
        }
    }

    /// A dimension of substance alone.
    const fn substance(substance: f64) -> Self {
        Self {
            substance,
            ..Self::new(0.0, 0.0, 0.0)
        }
    }

    /// Adds the time exponent, as upstream's `CalcDimension` does for a rate.
    #[must_use]
    const fn with_time(mut self, time: f64) -> Self {
        self.time += time;
        self
    }

    /// Adds the electric current exponent.
    #[must_use]
    const fn with_current(mut self, current: f64) -> Self {
        self.electric_current += current;
        self
    }

    /// Adds the pixels exponent.
    #[must_use]
    const fn with_pixels(mut self, pixels: f64) -> Self {
        self.pixels += pixels;
        self
    }

    /// The dimension of nothing, as upstream's `CalcDimension.scalar`.
    pub const fn scalar() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// Adds a dimension, as upstream's `CalcDimension.adding`.
    #[must_use]
    pub fn adding(self, other: Self, scale: f64) -> Self {
        Self {
            length: self.length + other.length * scale,
            mass: self.mass + other.mass * scale,
            time: self.time + other.time * scale,
            data: self.data + other.data * scale,
            electric_current: self.electric_current + other.electric_current * scale,
            pixels: self.pixels + other.pixels * scale,
            currency: self.currency + other.currency * scale,
            substance: self.substance + other.substance * scale,
        }
    }

    /// Raises a dimension to a power, as upstream's `CalcDimension.raised`.
    #[must_use]
    pub fn raised(self, power: f64) -> Self {
        Self {
            length: self.length * power,
            mass: self.mass * power,
            time: self.time * power,
            data: self.data * power,
            electric_current: self.electric_current * power,
            pixels: self.pixels * power,
            currency: self.currency * power,
            substance: self.substance * power,
        }
    }
}

/// A unit, as upstream's `UnitDef`.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitDef {
    /// The canonical display form, such as `mi` or `°F`.
    pub symbol: String,
    /// The long label for the card's badge, such as `Miles`.
    pub name: String,
    /// What the unit measures.
    pub category: UnitCategory,
    /// The scale onto the base unit.
    pub factor: f64,
    /// The offset onto the base unit; only the temperatures need one.
    pub offset: f64,
    /// The dimension of a unit that is not its category's own.
    pub derived_dimension: Option<Dimension>,
}

impl UnitDef {
    /// The dimension the unit measures.
    pub fn dimension(&self) -> Option<Dimension> {
        self.derived_dimension.or_else(|| self.category.dimension())
    }

    /// Whether the unit can be converted into the other one, as upstream's
    /// `UnitDef.isCompatible`.
    pub fn is_compatible_with(&self, other: &UnitDef) -> bool {
        if self.category != UnitCategory::Compound && self.category == other.category {
            return true;
        }
        match (self.dimension(), other.dimension()) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
    }
}

/// The unit table, as upstream's `CalcUnits.byName`.
fn table() -> &'static HashMap<String, UnitDef> {
    static TABLE: OnceLock<HashMap<String, UnitDef>> = OnceLock::new();
    TABLE.get_or_init(build_table)
}

/// The unit filed under `name`, if the table knows it.
pub fn by_name(name: &str) -> Option<&'static UnitDef> {
    table().get(name)
}

/// Whether the table knows the name, in any spelling.
pub fn knows(name: &str) -> bool {
    by_name(name).is_some()
}

/// Builds the table, as upstream's `CalcUnitCatalog.makeIndex`.
fn build_table() -> HashMap<String, UnitDef> {
    let mut table: HashMap<String, UnitDef> = HashMap::with_capacity(1024);
    let mut category = UnitCategory::Length;
    for line in RECORDS.lines() {
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() == 1 {
            category = UnitCategory::from_key(fields[0]).unwrap_or(category);
            continue;
        }
        if fields.len() != 4 && fields.len() != 5 {
            continue;
        }
        let Ok(factor) = fields[2].parse::<f64>() else {
            continue;
        };
        let offset = if fields.len() == 5 {
            fields[4].parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };
        let definition = UnitDef {
            symbol: fields[0].to_string(),
            name: fields[1].to_string(),
            category,
            factor,
            offset,
            derived_dimension: None,
        };
        for name in fields[3].split(',') {
            table.insert(name.to_string(), definition.clone());
        }
    }

    // The metric prefixes: a kilometre is a thousand metres, and so on.
    for (prefix, name, factor) in [
        ("p", "Pico", 1e-12),
        ("n", "Nano", 1e-9),
        ("µ", "Micro", 1e-6),
        ("m", "Milli", 1e-3),
        ("c", "Centi", 1e-2),
        ("d", "Deci", 1e-1),
        ("k", "Kilo", 1e3),
        ("M", "Mega", 1e6),
        ("G", "Giga", 1e9),
        ("T", "Tera", 1e12),
        ("P", "Peta", 1e15),
    ] {
        for key in ["m", "g", "s", "hz", "n", "j", "w", "pa"] {
            let Some(base) = table.get(key) else {
                continue;
            };
            let symbol = format!("{prefix}{}", base.symbol);
            let label = format!("{name}{}", base.name.to_lowercase());
            let unit = UnitDef {
                symbol: symbol.clone(),
                name: label.clone(),
                category: base.category,
                factor: base.factor * factor,
                offset: 0.0,
                derived_dimension: None,
            };
            let noun = label.to_lowercase();
            let mut aliases = vec![symbol, noun.clone()];
            if noun.ends_with('s') {
                aliases.push(noun[..noun.len() - 1].to_string());
            }
            if prefix == "µ" {
                aliases.push(format!("u{}", base.symbol));
                aliases.push(format!("μ{}", base.symbol));
            }
            for alias in aliases {
                table.entry(alias).or_insert_with(|| unit.clone());
            }
        }
    }

    // The byte rates, and the bits that share their prefix.
    for (prefix, label, factor) in [
        ("", "", 1.0),
        ("k", "Kilo", 1e3),
        ("M", "Mega", 1e6),
        ("G", "Giga", 1e9),
        ("T", "Tera", 1e12),
        ("Ki", "Kibi", 1024.0),
        ("Mi", "Mebi", 1_048_576.0),
        ("Gi", "Gibi", 1_073_741_824.0),
        ("Ti", "Tebi", 1_099_511_627_776.0),
    ] {
        let bytes = format!("{prefix}B/s");
        table.insert(
            bytes.clone(),
            UnitDef {
                symbol: bytes.clone(),
                name: format!(
                    "{} per Second",
                    if label.is_empty() {
                        "Bytes".to_string()
                    } else {
                        format!("{label}bytes")
                    }
                ),
                category: UnitCategory::DataRate,
                factor,
                offset: 0.0,
                derived_dimension: None,
            },
        );
        if !prefix.is_empty() {
            let bits = format!("{prefix}bit");
            table.insert(
                bits.clone(),
                UnitDef {
                    symbol: bits.clone(),
                    name: format!("{label}bits"),
                    category: UnitCategory::DigitalStorage,
                    factor: factor / 8.0,
                    offset: 0.0,
                    derived_dimension: None,
                },
            );
            let rate = format!("{bits}/s");
            table.insert(
                rate.clone(),
                UnitDef {
                    symbol: rate,
                    name: format!("{label}bits per Second"),
                    category: UnitCategory::DataRate,
                    factor: factor / 8.0,
                    offset: 0.0,
                    derived_dimension: None,
                },
            );
        }
    }

    table
}

/// The counterpart a unit is converted into when no target is named, as upstream's
/// `CalcUnits.autoTargets`.
///
/// The second field says whether the answer is written as feet and inches.
fn auto_target(symbol: &str) -> Option<(&'static str, bool)> {
    Some(match symbol {
        "mm" => ("in", false),
        "cm" => ("in", false),
        "m" => ("ft", true),
        "km" => ("mi", false),
        "dm" => ("cm", false),
        "in" => ("cm", false),
        "ft" => ("m", false),
        "yd" => ("m", false),
        "mi" => ("km", false),
        "mg" => ("g", false),
        "g" => ("oz", false),
        "kg" => ("lb", false),
        "oz" => ("g", false),
        "lb" => ("kg", false),
        "°C" => ("f", false),
        "°F" => ("c", false),
        "K" => ("c", false),
        "ms" => ("s", false),
        "s" => ("ms", false),
        "min" => ("s", false),
        "hr" => ("min", false),
        "day" => ("hr", false),
        "week" => ("day", false),
        "mo" => ("day", false),
        "yr" => ("day", false),
        "workdays" => ("hr", false),
        "mm²" => ("in2", false),
        "cm²" => ("in2", false),
        "m²" => ("ft2", false),
        "km²" => ("mi2", false),
        "in²" => ("cm2", false),
        "ft²" => ("m2", false),
        "yd²" => ("m2", false),
        "mi²" => ("km2", false),
        "acre" => ("m2", false),
        "ha" => ("acre", false),
        "dm²" => ("cm2", false),
        "mL" => ("floz", false),
        "L" => ("gal", false),
        "cup" => ("ml", false),
        "tbsp" => ("ml", false),
        "tsp" => ("ml", false),
        "gal" => ("l", false),
        "qt" => ("l", false),
        "pt" => ("ml", false),
        "fl oz" => ("ml", false),
        "cL" => ("ml", false),
        "dL" => ("ml", false),
        "mm³" => ("ml", false),
        "cm³" => ("ml", false),
        "dm³" => ("l", false),
        "m³" => ("l", false),
        "in³" => ("ml", false),
        "ft³" => ("l", false),
        "yd³" => ("l", false),
        "L/s" => ("l/min", false),
        "L/min" => ("l/h", false),
        "L/h" => ("l/min", false),
        "m³/s" => ("l/s", false),
        "m³/h" => ("l/min", false),
        "gal/min" => ("l/min", false),
        "mpg" => ("km/l", false),
        "km/L" => ("mpg", false),
        "bit" => ("b", false),
        "B" => ("bit", false),
        "kB" => ("kib", false),
        "MB" => ("mib", false),
        "GB" => ("gib", false),
        "TB" => ("tib", false),
        "PB" => ("tb", false),
        "KiB" => ("kb", false),
        "MiB" => ("mb", false),
        "GiB" => ("gb", false),
        "TiB" => ("tb", false),
        "deg" => ("rad", false),
        "rad" => ("deg", false),
        "grad" => ("deg", false),
        "turn" => ("deg", false),
        "arcmin" => ("deg", false),
        "arcsec" => ("deg", false),
        "km/h" => ("mph", false),
        "mph" => ("kmh", false),
        "m/s" => ("kmh", false),
        "kn" => ("kmh", false),
        "ft/s" => ("mph", false),
        "bar" => ("psi", false),
        "psi" => ("bar", false),
        "atm" => ("psi", false),
        "mbar" => ("psi", false),
        "kPa" => ("psi", false),
        "hPa" => ("psi", false),
        "mmHg" => ("psi", false),
        "Torr" => ("psi", false),
        "Mbps" => ("kbps", false),
        "Gbps" => ("mbps", false),
        "Kbps" => ("bps", false),
        "bps" => ("kbps", false),
        "Tbps" => ("gbps", false),
        "Bps" => ("bps", false),
        "kBps" => ("kbps", false),
        "MBps" => ("mbps", false),
        "GBps" => ("gbps", false),
        "TBps" => ("tbps", false),
        "Wh" => ("kwh", false),
        "mWh" => ("wh", false),
        "kWh" => ("wh", false),
        "MWh" => ("kwh", false),
        "W" => ("kw", false),
        "mW" => ("w", false),
        "kW" => ("w", false),
        "MW" => ("kw", false),
        "A" => ("ma", false),
        "mA" => ("a", false),
        "µA" => ("ma", false),
        "MA" => ("a", false),
        "V" => ("mv", false),
        "mV" => ("v", false),
        "kV" => ("v", false),
        "MV" => ("kv", false),
        "Ω" => ("kohm", false),
        "mΩ" => ("ohm", false),
        "kΩ" => ("ohm", false),
        "MΩ" => ("kohm", false),
        "As" => ("ah", false),
        "Ah" => ("mah", false),
        "mAh" => ("ah", false),
        "MAh" => ("ah", false),
        "px" => ("rem", false),
        "rem" => ("px", false),
        "em" => ("px", false),
        "ppi" => ("px/cm", false),
        "px/cm" => ("ppi", false),
        "px/mm" => ("ppi", false),
        "px/m" => ("ppi", false),
        _ => return None,
    })
}

/// Whether a token connects a value to a target, as upstream's `CalcUnits.isConnector`.
pub fn is_connector(token: &Token) -> bool {
    match token {
        Token::Arrow => true,
        Token::Ident(name) => matches!(name.as_str(), "to" | "in" | "as" | "into"),
        _ => false,
    }
}

/// The speed dimension, as upstream's `CalcUnits.speed`.
fn speed() -> Dimension {
    Dimension::new(1.0, 0.0, -1.0)
}

/// The pace dimension, as upstream's `CalcUnits.pace`.
fn pace() -> Dimension {
    Dimension::new(-1.0, 0.0, 1.0)
}

/// Whether the two units are each other's reciprocal, as upstream's
/// `CalcUnits.reciprocal`.
///
/// A pace is a speed backwards, and the fuel figures are each other backwards.
pub fn reciprocal(from: &UnitDef, to: &UnitDef) -> bool {
    let (Some(left), Some(right)) = (from.dimension(), to.dimension()) else {
        return false;
    };
    if is_fuel(to) && is_fuel(from) {
        return left.adding(right, 1.0) == Dimension::scalar();
    }
    (left == speed() && right == pace()) || (left == pace() && right == speed())
}

/// Whether a unit measures fuel, as upstream's `CalcUnits.isFuel`.
fn is_fuel(unit: &UnitDef) -> bool {
    unit.category == UnitCategory::FuelEconomy || unit.category == UnitCategory::FuelConsumption
}

/// The answer to a reciprocal conversion, as upstream's `CalcUnits.reciprocalValue`.
pub fn reciprocal_value(amount: f64, from: &UnitDef, to: &UnitDef) -> Option<f64> {
    let base = amount * from.factor;
    if base == 0.0 {
        return None;
    }
    let output = 1.0 / base / to.factor;
    output.is_finite().then_some(output)
}

/// The answer to a conversion between two units of the same kind, as upstream's
/// `CalcQuantity.convertUnit`.
pub fn convert(amount: f64, from: &UnitDef, to: &UnitDef) -> f64 {
    (amount * from.factor + from.offset - to.offset) / to.factor
}

/// What a conversion produced, as upstream's `CalcUnits.ConversionParse`.
#[derive(Debug, Clone, PartialEq)]
pub enum ConversionParse {
    /// The conversion worked.
    Value {
        /// The amount that was typed.
        input: f64,
        /// The unit it was in.
        from: UnitDef,
        /// The unit it was converted into.
        to: UnitDef,
        /// The answer.
        output: f64,
    },
    /// The two units do not measure the same thing.
    Mismatch {
        /// The unit the amount was in.
        from: UnitDef,
        /// The unit it was asked for.
        to: UnitDef,
    },
}

/// The conversion of `expr unit (to|in|->) unit`, as upstream's
/// `CalcUnits.parseConversion`.
///
/// Matching the last position lets `in` double as inches.
pub fn parse_conversion(tokens: &[Token], catalog: &dyn Catalog) -> Option<ConversionParse> {
    if tokens.len() < 3 || !is_connector(&tokens[tokens.len() - 2]) {
        return None;
    }
    let Token::Ident(to_name) = &tokens[tokens.len() - 1] else {
        return None;
    };
    let to = catalog.unit_definition(to_name)?;
    let Token::Ident(from_name) = &tokens[tokens.len() - 3] else {
        return None;
    };
    let from = catalog.unit_definition(from_name)?;

    let value_tokens = &tokens[..tokens.len() - 3];
    let input = if value_tokens.is_empty() {
        1.0
    } else {
        scalar(value_tokens)?
    };

    if from.category != to.category && reciprocal(&from, &to) {
        let output = reciprocal_value(input, &from, &to)?;
        return Some(ConversionParse::Value {
            input,
            from,
            to,
            output,
        });
    }
    if from.category != to.category {
        return Some(ConversionParse::Mismatch { from, to });
    }
    let output = convert(input, &from, &to);
    if !output.is_finite() {
        return None;
    }
    Some(ConversionParse::Value {
        input,
        from,
        to,
        output,
    })
}

/// The conversion of two unit names, as upstream's `CalcUnits.parseUnitPairConversion`.
///
/// `day s` is one day in seconds. The units must be of the same kind, so that a
/// two-word search does not produce a card.
pub fn parse_unit_pair_conversion(
    tokens: &[Token],
    catalog: &dyn Catalog,
) -> Option<ConversionParse> {
    if tokens.len() != 2 {
        return None;
    }
    let Token::Ident(from_name) = &tokens[0] else {
        return None;
    };
    let from = catalog.unit_definition(from_name)?;
    let Token::Ident(to_name) = &tokens[1] else {
        return None;
    };
    let to = catalog.unit_definition(to_name)?;
    if from.category != to.category {
        return None;
    }
    let output = convert(1.0, &from, &to);
    if !output.is_finite() {
        return None;
    }
    Some(ConversionParse::Value {
        input: 1.0,
        from,
        to,
        output,
    })
}

/// The conversion of `expr unit` with no connector, as upstream's
/// `CalcUnits.parseBareConversion`.
///
/// The `c`, `f` and `k` are left out, so `5k` stays a search.
#[derive(Debug, Clone, PartialEq)]
pub struct BareConversion {
    /// The amount that was typed.
    pub input: f64,
    /// The unit it was in.
    pub from: UnitDef,
    /// The unit it was converted into.
    pub to: UnitDef,
    /// The answer.
    pub output: f64,
    /// Whether the answer is written as feet and inches.
    pub compound: bool,
}

/// The conversion of `expr unit` with no connector, as upstream's
/// `CalcUnits.parseBareConversion`.
pub fn parse_bare_conversion(tokens: &[Token], catalog: &dyn Catalog) -> Option<BareConversion> {
    if tokens.len() < 2 {
        return None;
    }
    let Token::Ident(from_name) = &tokens[tokens.len() - 1] else {
        return None;
    };
    if matches!(from_name.as_str(), "c" | "f" | "k") {
        return None;
    }
    let from = catalog.unit_definition(from_name)?;
    let (target, compound) = auto_target(&from.symbol)?;
    let to = catalog.unit_definition(target)?;
    let value_tokens = &tokens[..tokens.len() - 1];
    let input = scalar(value_tokens)?;
    let output = convert(input, &from, &to);
    if !output.is_finite() {
        return None;
    }
    Some(BareConversion {
        input,
        from,
        to,
        output,
        compound,
    })
}

/// The value of the tokens, when they are a plain amount.
///
/// Upstream's `CalcExpressionParser.scalar` reads any expression, but upstream's engine
/// reaches the conversions only after its own quantity parser has declined the query.
/// This port has no quantity parser yet, so the conversion answers only when the amount
/// is a single number: `1 / 20ms to hz` means one divided by twenty milliseconds, which
/// is fifty hertz, and reading it as a plain amount would answer `0.00005 Hz` instead.
///
/// A query that is not answered here is not answered wrongly; it is left to the caller.
fn scalar(tokens: &[Token]) -> Option<f64> {
    match tokens {
        [Token::Number(value)] | [Token::CompactNumber(value)] => Some(*value),
        [Token::IntLiteral(value, _)] => Some(*value as f64),
        _ => None,
    }
}

/// The card of a conversion, as upstream's `CalcEngine` shows it.
fn conversion_card(parse: ConversionParse, query: &str) -> CalcResult {
    match parse {
        ConversionParse::Value {
            input,
            from,
            to,
            output,
        } => CalcResult::with_badges(
            format!("{} {}", Formatter::display(input), from.symbol),
            &from.name,
            &to.name,
            Payload::measurement(output, &to),
        ),
        ConversionParse::Mismatch { from, to } => CalcResult::new(
            query.to_string(),
            Payload::Error {
                message: format!(
                    "Cannot convert {} to {}.",
                    from.category.display_name(),
                    to.category.display_name()
                ),
            },
        ),
    }
}

/// The card of a bare conversion, as upstream's `CalcEngine` shows it.
fn bare_card(conversion: BareConversion) -> CalcResult {
    let payload = if conversion.compound {
        let text = Formatter::compound_feet_inches(conversion.output);
        Payload::Value {
            display: text.clone(),
            copy_text: text,
        }
    } else {
        Payload::measurement(conversion.output, &conversion.to)
    };
    CalcResult::with_badges(
        format!(
            "{} {}",
            Formatter::display(conversion.input),
            conversion.from.symbol
        ),
        &conversion.from.name,
        &conversion.to.name,
        payload,
    )
}

/// The name the target tokens spell, as upstream's `CalcQuantity.targetName`.
fn target_name(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|token| match token {
            Token::Ident(name) => name.clone(),
            Token::Op(op) => op.glyph().to_string(),
            Token::Number(value) | Token::CompactNumber(value) => Formatter::copy_text(*value),
            Token::IntLiteral(value, _) => value.to_string(),
            Token::Arrow => "->".to_string(),
            Token::Comma => ",".to_string(),
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// The conversion into two units of one kind, as upstream's `CalcQuantity` answers it.
///
/// `1.75 m to feet and inches` is one amount told in feet and inches.
pub fn parse_mixed_conversion(tokens: &[Token], catalog: &dyn Catalog) -> Option<CalcResult> {
    for index in 0..tokens.len() {
        if !is_connector(&tokens[index]) {
            continue;
        }
        let target = &tokens[index + 1..];
        if target.is_empty() {
            continue;
        }
        let Some(pair) = crate::mixed_units::pair(catalog, &target_name(target)) else {
            continue;
        };
        let source = &tokens[..index];
        if source.len() < 2 {
            continue;
        }
        let amount = match &source[source.len() - 2] {
            Token::Number(value) | Token::CompactNumber(value) => *value,
            _ => continue,
        };
        let Token::Ident(from_name) = &source[source.len() - 1] else {
            continue;
        };
        let Some(from) = catalog.unit_definition(from_name) else {
            continue;
        };
        // The two units of the answer must measure what the amount measures.
        if from.category != pair.0.category {
            continue;
        }
        let Some(text) = crate::mixed_units::format(amount, &from, &pair) else {
            continue;
        };
        return Some(CalcResult::with_badges(
            format!("{} {}", Formatter::display(amount), from.symbol),
            &from.name,
            &format!("{} and {}", pair.0.name, pair.1.name),
            Payload::Value {
                display: text.clone(),
                copy_text: text,
            },
        ));
    }
    None
}

/// The card of a unit conversion, as upstream's `CalcEngine` shows it.
///
/// Upstream runs the conversions after the typed arithmetic, so `10 pounds to kg`
/// stays a weight.
pub fn evaluate(tokens: &[Token], query: &str, catalog: &dyn Catalog) -> Option<CalcResult> {
    if let Some(result) = parse_mixed_conversion(tokens, catalog) {
        return Some(result);
    }
    if let Some(parse) = parse_conversion(tokens, catalog) {
        return Some(conversion_card(parse, query));
    }
    if let Some(parse) = parse_unit_pair_conversion(tokens, catalog) {
        return Some(conversion_card(parse, query));
    }
    parse_bare_conversion(tokens, catalog).map(bare_card)
}

/// The catalog of the calculator, with the units in it.
///
/// The currencies are a separate slice; this catalog knows the units and no money.
#[derive(Debug, Clone, Default)]
pub struct UnitCatalog;

impl UnitCatalog {
    /// A catalog with every unit in it.
    pub fn new() -> Self {
        Self
    }
}

impl Catalog for UnitCatalog {
    fn unit(&self, name: &str) -> Option<UnitInfo> {
        by_name(name).map(|unit| UnitInfo {
            id: name.to_string(),
            symbol: unit.symbol.clone(),
            name: unit.name.clone(),
            is_temperature: unit.category == UnitCategory::Temperature,
        })
    }

    fn currency(&self, _name: &str) -> Option<CurrencyInfo> {
        None
    }

    fn currency_sign(&self, _sign: char) -> Option<String> {
        None
    }

    fn unit_definition(&self, name: &str) -> Option<UnitDef> {
        by_name(name).cloned()
    }

    fn unit_ref(&self, name: &str) -> Option<&'static UnitDef> {
        by_name(name)
    }
}

/// The catalog's records: the symbol, the label, the scale, the aliases and, for the
/// temperatures, the offset.
///
/// A line with one field is a category header; the records that follow belong to it.
const RECORDS: &str = "\
length
mm|Millimeters|0.001|mm,millimeter,millimeters,millimetre,millimetres
cm|Centimeters|0.01|cm,centimeter,centimeters,centimetre,centimetres
dm|Decimeters|0.1|dm,decimeter,decimeters,decimetre,decimetres
m|Meters|1.0|m,meter,meters,metre,metres
km|Kilometers|1000.0|km,kilometer,kilometers,kilometre,kilometres
in|Inches|0.0254|in,inch,inches
ft|Feet|0.3048|ft,foot,feet
yd|Yards|0.9144|yd,yard,yards
mi|Miles|1609.344|mi,mile,miles
nmi|Nautical Miles|1852.0|nmi,nauticalmile,nauticalmiles,nautical mile,nautical miles
ly|Light Years|9460730472580800.0|ly,lightyear,lightyears,light year,light years
pc|Parsecs|3.0856775814913673e16|pc,parsec,parsecs
au|Astronomical Units|149597870700.0|au,astronomical unit,astronomical units
fathom|Fathoms|1.8288|fathom,fathoms
furlong|Furlongs|201.168|furlong,furlongs
pixels
px|Pixels|1.0|px,pixel,pixels
rem|REM|16.0|rem,rems
em|EM|16.0|em,ems
pixelArea
px²|Square Pixels|1.0|px2
pixelDensity
ppi|Pixels per Inch|39.37007874015748|ppi,px/in,px/inch,px/inches,pixels/inch
px/cm|Pixels per Centimeter|100.0|px/cm
px/mm|Pixels per Millimeter|1000.0|px/mm
px/m|Pixels per Meter|1.0|px/m
weight
mg|Milligrams|1e-06|mg,milligram,milligrams
g|Grams|0.001|g,gram,grams
kg|Kilograms|1.0|kg,kilogram,kilograms,kilo,kilos
oz|Ounces|0.028349523125|oz,ounce,ounces
lb|Pounds|0.45359237|lb,lbs,pound,pounds
t|Tonnes|1000.0|t,ton,tons,tonne,tonnes
ct|Carats|0.0002|ct,carat,carats
ozt|Troy Ounces|0.0311034768|ozt,troy ounce,troy ounces,troy oz
st|Stone|6.35029318|st,stone
short ton|US Tons|907.18474|shortton,uston
long ton|UK Tons|1016.0469088|longton,ukton
temperature
°C|Celsius|1.0|c,°c,celsius,centigrade|273.15
°F|Fahrenheit|0.5555555555555556|f,°f,fahrenheit|255.3722222222222
K|Kelvin|1.0|k,kelvin,kelvins
time
ms|Milliseconds|0.001|ms,millisecond,milliseconds
s|Seconds|1.0|s,sec,secs,second,seconds
min|Minutes|60.0|min,mins,minute,minutes
hr|Hours|3600.0|h,hr,hrs,hour,hours
day|Days|86400.0|d,day,days
week|Weeks|604800.0|wk,week,weeks
mo|Months|2629746.0|mo,month,months
yr|Years|31556952.0|yr,year,years
workdays|Workdays|28800.0|workday,workdays,businessday,businessdays
area
mm²|Square Millimeters|1e-06|mm2,sqmm,sq mm,square millimeter,square millimeters,square millimetre,square millimetres
cm²|Square Centimeters|0.0001|cm2,sqcm,sq cm,square centimeter,square centimeters,square centimetre,square centimetres
dm²|Square Decimeters|0.01|dm2,sqdm
m²|Square Meters|1.0|m2,sqm,sq m,square meter,square meters,square metre,square metres
km²|Square Kilometers|1000000.0|km2,sqkm,sq km,square kilometer,square kilometers,square kilometre,square kilometres
in²|Square Inches|0.00064516|in2,sqin,sq in,square inch,square inches
ft²|Square Feet|0.09290304|ft2,sqft,sq ft,square foot,square feet
yd²|Square Yards|0.83612736|yd2,sqyd,sq yd,square yard,square yards
mi²|Square Miles|2589988.110336|mi2,sqmi,sq mi,square mile,square miles
acre|Acres|4046.8564224|acre,acres
ha|Hectares|10000.0|ha,hectare,hectares
volume
mL|Milliliters|1e-06|ml,milliliter,milliliters,millilitre,millilitres
µL|Microliters|1e-09|ul,µl,μl,microliter,microliters,microlitre,microlitres
cL|Centiliters|1e-05|cl,centiliter,centiliters,centilitre,centilitres
dL|Deciliters|0.0001|dl,deciliter,deciliters,decilitre,decilitres
L|Liters|0.001|l,liter,liters,litre,litres
kL|Kiloliters|1.0|kl,kiloliter,kiloliters,kilolitre,kilolitres
ML|Megaliters|1000.0|megaliter,megaliters,megalitre,megalitres
metric cup|Metric Cups|0.00025|metric cup,metric cups
bu|Bushels|0.03523907016688|bu,bushel,bushels
bbl|Barrels|0.158987294928|bbl,barrel,barrels
acre ft|Acre-Feet|1233.48183754752|acre foot,acre feet
cup|Cups|0.0002365882365|cup,cups
tbsp|Tablespoons|1.478676478125e-05|tbsp,tablespoon,tablespoons
tsp|Teaspoons|4.92892159375e-06|tsp,teaspoon,teaspoons
gal|Gallons|0.003785411784|gal,gallon,gallons
qt|Quarts|0.000946352946|qt,quart,quarts
pt|Pints|0.000473176473|pt,pint,pints
fl oz|Fluid Ounces|2.95735295625e-05|floz,fl oz
mm³|Cubic Millimeters|1e-09|mm3
cm³|Cubic Centimeters|1e-06|cm3,cc,cubic centimeter,cubic centimeters,cubic centimetre,cubic centimetres
dm³|Cubic Decimeters|0.001|dm3
m³|Cubic Meters|1.0|m3,cu m,cubic meter,cubic meters,cubic metre,cubic metres
in³|Cubic Inches|1.6387064e-05|in3,cu in,cubic inch,cubic inches
ft³|Cubic Feet|0.028316846592|ft3,cu ft,cubic foot,cubic feet
yd³|Cubic Yards|0.764554857984|yd3,cu yd,cubic yard,cubic yards
km³|Cubic Kilometers|1000000000.0|km3,cubic km,cubic kilometer,cubic kilometers,cubic kilometre,cubic kilometres
mi³|Cubic Miles|4168181825.440579584|mi3,cubic mi,cubic mile,cubic miles
volumeFlow
L/s|Liters per Second|0.001|l/s,l/sec
L/min|Liters per Minute|1.6666666666666667e-05|l/min,lpm
L/h|Liters per Hour|2.7777777777777776e-07|l/h,l/hr,lph
m³/s|Cubic Meters per Second|1.0|m3/s,m3/sec
m³/h|Cubic Meters per Hour|0.0002777777777777778|m3/h,m3/hr
gal/min|Gallons per Minute|6.30901964e-05|gal/min,gpm
fuelEconomy
km/L|Kilometers per Liter|1000000.0|km/l
mpg|Miles per Gallon|425143.707430272|mpg
fuelConsumption
L/100km|Liters per 100 Kilometers|1e-08|l/100km
digitalStorage
bit|Bits|0.125|bit,bits
B|Bytes|1.0|b,byte,bytes
kB|Kilobytes|1000.0|kb,kilobyte,kilobytes
MB|Megabytes|1000000.0|mb,megabyte,megabytes
GB|Gigabytes|1000000000.0|gb,gigabyte,gigabytes
TB|Terabytes|1000000000000.0|tb,terabyte,terabytes
PB|Petabytes|1000000000000000.0|pb,petabyte,petabytes
KiB|Kibibytes|1024.0|kib,kibibyte,kibibytes
MiB|Mebibytes|1048576.0|mib,mebibyte,mebibytes
GiB|Gibibytes|1073741824.0|gib,gibibyte,gibibytes
TiB|Tebibytes|1099511627776.0|tib,tebibyte,tebibytes
PiB|Pebibytes|1125899906842624.0|pib,pebibyte,pebibytes
EB|Exabytes|1e18|eb,exabyte,exabytes
EiB|Exbibytes|1152921504606846976.0|eib,exbibyte,exbibytes
ZB|Zettabytes|1e21|zb,zettabyte,zettabytes
YB|Yottabytes|1e24|yb,yottabyte,yottabytes
angle
rad|Radians|1.0|rad,radian,radians
deg|Degrees|0.017453292519943295|deg,degree,degrees,°
grad|Gradians|0.015707963267948967|grad,grads,gradian,gradians,gon
arcmin|Arcminutes|0.0002908882086657216|arcmin,arcminute,arcminutes
arcsec|Arcseconds|4.84813681109536e-06|arcsec,arcsecond,arcseconds
turn|Turns|6.283185307179586|turn,turns,rev,revolution,revolutions
speed
m/s|Meters per Second|1.0|mps,m/s,m/sec
km/h|Kilometers per Hour|0.2777777777777778|kmh,kph,km/h,km/hr,kmph
mph|Miles per Hour|0.44704|mph,mi/h,mi/hr
ft/s|Feet per Second|0.3048|fps,ft/s,ft/sec
kn|Knots|0.5144444444444445|kn,knot,knots
km/s|Kilometers per Second|1000.0|km/s,km/sec
pressure
Pa|Pascals|1.0|pa,pascal,pascals
hPa|Hectopascals|100.0|hpa
kPa|Kilopascals|1000.0|kpa
bar|Bar|100000.0|bar,bars
mbar|Millibar|100.0|mbar,millibar,millibars
psi|PSI|6894.757293168|psi
atm|Atmospheres|101325.0|atm,atmosphere,atmospheres
mmHg|Millimeters of Mercury|133.322387415|mmhg
Torr|Torr|133.32236842105263|torr
dataRate
bps|Bits per Second|0.125|bps
Kbps|Kilobits per Second|125.0|kbps,kbit/s,kb/s
Mbps|Megabits per Second|125000.0|mbps,mbit/s,mb/s
Gbps|Gigabits per Second|125000000.0|gbps,gbit/s,gb/s
Tbps|Terabits per Second|125000000000.0|tbps,tbit/s,tb/s
Bps|Bytes per Second|1.0|Bps
kBps|Kilobytes per Second|1000.0|kBps,KBps
MBps|Megabytes per Second|1000000.0|MBps
GBps|Gigabytes per Second|1000000000.0|GBps
TBps|Terabytes per Second|1000000000000.0|TBps
acceleration
m/s²|Meters per Second Squared|1.0|m/s2,mps2
ft/s²|Feet per Second Squared|0.3048|ft/s2,fps2
force
N|Newtons|1.0|n,newton,newtons
energy
J|Joules|1.0|j,joule,joules
kJ|Kilojoules|1000.0|kj,kilojoule,kilojoules
Wh|Watt Hours|3600.0|wh
mWh|Milliwatt Hours|3.6|mwh
kWh|Kilowatt Hours|3600000.0|kwh
MWh|Megawatt Hours|3600000000.0|MWh,megawatthour,megawatthours
GWh|Gigawatt Hours|3600000000000.0|GWh,gwh,gigawatthour,gigawatthours
TWh|Terawatt Hours|3600000000000000.0|TWh,twh,terawatthour,terawatthours
cal|Calories|4.184|cal,calorie,calories
kcal|Kilocalories|4184.0|kcal,kilocalorie,kilocalories
power
W|Watts|1.0|w,watt,watts
mW|Milliwatts|0.001|mw,milliwatt,milliwatts
kW|Kilowatts|1000.0|kw,kilowatt,kilowatts
MW|Megawatts|1000000.0|MW,megawatt,megawatts
electricCurrent
A|Amperes|1.0|a,amp,amps,ampere,amperes
mA|Milliamperes|0.001|ma,milliamp,milliamps,milliampere,milliamperes
MA|Megaamperes|1000000.0|MA,megaamp,megaamps
µA|Microamperes|1e-06|ua,µa,μa,microamp,microamps
kA|Kiloamperes|1000.0|kA,kiloamp,kiloamps,kiloampere,kiloamperes
voltage
V|Volts|1.0|v,volt,volts
mV|Millivolts|0.001|mv,millivolt,millivolts
kV|Kilovolts|1000.0|kv,kilovolt,kilovolts
MV|Megavolts|1000000.0|MV,megavolt,megivolts
µV|Microvolts|1e-06|uv,µv,μv,microvolt,microvolts
resistance
Ω|Ohms|1.0|ω,ohm,ohms
mΩ|Milliohms|0.001|mω,milliohm,milliohms
kΩ|Kilohms|1000.0|kω,kohm,kohms,kilohm,kilohms,kiloohm,kilohms
MΩ|Megohms|1000000.0|MΩ,megohm,megohms
µΩ|Microohms|1e-06|µω,μω,microohm,microohms
electricCharge
As|Coulombs|1.0|as,coulomb,coulombs
Ah|Ampere Hours|3600.0|ah,amphour,amphours
mAh|Milliampere Hours|3.6|mah,milliamphour,milliamphours
MAh|Megaampere Hours|3600000000.0|MAh
frequency
Hz|Hertz|1.0|hz,hertz
kHz|Kilohertz|1000.0|khz,kilohertz
MHz|Megahertz|1000000.0|mhz,megahertz
volume
UK gal|UK Gallons|0.00454609|ukgal,ukgallon,ukgallons,imperial gallon,imperial gallons
UK qt|UK Quarts|0.0011365225|ukqt,ukquart,ukquarts,imperial quart,imperial quarts
UK pt|UK Pints|0.00056826125|ukpt,ukpint,ukpints,imperial pint,imperial pints
UK fl oz|UK Fluid Ounces|2.84130625e-05|ukfloz
UK tbsp|UK Tablespoons|1.7758164062499998e-05|uktbsp,imperial tablespoon,imperial tablespoons
UK tsp|UK Teaspoons|5.919388020833333e-06|uktsp,imperial teaspoon,imperial teaspoons
power
hp|Horsepower|745.6998715822702|hp,horsepower
energy
BTU|British Thermal Units|1055.05585262|btu
frequency
rpm|Revolutions per Minute|0.016666666666666666|rpm
rps|Revolutions per Second|1.0|rps,rev/s
rad/s|Radians per Second|0.15915494309189535|rad/s
force
lbf|Pounds Force|4.4482216152605|lbf,poundforce
pdl|Poundals|0.138254954376|pdl,poundal,poundals
pressure
ksi|KSI|6894757.293168|ksi
capacitance
F|Farads|1.0|farad,farads
mF|Millifarads|0.001|mF,millifarad,millifarads
µF|Microfarads|1e-06|uf,µf,μf,microfarad,microfarads
nF|Nanofarads|1e-09|nf,nanofarad,nanofarads
pF|Picofarads|1e-12|pf,picofarad,picofarads
inductance
H|Henries|1.0|henry,henries,henrys
mH|Millihenries|0.001|mH,millihenry,millihenries,millihenrys
µH|Microhenries|1e-06|uh,µh,μh,microhenry,microhenries,microhenrys
conductance
S|Siemens|1.0|siemens
mS|Millisiemens|0.001|millisiemens
µS|Microsiemens|1e-06|microsiemens
magneticFlux
Wb|Webers|1.0|wb,weber,webers
mWb|Milliwebers|0.001|mwb,milliweber,milliwebers
magneticFluxDensity
T|Teslas|1.0|tesla,teslas
mT|Milliteslas|0.001|mT,millitesla,milliteslas
µT|Microteslas|1e-06|µt,μt,microtesla,microteslas
G|Gauss|0.0001|gauss
torque
N·m|Newton Meters|1.0|Nm,newtonmeter,newtonmeters,newton meter,newton meters,newton metre,newton metres
lb·ft|Pound Feet|1.3558179483314004|lbft,lb ft,lbf ft,pound foot,pound feet
lb·in|Pound Inches|0.1129848290276167|lbin
oz·in|Ounce Inches|0.00706155176214271|ozin
substance
mol|Moles|1.0|mol,mole,moles
mmol|Millimoles|0.001|mmol,millimole,millimoles
µmol|Micromoles|1e-06|umol,µmol,μmol,micromole,micromoles
nmol|Nanomoles|1e-09|nmol,nanomole,nanomoles
";
