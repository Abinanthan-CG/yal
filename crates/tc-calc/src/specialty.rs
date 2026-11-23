//! Clean-room Swiss-Army-Knife specialty calculators:
//!
//! - Natural-language question unwrapper (`what is ...`, `how many ...`, `convert ...`)
//! - Data transfer time (`transfer 1 TB at 100 Mbps`)
//! - Running / cycling pace (`5:00 min/km in km/h`, `13:24 min/mile in mph`)
//! - Musical pitch 12-TET (`c4 to hz`, `440 hz to note`)
//! - Chemistry molar mass with full 118-element IUPAC table (`molar mass of C6H12O6`)
//! - Physical constants (`speed of light in km/s`, `1 astronomical unit / speed of light in minutes`)
//! - CSS & print pixel density (`2rem in px`, `32px in rem`, `2 inches in px at 72 ppi`, `1920x1080 aspect ratio`)
//! - Prime/double-prime heights (`6'2" to m`, `5'10" in cm`)
//! - Cooking ingredient mass <-> volume (`1 cup flour in grams`, `200g flour in cups`)
//! - Quoted text utilities (`first 10 characters of "hello world"`, `length of "..."`, `words in "..."`)
//! - Minor currency units (`$12 in cents`, `£5 in pence`, `€2.50 in euro cents`)
//! - Extra specialized units & fraction forms (`30 dBm in watts`, `491.67 rankine in celsius`,
//!   `1 chain in yards`, `1 fortnight in days`, `1 nibble in bits`, `55h in workdays`,
//!   `1 L/s in gpm`, `1.75 as improper`, `7/4 as mixed`)

use crate::context::CalcContext;
use crate::datetime::format_timespan_seconds;
use crate::formatter::Formatter;
use crate::result::{CalcResult, Payload};

/// Strip natural-language question leads such as `"what is ..."` or `"convert ..."`.
pub fn unwrap_question(query: &str) -> Option<&str> {
    let trimmed = query.trim().trim_end_matches('?').trim();
    let lower = trimmed.to_ascii_lowercase();
    const PREFIXES: &[&str] = &[
        "what is ",
        "what's ",
        "whats ",
        "how much is ",
        "how many ",
        "convert ",
        "calculate ",
        "compute ",
        "find ",
    ];
    for prefix in PREFIXES {
        if lower.starts_with(prefix) {
            let rest = trimmed[prefix.len()..].trim();
            if !rest.is_empty() {
                return Some(rest);
            }
        }
    }
    None
}

/// Evaluate all specialty Swiss-Army-Knife domains.
pub fn evaluate(query: &str, ctx: &CalcContext) -> Option<CalcResult> {
    let raw = query.trim().trim_end_matches('?').trim();
    if raw.is_empty() || raw.len() > 256 {
        return None;
    }

    // Order from most syntactically specific to most general.
    if let Some(c) = eval_text_utility(raw) {
        return Some(c);
    }
    if let Some(c) = eval_city_distance(raw) {
        return Some(c);
    }
    if let Some(c) = eval_molar_mass(raw) {
        return Some(c);
    }
    if let Some(c) = eval_transfer_time(raw) {
        return Some(c);
    }
    if let Some(c) = eval_pace(raw) {
        return Some(c);
    }
    if let Some(c) = eval_pitch(raw) {
        return Some(c);
    }
    if let Some(c) = eval_physical_constants(raw) {
        return Some(c);
    }
    if let Some(c) = eval_pixel_density_and_css(raw, ctx) {
        return Some(c);
    }
    if let Some(c) = eval_prime_height(raw) {
        return Some(c);
    }
    if let Some(c) = eval_cooking(raw) {
        return Some(c);
    }
    if let Some(c) = eval_minor_currency(raw) {
        return Some(c);
    }
    if let Some(c) = eval_fraction_forms(raw) {
        return Some(c);
    }
    if let Some(c) = eval_specialized_units(raw) {
        return Some(c);
    }
    None
}

// ---------------------------------------------------------------------------
// 1. Quoted Text Utilities
// ---------------------------------------------------------------------------

fn extract_quoted(s: &str) -> Option<(&str, &str)> {
    let start = s.find('"')?;
    let rest = &s[start + 1..];
    let end = rest.rfind('"')?;
    let prefix = s[..start].trim();
    let content = &rest[..end];
    if !rest[end + 1..].trim().is_empty() {
        return None;
    }
    Some((prefix, content))
}

fn eval_text_utility(raw: &str) -> Option<CalcResult> {
    let (prefix_raw, text) = extract_quoted(raw)?;
    let prefix = prefix_raw
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // `first <n> characters/chars/letters of "..."`
    if let Some(rest) = prefix.strip_prefix("first ") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 2
            && matches!(
                parts[1],
                "character" | "characters" | "char" | "chars" | "letter" | "letters"
            )
        {
            let n = parts[0].parse::<usize>().ok()?;
            let out: String = text.chars().take(n).collect();
            let mut card =
                CalcResult::with_badges(raw.to_string(), "Text", "Substring", Payload::text(&out));
            card.can_chain = false;
            return Some(card);
        }
    }

    // `last <n> characters/chars/letters of "..."`
    if let Some(rest) = prefix.strip_prefix("last ") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 2
            && matches!(
                parts[1],
                "character" | "characters" | "char" | "chars" | "letter" | "letters"
            )
        {
            let n = parts[0].parse::<usize>().ok()?;
            let chars: Vec<char> = text.chars().collect();
            let start = chars.len().saturating_sub(n);
            let out: String = chars[start..].iter().collect();
            let mut card =
                CalcResult::with_badges(raw.to_string(), "Text", "Substring", Payload::text(&out));
            card.can_chain = false;
            return Some(card);
        }
    }

    match prefix.as_str() {
        "length of" | "length" | "len of" | "len" | "characters in" | "characters of"
        | "chars in" | "chars of" => {
            let count = text.chars().count();
            let mut card = CalcResult::with_badges(
                raw.to_string(),
                "Text",
                "Characters",
                Payload::number(count as f64),
            );
            card.can_chain = false;
            Some(card)
        }
        "words in" | "words of" | "word count" | "word count of" => {
            let count = text.split_whitespace().count();
            let mut card = CalcResult::with_badges(
                raw.to_string(),
                "Text",
                "Words",
                Payload::number(count as f64),
            );
            card.can_chain = false;
            Some(card)
        }
        "uppercase" | "upper" | "uppercase of" => {
            let mut card = CalcResult::with_badges(
                raw.to_string(),
                "Text",
                "Uppercase",
                Payload::text(&text.to_uppercase()),
            );
            card.can_chain = false;
            Some(card)
        }
        "lowercase" | "lower" | "lowercase of" => {
            let mut card = CalcResult::with_badges(
                raw.to_string(),
                "Text",
                "Lowercase",
                Payload::text(&text.to_lowercase()),
            );
            card.can_chain = false;
            Some(card)
        }
        "reverse" | "reverse of" => {
            let rev: String = text.chars().rev().collect();
            let mut card =
                CalcResult::with_badges(raw.to_string(), "Text", "Reversed", Payload::text(&rev));
            card.can_chain = false;
            Some(card)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// 1b. Great-Circle Distance Between World Cities (Haversine formula)
// ---------------------------------------------------------------------------

fn eval_city_distance(raw: &str) -> Option<CalcResult> {
    let folded = fold_ascii_simple(raw);
    let rest = folded
        .strip_prefix("distance between ")
        .or_else(|| folded.strip_prefix("distance from "))
        .or_else(|| folded.strip_prefix("distance "))?;

    // Optional trailing target unit: `in miles`, `to km`, `in nm`, `in meters`.
    let (places_part, target_unit) = if let Some((l, r)) = rest
        .rsplit_once(" in ")
        .or_else(|| rest.rsplit_once(" to "))
    {
        if matches!(
            r.trim(),
            "km" | "kilometers"
                | "kilometres"
                | "mi"
                | "mile"
                | "miles"
                | "m"
                | "meters"
                | "nm"
                | "nmi"
                | "nautical miles"
        ) {
            (l.trim(), Some(r.trim()))
        } else {
            (rest.trim(), None)
        }
    } else {
        (rest.trim(), None)
    };

    let (a_str, b_str) = places_part
        .split_once(" to ")
        .or_else(|| places_part.split_once(" and "))
        .or_else(|| places_part.split_once(" - "))?;
    let a = lookup_city_coords(a_str.trim())?;
    let b = lookup_city_coords(b_str.trim())?;

    let km = haversine_km(a.1, a.2, b.1, b.2);
    let (val, sym) = match target_unit {
        Some("mi" | "mile" | "miles") => (km / 1.609_344, "mi"),
        Some("m" | "meters") => (km * 1000.0, "m"),
        Some("nm" | "nmi" | "nautical miles") => (km / 1.852, "nmi"),
        _ => (km, "km"),
    };
    let rounded = (val * 100.0).round() / 100.0;
    let text = Formatter::copy_text(rounded);
    Some(CalcResult::with_badges(
        format!("{} → {}", a.0, b.0),
        "Distance",
        sym,
        Payload::Value {
            display: format!("{} {sym}", Formatter::grouped(&text)),
            copy_text: format!("{text} {sym}"),
        },
    ))
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R_KM: f64 = 6_371.008_8;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let lat1_r = lat1.to_radians();
    let lat2_r = lat2.to_radians();
    let a = (dlat / 2.0).sin().powi(2) + lat1_r.cos() * lat2_r.cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    R_KM * c
}

fn fold_ascii_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let m = match ch {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => {
                'a'
            }
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' | 'Ø' => {
                'o'
            }
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ñ' | 'Ñ' => 'n',
            'ç' | 'Ç' => 'c',
            other => other.to_ascii_lowercase(),
        };
        out.push(m);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Curated WGS84 coordinates `(display_name, lat_deg, lon_deg)` for major world cities.
fn lookup_city_coords(key: &str) -> Option<(&'static str, f64, f64)> {
    Some(match key {
        "london" | "ldn" | "lhr" => ("London", 51.5074, -0.1278),
        "paris" | "cdg" => ("Paris", 48.8566, 2.3522),
        "chamonix" => ("Chamonix", 45.9237, 6.8694),
        "tokyo" | "nrt" | "hnd" => ("Tokyo", 35.6762, 139.6503),
        "new york" | "nyc" | "jfk" => ("New York", 40.7128, -74.0060),
        "san francisco" | "sf" | "sfo" => ("San Francisco", 37.7749, -122.4194),
        "los angeles" | "la" | "lax" => ("Los Angeles", 34.0522, -118.2437),
        "chicago" | "chi" | "ord" => ("Chicago", 41.8781, -87.6298),
        "seattle" | "sea" => ("Seattle", 47.6062, -122.3321),
        "boston" | "bos" => ("Boston", 42.3601, -71.0589),
        "miami" | "mia" => ("Miami", 25.7617, -80.1918),
        "austin" => ("Austin", 30.2672, -97.7431),
        "dallas" | "dfw" => ("Dallas", 32.7767, -96.7970),
        "denver" | "den" => ("Denver", 39.7392, -104.9903),
        "washington" | "dc" => ("Washington, D.C.", 38.9072, -77.0369),
        "honolulu" => ("Honolulu", 21.3069, -157.8583),
        "toronto" | "yyz" => ("Toronto", 43.6532, -79.3832),
        "vancouver" | "yvr" => ("Vancouver", 49.2827, -123.1207),
        "montreal" => ("Montreal", 45.5017, -73.5673),
        "mexico city" | "mex" => ("Mexico City", 19.4326, -99.1332),
        "sao paulo" | "gru" => ("São Paulo", -23.5505, -46.6333),
        "rio de janeiro" | "rio" => ("Rio de Janeiro", -22.9068, -43.1729),
        "buenos aires" => ("Buenos Aires", -34.6037, -58.3816),
        "santiago" => ("Santiago", -33.4489, -70.6693),
        "lima" => ("Lima", -12.0464, -77.0428),
        "bogota" => ("Bogotá", 4.7110, -74.0721),
        "berlin" => ("Berlin", 52.5200, 13.4050),
        "munich" | "munchen" => ("Munich", 48.1351, 11.5820),
        "frankfurt" | "fra" => ("Frankfurt", 50.1109, 8.6821),
        "amsterdam" | "ams" => ("Amsterdam", 52.3676, 4.9041),
        "madrid" => ("Madrid", 40.4168, -3.7038),
        "barcelona" => ("Barcelona", 41.3874, 2.1686),
        "rome" => ("Rome", 41.9028, 12.4964),
        "milan" => ("Milan", 45.4642, 9.1900),
        "zurich" => ("Zurich", 47.3769, 8.5417),
        "geneva" => ("Geneva", 46.2044, 6.1432),
        "vienna" => ("Vienna", 48.2082, 16.3738),
        "prague" => ("Prague", 50.0755, 14.4378),
        "warsaw" => ("Warsaw", 52.2297, 21.0122),
        "dublin" => ("Dublin", 53.3498, -6.2603),
        "lisbon" => ("Lisbon", 38.7223, -9.1393),
        "stockholm" => ("Stockholm", 59.3293, 18.0686),
        "oslo" => ("Oslo", 59.9139, 10.7522),
        "copenhagen" => ("Copenhagen", 55.6761, 12.5683),
        "helsinki" => ("Helsinki", 60.1699, 24.9384),
        "athens" => ("Athens", 37.9838, 23.7275),
        "istanbul" => ("Istanbul", 41.0082, 28.9784),
        "moscow" => ("Moscow", 55.7558, 37.6173),
        "reykjavik" => ("Reykjavik", 64.1466, -21.9426),
        "dubai" | "dxb" => ("Dubai", 25.2048, 55.2708),
        "riyadh" => ("Riyadh", 24.7136, 46.6753),
        "doha" => ("Doha", 25.2854, 51.5310),
        "cairo" => ("Cairo", 30.0444, 31.2357),
        "johannesburg" => ("Johannesburg", -26.2041, 28.0473),
        "cape town" => ("Cape Town", -33.9249, 18.4241),
        "nairobi" => ("Nairobi", -1.2921, 36.8219),
        "lagos" => ("Lagos", 6.5244, 3.3792),
        "mumbai" | "bom" => ("Mumbai", 19.0760, 72.8777),
        "delhi" | "new delhi" | "del" => ("New Delhi", 28.6139, 77.2090),
        "bengaluru" | "bangalore" | "blr" => ("Bengaluru", 12.9716, 77.5946),
        "chennai" | "maa" => ("Chennai", 13.0827, 80.2707),
        "kochi" | "cochin" | "cok" => ("Kochi", 9.9312, 76.2673),
        "kolkata" => ("Kolkata", 22.5726, 88.3639),
        "hyderabad" => ("Hyderabad", 17.3850, 78.4867),
        "pune" => ("Pune", 18.5204, 73.8567),
        "colombo" => ("Colombo", 6.9271, 79.8612),
        "kathmandu" => ("Kathmandu", 27.7172, 85.3240),
        "karachi" => ("Karachi", 24.8607, 67.0011),
        "dhaka" => ("Dhaka", 23.8103, 90.4125),
        "bangkok" => ("Bangkok", 13.7563, 100.5018),
        "singapore" | "sin" => ("Singapore", 1.3521, 103.8198),
        "kuala lumpur" => ("Kuala Lumpur", 3.1390, 101.6869),
        "jakarta" => ("Jakarta", -6.2088, 106.8456),
        "manila" => ("Manila", 14.5995, 120.9842),
        "hong kong" | "hkg" => ("Hong Kong", 22.3193, 114.1694),
        "taipei" => ("Taipei", 25.0330, 121.5654),
        "shanghai" => ("Shanghai", 31.2304, 121.4737),
        "beijing" => ("Beijing", 39.9042, 116.4074),
        "seoul" | "icn" => ("Seoul", 37.5665, 126.9780),
        "osaka" => ("Osaka", 34.6937, 135.5023),
        "sydney" | "syd" => ("Sydney", -33.8688, 151.2093),
        "melbourne" | "mel" => ("Melbourne", -37.8136, 144.9631),
        "brisbane" | "bne" => ("Brisbane", -27.4705, 153.0260),
        "perth" => ("Perth", -31.9505, 115.8605),
        "auckland" => ("Auckland", -36.8485, 174.7633),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// 2. Chemistry Molar Mass (full 118-element IUPAC atomic weights)
// ---------------------------------------------------------------------------

fn eval_molar_mass(raw: &str) -> Option<CalcResult> {
    let lower = raw.to_ascii_lowercase();
    let formula_raw = if let Some(pos) = lower.strip_prefix("molar mass of ") {
        &raw[raw.len() - pos.len()..]
    } else if let Some(pos) = lower.strip_prefix("molar mass ") {
        &raw[raw.len() - pos.len()..]
    } else if let Some(pos) = lower.strip_prefix("molecular weight of ") {
        &raw[raw.len() - pos.len()..]
    } else {
        let pos = lower.strip_prefix("molecular mass of ")?;
        &raw[raw.len() - pos.len()..]
    };

    let formula = formula_raw.trim();
    if formula.is_empty() {
        return None;
    }

    let mass = parse_chemical_formula(formula)?;
    let rounded = (mass * 1000.0).round() / 1000.0;
    let num = Formatter::copy_text(rounded);
    let display = format!("{} g/mol", Formatter::grouped(&num));
    let copy_text = format!("{num} g/mol");
    Some(CalcResult::with_badges(
        formula.to_string(),
        "Chemistry",
        "Molar Mass",
        Payload::Value { display, copy_text },
    ))
}

fn parse_chemical_formula(formula: &str) -> Option<f64> {
    // Support hydration dot `·` or `*` (e.g., `CuSO4·5H2O`).
    let mut total = 0.0;
    for part in formula.split(['·', '*', '•']) {
        let p = part.trim();
        if p.is_empty() {
            return None;
        }
        let chars: Vec<char> = p.chars().collect();
        let mut idx = 0usize;
        // Optional leading stoichiometric multiplier (`5H2O`).
        let mult = parse_leading_uint(&chars, &mut idx).unwrap_or(1) as f64;
        let sub = parse_formula_group(&chars, &mut idx, None)?;
        if idx != chars.len() {
            return None;
        }
        total += mult * sub;
    }
    if total > 0.0 {
        Some(total)
    } else {
        None
    }
}

fn parse_formula_group(chars: &[char], idx: &mut usize, closing: Option<char>) -> Option<f64> {
    let mut sum = 0.0;
    let mut saw_item = false;
    while *idx < chars.len() {
        let ch = chars[*idx];
        if Some(ch) == closing {
            *idx += 1;
            return if saw_item { Some(sum) } else { None };
        }
        if ch == '(' || ch == '[' {
            let expected = if ch == '(' { ')' } else { ']' };
            *idx += 1;
            let inner = parse_formula_group(chars, idx, Some(expected))?;
            let count = parse_leading_uint(chars, idx).unwrap_or(1) as f64;
            sum += inner * count;
            saw_item = true;
            continue;
        }
        if ch.is_ascii_uppercase() {
            let start = *idx;
            *idx += 1;
            if *idx < chars.len() && chars[*idx].is_ascii_lowercase() {
                *idx += 1;
            }
            let sym: String = chars[start..*idx].iter().collect();
            let weight = atomic_weight(&sym)?;
            let count = parse_leading_uint(chars, idx).unwrap_or(1) as f64;
            sum += weight * count;
            saw_item = true;
            continue;
        }
        return None;
    }
    if closing.is_some() || !saw_item {
        None
    } else {
        Some(sum)
    }
}

fn parse_leading_uint(chars: &[char], idx: &mut usize) -> Option<u32> {
    let start = *idx;
    while *idx < chars.len() && chars[*idx].is_ascii_digit() {
        *idx += 1;
    }
    if *idx == start {
        return None;
    }
    let s: String = chars[start..*idx].iter().collect();
    s.parse::<u32>().ok()
}

/// Standard IUPAC atomic weights (g/mol) for all 118 elements.
fn atomic_weight(symbol: &str) -> Option<f64> {
    Some(match symbol {
        "H" => 1.008,
        "He" => 4.0026,
        "Li" => 6.94,
        "Be" => 9.0122,
        "B" => 10.81,
        "C" => 12.011,
        "N" => 14.007,
        "O" => 15.999,
        "F" => 18.998,
        "Ne" => 20.180,
        "Na" => 22.990,
        "Mg" => 24.305,
        "Al" => 26.982,
        "Si" => 28.085,
        "P" => 30.974,
        "S" => 32.06,
        "Cl" => 35.45,
        "Ar" => 39.948,
        "K" => 39.098,
        "Ca" => 40.078,
        "Sc" => 44.956,
        "Ti" => 47.867,
        "V" => 50.942,
        "Cr" => 51.996,
        "Mn" => 54.938,
        "Fe" => 55.845,
        "Co" => 58.933,
        "Ni" => 58.693,
        "Cu" => 63.546,
        "Zn" => 65.38,
        "Ga" => 69.723,
        "Ge" => 72.630,
        "As" => 74.922,
        "Se" => 78.971,
        "Br" => 79.904,
        "Kr" => 83.798,
        "Rb" => 85.468,
        "Sr" => 87.62,
        "Y" => 88.906,
        "Zr" => 91.224,
        "Nb" => 92.906,
        "Mo" => 95.95,
        "Tc" => 98.0,
        "Ru" => 101.07,
        "Rh" => 102.91,
        "Pd" => 106.42,
        "Ag" => 107.87,
        "Cd" => 112.41,
        "In" => 114.82,
        "Sn" => 118.71,
        "Sb" => 121.76,
        "Te" => 127.60,
        "I" => 126.90,
        "Xe" => 131.29,
        "Cs" => 132.91,
        "Ba" => 137.33,
        "La" => 138.91,
        "Ce" => 140.12,
        "Pr" => 140.91,
        "Nd" => 144.24,
        "Pm" => 145.0,
        "Sm" => 150.36,
        "Eu" => 151.96,
        "Gd" => 157.25,
        "Tb" => 158.93,
        "Dy" => 162.50,
        "Ho" => 164.93,
        "Er" => 167.26,
        "Tm" => 168.93,
        "Yb" => 173.05,
        "Lu" => 174.97,
        "Hf" => 178.49,
        "Ta" => 180.95,
        "W" => 183.84,
        "Re" => 186.21,
        "Os" => 190.23,
        "Ir" => 192.22,
        "Pt" => 195.08,
        "Au" => 196.97,
        "Hg" => 200.59,
        "Tl" => 204.38,
        "Pb" => 207.2,
        "Bi" => 208.98,
        "Po" => 209.0,
        "At" => 210.0,
        "Rn" => 222.0,
        "Fr" => 223.0,
        "Ra" => 226.0,
        "Ac" => 227.0,
        "Th" => 232.04,
        "Pa" => 231.04,
        "U" => 238.03,
        "Np" => 237.0,
        "Pu" => 244.0,
        "Am" => 243.0,
        "Cm" => 247.0,
        "Bk" => 247.0,
        "Cf" => 251.0,
        "Es" => 252.0,
        "Fm" => 257.0,
        "Md" => 258.0,
        "No" => 259.0,
        "Lr" => 266.0,
        "Rf" => 267.0,
        "Db" => 268.0,
        "Sg" => 269.0,
        "Bh" => 270.0,
        "Hs" => 277.0,
        "Mt" => 278.0,
        "Ds" => 281.0,
        "Rg" => 282.0,
        "Cn" => 285.0,
        "Nh" => 286.0,
        "Fl" => 289.0,
        "Mc" => 290.0,
        "Lv" => 293.0,
        "Ts" => 294.0,
        "Og" => 294.0,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// 3. Data Transfer / Download / Upload Time
// ---------------------------------------------------------------------------

fn eval_transfer_time(raw: &str) -> Option<CalcResult> {
    let lower = raw.to_ascii_lowercase();
    let body = lower
        .strip_prefix("how long to download ")
        .or_else(|| lower.strip_prefix("how long to upload "))
        .or_else(|| lower.strip_prefix("how long to transfer "))
        .or_else(|| lower.strip_prefix("time to download "))
        .or_else(|| lower.strip_prefix("time to upload "))
        .or_else(|| lower.strip_prefix("time to transfer "))
        .or_else(|| lower.strip_prefix("download time for "))
        .or_else(|| lower.strip_prefix("upload time for "))
        .or_else(|| lower.strip_prefix("transfer time for "))
        .or_else(|| lower.strip_prefix("transfer "))
        .or_else(|| lower.strip_prefix("download "))
        .or_else(|| lower.strip_prefix("upload "))
        .unwrap_or(&lower);
    let (size_part, speed_part) = body
        .split_once(" at ")
        .or_else(|| body.split_once(" over "))
        .or_else(|| body.split_once(" @ "))?;

    let bits = parse_data_bits(size_part.trim())?;
    let bps = parse_bitrate_bps(speed_part.trim())?;
    if bps <= 0.0 {
        return None;
    }
    let seconds = bits / bps;
    let display = format_timespan_seconds(seconds);
    let mut card = CalcResult::with_badges(
        raw.to_string(),
        "Transfer",
        "Duration",
        Payload::text(&display),
    );
    card.can_chain = false;
    Some(card)
}

fn parse_data_bits(s: &str) -> Option<f64> {
    let (val, unit) = parse_qty_unit(s)?;
    let bits_per = match unit {
        "b" | "bit" | "bits" => 1.0,
        "byte" | "bytes" => 8.0,
        "kb" | "kilobyte" | "kilobytes" => 8.0 * 1e3,
        "mb" | "megabyte" | "megabytes" => 8.0 * 1e6,
        "gb" | "gigabyte" | "gigabytes" => 8.0 * 1e9,
        "tb" | "terabyte" | "terabytes" => 8.0 * 1e12,
        "pb" | "petabyte" | "petabytes" => 8.0 * 1e15,
        "kib" => 8.0 * 1024.0,
        "mib" => 8.0 * 1024.0_f64.powi(2),
        "gib" => 8.0 * 1024.0_f64.powi(3),
        "tib" => 8.0 * 1024.0_f64.powi(4),
        _ => return None,
    };
    Some(val * bits_per)
}

fn parse_bitrate_bps(s: &str) -> Option<f64> {
    let (val, unit) = parse_qty_unit(s)?;
    let bps_per = match unit {
        "bps" | "b/s" | "bit/s" | "bits/s" => 1.0,
        "kbps" | "kb/s" if unit == "kbps" => 1e3,
        "mbps" => 1e6,
        "gbps" => 1e9,
        "tbps" => 1e12,
        "kb/s" => 8.0 * 1e3,
        "mb/s" => 8.0 * 1e6,
        "gb/s" => 8.0 * 1e9,
        "kib/s" => 8.0 * 1024.0,
        "mib/s" => 8.0 * 1024.0_f64.powi(2),
        "gib/s" => 8.0 * 1024.0_f64.powi(3),
        _ => return None,
    };
    Some(val * bps_per)
}

// ---------------------------------------------------------------------------
// 4. Running / Cycling Pace Conversions
// ---------------------------------------------------------------------------

fn eval_pace(raw: &str) -> Option<CalcResult> {
    let lower = raw.to_ascii_lowercase();
    let (lhs, rhs) = split_connector(&lower)?;

    // Pace -> Speed or Pace: `5:00 min/km in km/h`, `13:24 min/mile in mph`
    if let Some((pace_mins, is_mile)) = parse_pace_expr(lhs) {
        let km_per_unit = if is_mile { 1.609_344 } else { 1.0 };
        let kmh = (60.0 / pace_mins) * km_per_unit;
        let mph = kmh / 1.609_344;
        let (display, copy_text, target_badge) = match rhs {
            "km/h" | "kph" | "kmh" => {
                let t = Formatter::copy_text(kmh);
                (
                    format!("{} km/h", Formatter::grouped(&t)),
                    format!("{t} km/h"),
                    "Speed",
                )
            }
            "mph" | "mi/h" => {
                let t = Formatter::copy_text(mph);
                (
                    format!("{} mph", Formatter::grouped(&t)),
                    format!("{t} mph"),
                    "Speed",
                )
            }
            "min/km" => {
                let mk = 60.0 / kmh;
                let s = format_pace_mmss(mk, "min/km");
                (s.clone(), s, "Pace")
            }
            "min/mile" | "min/mi" => {
                let mm = 60.0 / mph;
                let s = format_pace_mmss(mm, "min/mi");
                (s.clone(), s, "Pace")
            }
            _ => return None,
        };
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "Pace",
            target_badge,
            Payload::Value { display, copy_text },
        ));
    }

    // Speed -> Pace: `10 km/h in min/km`, `6 mph in min/mile`
    if matches!(rhs, "min/km" | "min/mile" | "min/mi") {
        let (speed, unit) = parse_qty_unit(lhs)?;
        if speed <= 0.0 {
            return None;
        }
        let kmh = match unit {
            "km/h" | "kph" | "kmh" => speed,
            "mph" | "mi/h" => speed * 1.609_344,
            "m/s" => speed * 3.6,
            _ => return None,
        };
        let (pace_mins, label) = if rhs == "min/km" {
            (60.0 / kmh, "min/km")
        } else {
            (60.0 / (kmh / 1.609_344), "min/mi")
        };
        let s = format_pace_mmss(pace_mins, label);
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "Speed",
            "Pace",
            Payload::text(&s),
        ));
    }

    None
}

fn parse_pace_expr(s: &str) -> Option<(f64, bool)> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let (m_str, s_str) = parts[0].split_once(':')?;
    let mins = m_str.parse::<f64>().ok()?;
    let secs = s_str.parse::<f64>().ok()?;
    if mins < 0.0 || !(0.0..60.0).contains(&secs) {
        return None;
    }
    let total = mins + secs / 60.0;
    if total <= 0.0 {
        return None;
    }
    let is_mile = match parts[1] {
        "min/km" | "/km" => false,
        "min/mile" | "min/mi" | "/mi" | "/mile" => true,
        _ => return None,
    };
    Some((total, is_mile))
}

fn format_pace_mmss(total_mins: f64, suffix: &str) -> String {
    let total_secs = (total_mins * 60.0).round() as u64;
    let m = total_secs / 60;
    let s = total_secs % 60;
    format!("{m}:{s:02} {suffix}")
}

// ---------------------------------------------------------------------------
// 5. Musical Pitch <-> Frequency (12-TET, A4 = 440 Hz)
// ---------------------------------------------------------------------------

fn eval_pitch(raw: &str) -> Option<CalcResult> {
    let lower = raw.to_ascii_lowercase();
    let (lhs, rhs) = split_connector(&lower)?;

    if matches!(rhs, "hz" | "hertz") {
        let (canonical, midi) = parse_musical_note(lhs)?;
        let freq = 440.0 * 2.0_f64.powf((midi as f64 - 69.0) / 12.0);
        let text = Formatter::copy_text(freq);
        return Some(CalcResult::with_badges(
            canonical,
            "Pitch",
            "Frequency",
            Payload::Value {
                display: format!("{} Hz", Formatter::grouped(&text)),
                copy_text: format!("{text} Hz"),
            },
        ));
    }

    if matches!(rhs, "note" | "pitch") {
        let (freq, unit) = parse_qty_unit(lhs)?;
        if !matches!(unit, "hz" | "hertz") || freq <= 0.0 {
            return None;
        }
        let midi_f = 69.0 + 12.0 * (freq / 440.0).log2();
        let midi = midi_f.round() as i32;
        if !(0..=127).contains(&midi) {
            return None;
        }
        const NAMES: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let note_name = NAMES[(midi.rem_euclid(12)) as usize];
        let octave = (midi / 12) - 1;
        let out = format!("{note_name}{octave}");
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "Frequency",
            "Pitch",
            Payload::text(&out),
        ));
    }
    None
}

fn parse_musical_note(s: &str) -> Option<(String, i32)> {
    let chars: Vec<char> = s.trim().chars().collect();
    if chars.len() < 2 || chars.len() > 4 {
        return None;
    }
    let letter = chars[0].to_ascii_uppercase();
    let base_semi: i32 = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut idx = 1usize;
    let mut acc_str = "";
    let acc: i32 = match chars[idx] {
        '#' | '♯' => {
            idx += 1;
            acc_str = "#";
            1
        }
        'b' | '♭' => {
            idx += 1;
            acc_str = "b";
            -1
        }
        _ => 0,
    };
    let oct_str: String = chars[idx..].iter().collect();
    let octave: i32 = oct_str.parse().ok()?;
    if !(-1..=9).contains(&octave) {
        return None;
    }
    let midi = (octave + 1) * 12 + base_semi + acc;
    Some((format!("{letter}{acc_str}{octave}"), midi))
}

// ---------------------------------------------------------------------------
// 6. Physical Constants & Constant Expressions
// ---------------------------------------------------------------------------

fn eval_physical_constants(raw: &str) -> Option<CalcResult> {
    let lower = raw
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    const C_MPS: f64 = 299_792_458.0;
    const AU_M: f64 = 149_597_870_700.0;

    // `<n> astronomical unit(s) / speed of light in <time_unit>`
    if lower.contains("astronomical unit")
        && lower.contains("speed of light")
        && lower.contains('/')
    {
        let (expr_part, target) = split_connector(&lower).unwrap_or((&lower, "minutes"));
        let (lhs, rhs) = expr_part.split_once('/')?;
        if rhs.trim() != "speed of light" {
            return None;
        }
        let mult_str = lhs
            .replace("astronomical units", "")
            .replace("astronomical unit", "");
        let mult = if mult_str.trim().is_empty() {
            1.0
        } else {
            mult_str.trim().parse::<f64>().ok()?
        };
        let seconds = (mult * AU_M) / C_MPS;
        let (val, sym) = match target {
            "s" | "sec" | "secs" | "second" | "seconds" => (seconds, "s"),
            "min" | "mins" | "minute" | "minutes" => (seconds / 60.0, "min"),
            "h" | "hr" | "hrs" | "hour" | "hours" => (seconds / 3600.0, "hr"),
            _ => return None,
        };
        let t = Formatter::copy_text(val);
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "Physics",
            "Light Time",
            Payload::Value {
                display: format!("{} {sym}", Formatter::grouped(&t)),
                copy_text: format!("{t} {sym}"),
            },
        ));
    }

    // `speed of light [in <speed_unit>]`
    if lower == "speed of light" {
        return Some(CalcResult::with_badges(
            "Speed of Light".to_string(),
            "Constant",
            "m/s",
            Payload::Value {
                display: "299,792,458 m/s".to_string(),
                copy_text: "299792458 m/s".to_string(),
            },
        ));
    }
    if let Some((lhs, rhs)) = split_connector(&lower) {
        if lhs == "speed of light" {
            let (val, sym) = match rhs {
                "m/s" => (C_MPS, "m/s"),
                "km/s" => (C_MPS / 1000.0, "km/s"),
                "km/h" | "kph" => (C_MPS * 3.6, "km/h"),
                "mph" | "mi/h" => (C_MPS * 2.236_936_292_054_4, "mph"),
                "mi/s" | "miles/s" => (C_MPS / 1609.344, "mi/s"),
                _ => return None,
            };
            let t = Formatter::copy_text(val);
            return Some(CalcResult::with_badges(
                "Speed of Light".to_string(),
                "Constant",
                sym,
                Payload::Value {
                    display: format!("{} {sym}", Formatter::grouped(&t)),
                    copy_text: format!("{t} {sym}"),
                },
            ));
        }
    }

    match lower.as_str() {
        "planck constant" | "plancks constant" => Some(CalcResult::with_badges(
            raw.to_string(),
            "Constant",
            "J·s",
            Payload::text("6.62607015e-34 J·s"),
        )),
        "avogadro" | "avogadro constant" | "avogadro number" => Some(CalcResult::with_badges(
            raw.to_string(),
            "Constant",
            "1/mol",
            Payload::text("6.02214076e23 /mol"),
        )),
        "standard gravity" => Some(CalcResult::with_badges(
            raw.to_string(),
            "Constant",
            "m/s²",
            Payload::text("9.80665 m/s²"),
        )),
        "golden ratio" => Some(CalcResult::with_badges(
            raw.to_string(),
            "Constant",
            "φ",
            Payload::number(1.618_033_988_749_895),
        )),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// 7. CSS Units, PPI / Print Pixel Density, and Aspect Ratio
// ---------------------------------------------------------------------------

fn eval_pixel_density_and_css(raw: &str, ctx: &CalcContext) -> Option<CalcResult> {
    let lower = raw
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Aspect ratio: `1920x1080 aspect ratio` / `aspect ratio of 1920x1080`
    if lower.contains("aspect ratio") {
        let cleaned = lower
            .replace("aspect ratio of", "")
            .replace("aspect ratio", "");
        let cleaned = cleaned.trim();
        let (ws, hs) = cleaned
            .split_once('x')
            .or_else(|| cleaned.split_once('×'))
            .or_else(|| cleaned.split_once(' '))?;
        let w = ws.trim().parse::<u64>().ok()?;
        let h = hs.trim().parse::<u64>().ok()?;
        if w == 0 || h == 0 {
            return None;
        }
        let g = gcd_u64(w, h);
        let ratio = format!("{}:{}", w / g, h / g);
        let mut card = CalcResult::with_badges(
            format!("{w}×{h}"),
            "Resolution",
            "Aspect Ratio",
            Payload::text(&ratio),
        );
        card.can_chain = false;
        return Some(card);
    }

    // Physical <-> Pixels at PPI/DPI: `2 inches in px at 72 ppi`, `300 px in inches at 300 dpi`
    if let Some((conv_part, ppi_part)) =
        lower.split_once(" at ").or_else(|| lower.split_once(" @ "))
    {
        let (ppi, ppi_u) = parse_qty_unit(ppi_part.trim())?;
        if !matches!(ppi_u, "ppi" | "dpi") || ppi <= 0.0 {
            return None;
        }
        let (lhs, rhs) = split_connector(conv_part.trim())?;
        let (val, src_u) = parse_qty_unit(lhs)?;
        if matches!(rhs, "px" | "pixel" | "pixels") {
            let inches = physical_to_inches(val, src_u)?;
            let px = inches * ppi;
            let t = Formatter::copy_text(px);
            return Some(CalcResult::with_badges(
                raw.to_string(),
                &format!("{} {ppi_u}", Formatter::display(ppi)),
                "Pixels",
                Payload::Value {
                    display: format!("{}px", Formatter::grouped(&t)),
                    copy_text: format!("{t}px"),
                },
            ));
        }
        if matches!(src_u, "px" | "pixel" | "pixels") {
            let inches = val / ppi;
            let (out, sym) = inches_to_physical(inches, rhs)?;
            let t = Formatter::copy_text(out);
            return Some(CalcResult::with_badges(
                raw.to_string(),
                &format!("{} {ppi_u}", Formatter::display(ppi)),
                sym,
                Payload::Value {
                    display: format!("{} {sym}", Formatter::grouped(&t)),
                    copy_text: format!("{t} {sym}"),
                },
            ));
        }
    }

    // CSS `rem` / `em` <-> `px` using `ctx.rem_px` (default 16px).
    if let Some((lhs, rhs)) = split_connector(&lower) {
        let (val, src_u) = parse_qty_unit(lhs)?;
        if matches!(src_u, "rem" | "em") && matches!(rhs, "px" | "pixel" | "pixels") {
            let px = val * ctx.rem_px;
            let t = Formatter::copy_text(px);
            return Some(CalcResult::with_badges(
                raw.to_string(),
                src_u,
                "Pixels",
                Payload::Value {
                    display: format!("{}px", Formatter::grouped(&t)),
                    copy_text: format!("{t}px"),
                },
            ));
        }
        if matches!(src_u, "px" | "pixel" | "pixels") && matches!(rhs, "rem" | "em") {
            let out = val / ctx.rem_px;
            let t = Formatter::copy_text(out);
            return Some(CalcResult::with_badges(
                raw.to_string(),
                "Pixels",
                rhs,
                Payload::Value {
                    display: format!("{}{rhs}", Formatter::grouped(&t)),
                    copy_text: format!("{t}{rhs}"),
                },
            ));
        }
    }

    None
}

fn physical_to_inches(val: f64, unit: &str) -> Option<f64> {
    Some(match unit {
        "in" | "inch" | "inches" => val,
        "cm" | "centimeter" | "centimeters" => val / 2.54,
        "mm" | "millimeter" | "millimeters" => val / 25.4,
        "pt" | "point" | "points" => val / 72.0,
        _ => return None,
    })
}

fn inches_to_physical(inches: f64, unit: &str) -> Option<(f64, &'static str)> {
    Some(match unit {
        "in" | "inch" | "inches" => (inches, "in"),
        "cm" | "centimeter" | "centimeters" => (inches * 2.54, "cm"),
        "mm" | "millimeter" | "millimeters" => (inches * 25.4, "mm"),
        "pt" | "point" | "points" => (inches * 72.0, "pt"),
        _ => return None,
    })
}

fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

// ---------------------------------------------------------------------------
// 8. Height Prime / Double-Prime Notation (`6'2" to m`, `6′2″ to m`, `5'10"`)
// ---------------------------------------------------------------------------

fn eval_prime_height(raw: &str) -> Option<CalcResult> {
    let lower = raw.to_ascii_lowercase();
    let (lhs, target) = split_connector(&lower).unwrap_or((&lower, "m"));
    let lhs = lhs.trim();

    let prime_idx = lhs.find(['\'', '′'])?;
    let feet = lhs[..prime_idx].trim().parse::<f64>().ok()?;
    let after = lhs[prime_idx + lhs[prime_idx..].chars().next()?.len_utf8()..].trim();
    let inches_str = after.trim_end_matches(['"', '″']).trim();
    let inches = if inches_str.is_empty() {
        0.0
    } else {
        inches_str.parse::<f64>().ok()?
    };

    let meters = feet * 0.3048 + inches * 0.0254;
    let (val, sym, badge) = match target {
        "m" | "meter" | "meters" | "metre" | "metres" => (meters, "m", "Meters"),
        "cm" | "centimeter" | "centimeters" => (meters * 100.0, "cm", "Centimeters"),
        "mm" | "millimeter" | "millimeters" => (meters * 1000.0, "mm", "Millimeters"),
        "in" | "inch" | "inches" => (meters / 0.0254, "in", "Inches"),
        "ft" | "foot" | "feet" => (meters / 0.3048, "ft", "Feet"),
        _ => return None,
    };
    let t = Formatter::copy_text(val);
    Some(CalcResult::with_badges(
        lhs.to_string(),
        "Height",
        badge,
        Payload::Value {
            display: format!("{} {sym}", Formatter::grouped(&t)),
            copy_text: format!("{t} {sym}"),
        },
    ))
}

// ---------------------------------------------------------------------------
// 9. Cooking Ingredient Mass <-> Volume
// ---------------------------------------------------------------------------

fn eval_cooking(raw: &str) -> Option<CalcResult> {
    let lower = raw
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let (lhs, rhs) = split_connector(&lower)?;

    // Parse `<qty> <unit> [of] <ingredient>`
    let parts: Vec<&str> = lhs.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let (qty, src_unit, ing_start) = if let Ok(v) = parts[0].parse::<f64>() {
        if parts.len() < 3 {
            return None;
        }
        (v, parts[1], 2usize)
    } else if let Some((v, u)) = split_attached(parts[0]) {
        (v, u, 1usize)
    } else {
        return None;
    };

    let mut ing_tokens = &parts[ing_start..];
    if ing_tokens.first() == Some(&"of") {
        ing_tokens = &ing_tokens[1..];
    }
    if ing_tokens.is_empty() {
        return None;
    }
    let ing_name = ing_tokens.join(" ");
    let (density_g_per_ml, label) = cooking_density(&ing_name)?;

    // Volume -> Mass
    if let Some(src_ml) = cooking_volume_ml(src_unit) {
        if let Some((dst_g, dst_sym)) = cooking_mass_grams(rhs) {
            let grams = qty * src_ml * density_g_per_ml;
            let out = grams / dst_g;
            let t = Formatter::copy_text(out);
            return Some(CalcResult::with_badges(
                raw.to_string(),
                label,
                dst_sym,
                Payload::Value {
                    display: format!("{} {dst_sym}", Formatter::grouped(&t)),
                    copy_text: format!("{t} {dst_sym}"),
                },
            ));
        }
    }

    // Mass -> Volume
    if let Some((src_g, _)) = cooking_mass_grams(src_unit) {
        if let Some(dst_ml) = cooking_volume_ml(rhs) {
            let ml = (qty * src_g) / density_g_per_ml;
            let out = ml / dst_ml;
            let sym = cooking_volume_symbol(rhs);
            let t = Formatter::copy_text(out);
            return Some(CalcResult::with_badges(
                raw.to_string(),
                label,
                sym,
                Payload::Value {
                    display: format!("{} {sym}", Formatter::grouped(&t)),
                    copy_text: format!("{t} {sym}"),
                },
            ));
        }
    }

    None
}

fn cooking_density(name: &str) -> Option<(f64, &'static str)> {
    // Standard culinary densities (g/mL).
    Some(match name {
        "water" => (1.0, "Water"),
        "milk" => (1.03, "Milk"),
        "flour" | "all purpose flour" | "all-purpose flour" | "wheat flour" => (0.593, "Flour"),
        "bread flour" => (0.537, "Bread Flour"),
        "sugar" | "white sugar" | "granulated sugar" => (0.845, "Sugar"),
        "brown sugar" => (0.93, "Brown Sugar"),
        "powdered sugar" | "icing sugar" => (0.507, "Powdered Sugar"),
        "butter" => (0.959, "Butter"),
        "oil" | "olive oil" | "vegetable oil" => (0.916, "Oil"),
        "honey" => (1.42, "Honey"),
        "salt" | "table salt" => (1.217, "Salt"),
        "oats" | "rolled oats" => (0.38, "Oats"),
        "rice" | "white rice" => (0.782, "Rice"),
        "cocoa" | "cocoa powder" => (0.423, "Cocoa Powder"),
        "baking powder" => (0.90, "Baking Powder"),
        "baking soda" => (0.93, "Baking Soda"),
        _ => return None,
    })
}

fn cooking_volume_ml(u: &str) -> Option<f64> {
    Some(match u {
        "ml" | "milliliter" | "milliliters" => 1.0,
        "l" | "liter" | "liters" => 1000.0,
        "tsp" | "teaspoon" | "teaspoons" => 4.928_921_593_75,
        "tbsp" | "tablespoon" | "tablespoons" => 14.786_764_781_25,
        "cup" | "cups" => 236.588_236_5,
        "fl oz" | "floz" => 29.573_529_562_5,
        _ => return None,
    })
}

fn cooking_volume_symbol(u: &str) -> &'static str {
    match u {
        "ml" | "milliliter" | "milliliters" => "mL",
        "l" | "liter" | "liters" => "L",
        "tsp" | "teaspoon" | "teaspoons" => "tsp",
        "tbsp" | "tablespoon" | "tablespoons" => "tbsp",
        "cup" | "cups" => "cups",
        _ => "mL",
    }
}

fn cooking_mass_grams(u: &str) -> Option<(f64, &'static str)> {
    Some(match u {
        "g" | "gram" | "grams" => (1.0, "g"),
        "kg" | "kilogram" | "kilograms" => (1000.0, "kg"),
        "oz" | "ounce" | "ounces" => (28.349_523_125, "oz"),
        "lb" | "lbs" | "pound" | "pounds" => (453.592_37, "lb"),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// 10. Minor Currency Units (`$12 in cents`, `£5 in pence`, `€2.50 in euro cents`)
// ---------------------------------------------------------------------------

fn eval_minor_currency(raw: &str) -> Option<CalcResult> {
    let lower = raw
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let (lhs, rhs) = split_connector(&lower)?;

    // Major -> Minor (`$12 in cents`, `12 usd in cents`, `£5 in pence`, `€2.50 in euro cents`, `₹10 in paise`)
    if matches!(
        rhs,
        "cent" | "cents" | "euro cent" | "euro cents" | "pence" | "penny" | "paise" | "paisa"
    ) {
        let amt = parse_major_currency_amount(lhs)?;
        let minor = amt * 100.0;
        let unit = match rhs {
            "pence" | "penny" => "pence",
            "paise" | "paisa" => "paise",
            _ => "cents",
        };
        let t = Formatter::copy_text(minor);
        let mut card = CalcResult::with_badges(
            raw.to_string(),
            "Currency",
            "Minor Unit",
            Payload::Value {
                display: format!("{} {unit}", Formatter::grouped(&t)),
                copy_text: format!("{t} {unit}"),
            },
        );
        card.is_currency = true;
        return Some(card);
    }

    // Minor -> Major (`1200 cents in usd`, `500 pence in gbp`)
    if let Some((amt, unit)) = parse_qty_unit(lhs) {
        if matches!(unit, "cent" | "cents" | "pence" | "paise") {
            let code = match rhs {
                "usd" | "dollar" | "dollars" | "$" => "USD",
                "eur" | "euro" | "euros" | "€" => "EUR",
                "gbp" | "pound" | "pounds" | "£" => "GBP",
                "inr" | "rupee" | "rupees" | "₹" => "INR",
                _ => return None,
            };
            let major = amt / 100.0;
            let mut card = CalcResult::with_badges(
                raw.to_string(),
                "Minor Unit",
                code,
                Payload::money(major, code),
            );
            card.is_currency = true;
            return Some(card);
        }
    }

    None
}

fn parse_major_currency_amount(s: &str) -> Option<f64> {
    let t = s.trim();
    for sym in ['$', '£', '€', '₹'] {
        if let Some(rest) = t.strip_prefix(sym) {
            return rest.trim().replace(',', "").parse::<f64>().ok();
        }
    }
    let (v, u) = parse_qty_unit(t)?;
    if matches!(
        u,
        "usd"
            | "dollar"
            | "dollars"
            | "eur"
            | "euro"
            | "euros"
            | "gbp"
            | "pound"
            | "pounds"
            | "inr"
            | "rupee"
            | "rupees"
    ) {
        Some(v)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// 11. Fraction Forms (`1.75 as improper`, `7/4 as mixed`, `1.75 as fraction`)
// ---------------------------------------------------------------------------

fn eval_fraction_forms(raw: &str) -> Option<CalcResult> {
    let lower = raw
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let (lhs, rhs) = split_connector(&lower)?;
    if !matches!(
        rhs,
        "improper" | "improper fraction" | "mixed" | "mixed number" | "mixed fraction"
    ) {
        return None;
    }

    let val = parse_simple_number_or_ratio(lhs)?;
    let (num, den) = approximate_fraction(val.abs(), 1000)?;
    let sign = if val < 0.0 { "-" } else { "" };

    let out = if rhs.starts_with("improper") {
        format!("{sign}{num}/{den}")
    } else {
        let whole = num / den;
        let rem = num % den;
        if rem == 0 {
            format!("{sign}{whole}")
        } else if whole == 0 {
            format!("{sign}{}", format_vulgar_or_slash(rem, den))
        } else {
            format!("{sign}{whole} {}", format_vulgar_or_slash(rem, den))
        }
    };

    Some(CalcResult::with_badges(
        raw.to_string(),
        "Number",
        if rhs.starts_with("improper") {
            "Improper Fraction"
        } else {
            "Mixed Number"
        },
        Payload::text(&out),
    ))
}

fn parse_simple_number_or_ratio(s: &str) -> Option<f64> {
    if let Some((a, b)) = s.split_once('/') {
        let n = a.trim().parse::<f64>().ok()?;
        let d = b.trim().parse::<f64>().ok()?;
        if d == 0.0 {
            return None;
        }
        return Some(n / d);
    }
    s.trim().parse::<f64>().ok()
}

fn approximate_fraction(x: f64, max_den: u64) -> Option<(u64, u64)> {
    if !x.is_finite() {
        return None;
    }
    let mut best_n = x.round() as u64;
    let mut best_d = 1u64;
    let mut best_err = (x - best_n as f64).abs();
    for d in 1..=max_den {
        let n = (x * d as f64).round() as u64;
        let err = (x - n as f64 / d as f64).abs();
        if err < best_err - 1e-12 {
            best_err = err;
            best_n = n;
            best_d = d;
            if best_err < 1e-9 {
                break;
            }
        }
    }
    let g = gcd_u64(best_n, best_d);
    Some((best_n / g, best_d / g))
}

fn format_vulgar_or_slash(n: u64, d: u64) -> String {
    match (n, d) {
        (1, 2) => "½".to_string(),
        (1, 3) => "⅓".to_string(),
        (2, 3) => "⅔".to_string(),
        (1, 4) => "¼".to_string(),
        (3, 4) => "¾".to_string(),
        (1, 5) => "⅕".to_string(),
        (2, 5) => "⅖".to_string(),
        (3, 5) => "⅗".to_string(),
        (4, 5) => "⅘".to_string(),
        (1, 6) => "⅙".to_string(),
        (5, 6) => "⅚".to_string(),
        (1, 8) => "⅛".to_string(),
        (3, 8) => "⅜".to_string(),
        (5, 8) => "⅝".to_string(),
        (7, 8) => "⅞".to_string(),
        _ => format!("{n}/{d}"),
    }
}

// ---------------------------------------------------------------------------
// 12. Specialized Units (`dBm`, `Rankine`, `chain`, `fortnight`, `nibble`, `workdays`, `L/s -> gpm`)
// ---------------------------------------------------------------------------

fn eval_specialized_units(raw: &str) -> Option<CalcResult> {
    let lower = raw
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let (lhs, rhs) = split_connector(&lower)?;
    let (val, src) = parse_qty_unit(lhs)?;

    // dBm <-> Watts / mW (logarithmic power scale: P_W = 10^((dBm - 30) / 10))
    if src == "dbm" {
        let watts = 10.0_f64.powf((val - 30.0) / 10.0);
        let (out, sym) = match rhs {
            "w" | "watt" | "watts" => (watts, "W"),
            "mw" | "milliwatt" | "milliwatts" => (watts * 1000.0, "mW"),
            _ => return None,
        };
        let t = Formatter::copy_text(out);
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "dBm",
            sym,
            Payload::Value {
                display: format!("{} {sym}", Formatter::grouped(&t)),
                copy_text: format!("{t} {sym}"),
            },
        ));
    }
    if rhs == "dbm"
        && matches!(
            src,
            "w" | "watt" | "watts" | "mw" | "milliwatt" | "milliwatts"
        )
    {
        let watts = if src.starts_with("mw") || src.starts_with("milli") {
            val / 1000.0
        } else {
            val
        };
        if watts <= 0.0 {
            return None;
        }
        let dbm = 10.0 * watts.log10() + 30.0;
        let t = Formatter::copy_text(dbm);
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "Power",
            "dBm",
            Payload::Value {
                display: format!("{} dBm", Formatter::grouped(&t)),
                copy_text: format!("{t} dBm"),
            },
        ));
    }

    // Rankine <-> Celsius / Fahrenheit / Kelvin
    if matches!(src, "r" | "rankine" | "°r") || matches!(rhs, "r" | "rankine" | "°r") {
        let kelvin = match src {
            "r" | "rankine" | "°r" => val * 5.0 / 9.0,
            "c" | "celsius" | "°c" => val + 273.15,
            "f" | "fahrenheit" | "°f" => (val + 459.67) * 5.0 / 9.0,
            "k" | "kelvin" => val,
            _ => return None,
        };
        let (out, sym) = match rhs {
            "r" | "rankine" | "°r" => (kelvin * 9.0 / 5.0, "°R"),
            "c" | "celsius" | "°c" => (kelvin - 273.15, "°C"),
            "f" | "fahrenheit" | "°f" => (kelvin * 9.0 / 5.0 - 459.67, "°F"),
            "k" | "kelvin" => (kelvin, "K"),
            _ => return None,
        };
        let clean = if out.abs() < 1e-9 { 0.0 } else { out };
        let t = Formatter::copy_text(clean);
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "Temperature",
            sym,
            Payload::Value {
                display: format!("{} {sym}", Formatter::grouped(&t)),
                copy_text: format!("{t} {sym}"),
            },
        ));
    }

    // Surveying length: `chain` (22 yd), `furlong` (220 yd)
    if matches!(src, "chain" | "chains" | "furlong" | "furlongs")
        || matches!(rhs, "chain" | "chains" | "furlong" | "furlongs")
    {
        let meters = match src {
            "chain" | "chains" => val * 20.1168,
            "furlong" | "furlongs" => val * 201.168,
            "yd" | "yard" | "yards" => val * 0.9144,
            "ft" | "foot" | "feet" => val * 0.3048,
            "m" | "meter" | "meters" => val,
            "mi" | "mile" | "miles" => val * 1609.344,
            _ => return None,
        };
        let (out, sym) = match rhs {
            "chain" | "chains" => (meters / 20.1168, "ch"),
            "furlong" | "furlongs" => (meters / 201.168, "fur"),
            "yd" | "yard" | "yards" => (meters / 0.9144, "yd"),
            "ft" | "foot" | "feet" => (meters / 0.3048, "ft"),
            "m" | "meter" | "meters" => (meters, "m"),
            "mi" | "mile" | "miles" => (meters / 1609.344, "mi"),
            _ => return None,
        };
        let t = Formatter::copy_text(out);
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "Length",
            sym,
            Payload::Value {
                display: format!("{} {sym}", Formatter::grouped(&t)),
                copy_text: format!("{t} {sym}"),
            },
        ));
    }

    // `fortnight` (14 days) and `workday` (8 hours)
    if matches!(
        src,
        "fortnight" | "fortnights" | "workday" | "workdays" | "workweek" | "workweeks"
    ) || matches!(
        rhs,
        "fortnight" | "fortnights" | "workday" | "workdays" | "workweek" | "workweeks"
    ) {
        let hours = match src {
            "h" | "hr" | "hrs" | "hour" | "hours" => val,
            "d" | "day" | "days" => val * 24.0,
            "w" | "wk" | "wks" | "week" | "weeks" => val * 168.0,
            "fortnight" | "fortnights" => val * 336.0,
            "workday" | "workdays" => val * 8.0,
            "workweek" | "workweeks" => val * 40.0,
            _ => return None,
        };
        let (out, sym) = match rhs {
            "h" | "hr" | "hrs" | "hour" | "hours" => (hours, "hours"),
            "d" | "day" | "days" => (hours / 24.0, "days"),
            "w" | "wk" | "wks" | "week" | "weeks" => (hours / 168.0, "weeks"),
            "fortnight" | "fortnights" => (hours / 336.0, "fortnights"),
            "workday" | "workdays" => (hours / 8.0, "workdays"),
            "workweek" | "workweeks" => (hours / 40.0, "workweeks"),
            _ => return None,
        };
        let t = Formatter::copy_text(out);
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "Time",
            sym,
            Payload::Value {
                display: format!("{} {sym}", Formatter::grouped(&t)),
                copy_text: format!("{t} {sym}"),
            },
        ));
    }

    // `nibble` (4 bits)
    if matches!(src, "nibble" | "nibbles") || matches!(rhs, "nibble" | "nibbles") {
        let bits = match src {
            "nibble" | "nibbles" => val * 4.0,
            "b" | "bit" | "bits" => val,
            "byte" | "bytes" => val * 8.0,
            _ => return None,
        };
        let (out, sym) = match rhs {
            "nibble" | "nibbles" => (bits / 4.0, "nibbles"),
            "b" | "bit" | "bits" => (bits, "bit"),
            "byte" | "bytes" => (bits / 8.0, "B"),
            _ => return None,
        };
        let t = Formatter::copy_text(out);
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "Data",
            sym,
            Payload::Value {
                display: format!("{} {sym}", Formatter::grouped(&t)),
                copy_text: format!("{t} {sym}"),
            },
        ));
    }

    // Volumetric flow rate: `L/s` <-> `gpm` (US gallons per minute) <-> `L/min`
    if matches!(src, "l/s" | "l/min" | "gpm" | "cfm")
        && matches!(rhs, "l/s" | "l/min" | "gpm" | "cfm")
    {
        let lps = match src {
            "l/s" => val,
            "l/min" => val / 60.0,
            "gpm" => val * 0.063_090_196_4,
            "cfm" => val * 0.471_947_443_2,
            _ => return None,
        };
        let (out, sym) = match rhs {
            "l/s" => (lps, "L/s"),
            "l/min" => (lps * 60.0, "L/min"),
            "gpm" => (lps / 0.063_090_196_4, "gpm"),
            "cfm" => (lps / 0.471_947_443_2, "cfm"),
            _ => return None,
        };
        let t = Formatter::copy_text(out);
        return Some(CalcResult::with_badges(
            raw.to_string(),
            "Flow Rate",
            sym,
            Payload::Value {
                display: format!("{} {sym}", Formatter::grouped(&t)),
                copy_text: format!("{t} {sym}"),
            },
        ));
    }

    None
}

// ---------------------------------------------------------------------------
// Shared parsing helpers
// ---------------------------------------------------------------------------

fn split_connector(s: &str) -> Option<(&str, &str)> {
    for sep in [" to ", " in ", " into ", " as ", " -> "] {
        if let Some((l, r)) = s.rsplit_once(sep) {
            let l = l.trim();
            let r = r.trim();
            if !l.is_empty() && !r.is_empty() {
                return Some((l, r));
            }
        }
    }
    None
}

fn parse_qty_unit(s: &str) -> Option<(f64, &str)> {
    let t = s.trim();
    let parts: Vec<&str> = t.split_whitespace().collect();
    match parts.as_slice() {
        [num, unit] => Some((num.replace(',', "").parse::<f64>().ok()?, *unit)),
        [attached] => split_attached(attached),
        _ => None,
    }
}

fn split_attached(s: &str) -> Option<(f64, &str)> {
    let idx = s.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != ',')?;
    if idx == 0 {
        return None;
    }
    let val = s[..idx].replace(',', "").parse::<f64>().ok()?;
    let unit = s[idx..].trim();
    if unit.is_empty() {
        None
    } else {
        Some((val, unit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disp(q: &str) -> Option<String> {
        let card = evaluate(q, &CalcContext::default())?;
        match card.payload {
            Payload::Value { display, .. } => Some(display),
            Payload::Error { .. } => None,
        }
    }

    #[test]
    fn unwraps_natural_language_questions() {
        assert_eq!(unwrap_question("what is 15% of 240?"), Some("15% of 240"));
        assert_eq!(
            unwrap_question("How many days until Dec 25?"),
            Some("days until Dec 25")
        );
        assert_eq!(unwrap_question("Convert 10ft in m"), Some("10ft in m"));
        assert_eq!(unwrap_question("2+2"), None);
    }

    #[test]
    fn computes_transfer_time_pace_and_pitch() {
        assert_eq!(
            disp("transfer 1 TB at 100 Mbps").as_deref(),
            Some("22 hours 13 min 20 s")
        );
        assert_eq!(disp("5:00 min/km in km/h").as_deref(), Some("12 km/h"));
        assert!(disp("13:24 min/mile in mph").unwrap().starts_with("4.4776"));
        assert_eq!(disp("10 km/h in min/km").as_deref(), Some("6:00 min/km"));
        assert!(disp("c4 to hz").unwrap().starts_with("261.6255"));
        assert_eq!(disp("a4 to hz").as_deref(), Some("440 Hz"));
        assert_eq!(disp("440 hz to note").as_deref(), Some("A4"));
        assert_eq!(disp("261.63 hz to note").as_deref(), Some("C4"));
    }

    #[test]
    fn computes_chemistry_molar_mass_with_groups_and_hydrates() {
        assert_eq!(
            disp("molar mass of C6H12O6").as_deref(),
            Some("180.156 g/mol")
        );
        assert_eq!(disp("molar mass of H2O").as_deref(), Some("18.015 g/mol"));
        assert_eq!(
            disp("molar mass of Ca(OH)2").as_deref(),
            Some("74.092 g/mol")
        );
        assert_eq!(
            disp("molecular weight of NaCl").as_deref(),
            Some("58.44 g/mol")
        );
    }

    #[test]
    fn computes_physical_constants_css_ppi_aspect_ratio_and_heights() {
        assert_eq!(
            disp("speed of light in km/s").as_deref(),
            Some("299,792.458 km/s")
        );
        assert!(disp("1 astronomical unit / speed of light in minutes")
            .unwrap()
            .starts_with("8.3167"));
        assert_eq!(disp("2rem in px").as_deref(), Some("32px"));
        assert_eq!(disp("32px in rem").as_deref(), Some("2rem"));
        assert_eq!(disp("2 inches in px at 72 ppi").as_deref(), Some("144px"));
        assert_eq!(disp("1920x1080 aspect ratio").as_deref(), Some("16:9"));
        assert_eq!(disp("6'2\" to m").as_deref(), Some("1.8796 m"));
        assert_eq!(disp("6′2″ to m").as_deref(), Some("1.8796 m"));
    }

    #[test]
    fn computes_cooking_text_minor_currency_fractions_and_specialty_units() {
        assert!(disp("1 cup flour in grams").unwrap().starts_with("140.29"));
        assert_eq!(
            disp(r#"first 10 characters of "hello world""#).as_deref(),
            Some("hello worl")
        );
        assert_eq!(disp(r#"length of "hello world""#).as_deref(), Some("11"));
        assert_eq!(disp(r#"words in "one two three""#).as_deref(), Some("3"));
        assert_eq!(disp("$12 in cents").as_deref(), Some("1,200 cents"));
        assert_eq!(disp("£5 in pence").as_deref(), Some("500 pence"));
        assert_eq!(disp("1200 cents in usd").as_deref(), Some("12.00 USD"));
        assert_eq!(disp("1.75 as improper").as_deref(), Some("7/4"));
        assert_eq!(disp("7/4 as mixed").as_deref(), Some("1 ¾"));
        assert_eq!(disp("30 dBm in watts").as_deref(), Some("1 W"));
        assert_eq!(disp("491.67 rankine in celsius").as_deref(), Some("0 °C"));
        assert_eq!(disp("1 chain in yards").as_deref(), Some("22 yd"));
        assert_eq!(disp("1 fortnight in days").as_deref(), Some("14 days"));
        assert_eq!(disp("1 nibble in bits").as_deref(), Some("4 bit"));
        assert_eq!(disp("55h in workdays").as_deref(), Some("6.875 workdays"));
        assert!(disp("1 L/s in gpm").unwrap().starts_with("15.8503"));
    }
}
