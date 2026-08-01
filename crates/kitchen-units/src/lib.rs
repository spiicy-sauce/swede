//! `kitchen-units` — culinary unit parsing, conversion, and human-friendly
//! rollup.
//!
//! Standalone and dependency-free. Parse a quantity string, scale it, convert
//! between units of the same measurement system, and render it back the way a
//! recipe would (rolling small units up into larger ones: `48 tsp` → `1 cup`,
//! `1600 g` → `1.6 kg`), rounded to common cooking fractions.
//!
//! ```
//! use kitchen_units::Quantity;
//! let q = Quantity::parse("4 tsp").unwrap();
//! assert_eq!(q.humanize(), "1 1/3 tbsp");
//! assert_eq!(Quantity::parse("16 tbsp").unwrap().humanize(), "1 cup");
//! assert_eq!(Quantity::parse("0.5 cup").unwrap().scale(3.0).humanize(), "1 1/2 cups");
//! ```

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum System {
    UsVolume,
    MetricVolume,
    UsMass,
    MetricMass,
    Count,
    Length,
}

/// A unit of measure, with its size in the system's base unit (ml for volume,
/// g for mass, `each` for count, mm for length).
#[derive(Clone, Copy, Debug)]
pub struct Unit {
    pub symbol: &'static str,
    pub system: System,
    pub base: f64,
}

macro_rules! units {
    ($(($sym:literal, $sys:expr, $base:expr)),* $(,)?) => {
        &[$(Unit { symbol: $sym, system: $sys, base: $base }),*]
    };
}

/// The known units. Base: ml (volume), g (mass), each (count), mm (length).
pub const UNITS: &[Unit] = units![
    // exact integer multiples so 3 tsp == 1 tbsp and 48 tsp == 1 cup precisely
    ("tsp", System::UsVolume, 4.92892159375),
    ("tbsp", System::UsVolume, 14.78676478125),
    ("floz", System::UsVolume, 29.5735295625),
    ("cup", System::UsVolume, 236.5882365),
    ("pint", System::UsVolume, 473.176473),
    ("quart", System::UsVolume, 946.352946),
    ("gallon", System::UsVolume, 3785.411784),
    ("ml", System::MetricVolume, 1.0),
    ("dl", System::MetricVolume, 100.0),
    ("l", System::MetricVolume, 1000.0),
    ("mg", System::MetricMass, 0.001),
    ("g", System::MetricMass, 1.0),
    ("kg", System::MetricMass, 1000.0),
    // exact: 1 lb == 16 oz
    ("oz", System::UsMass, 28.349523125),
    ("lb", System::UsMass, 453.59237),
    ("each", System::Count, 1.0),
    ("mm", System::Length, 1.0),
    ("cm", System::Length, 10.0),
    ("in", System::Length, 25.4),
    ("ft", System::Length, 304.8),
];

fn find(symbol: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|u| u.symbol == symbol)
}

/// Resolve a unit symbol or alias to a canonical [`Unit`].
///
/// Case matters for the ambiguous one-letter forms (`T` = tbsp, `t` = tsp);
/// multi-character names also match case-insensitively.
pub fn lookup(sym: &str) -> Option<&'static Unit> {
    let s = sym.trim();
    let canonical = |s: &str| -> Option<&'static str> {
        Some(match s {
            "t" | "tsp" | "teaspoon" | "teaspoons" => "tsp",
            "T" | "tbsp" | "tbs" | "tablespoon" | "tablespoons" => "tbsp",
            "c" | "cup" | "cups" => "cup",
            "floz" | "fluid ounce" | "fluid ounces" => "floz",
            "pt" | "pint" | "pints" => "pint",
            "qt" | "quart" | "quarts" => "quart",
            "gal" | "gallon" | "gallons" => "gallon",
            "ml" | "mL" | "milliliter" | "milliliters" | "millilitre" | "millilitres" => "ml",
            "dl" => "dl",
            "l" | "L" | "liter" | "liters" | "litre" | "litres" => "l",
            "oz" | "ounce" | "ounces" => "oz",
            "lb" | "lbs" | "pound" | "pounds" => "lb",
            "mg" | "milligram" | "milligrams" => "mg",
            "g" | "gram" | "grams" => "g",
            "kg" | "kilogram" | "kilograms" => "kg",
            "mm" | "millimeter" | "millimeters" => "mm",
            "cm" | "centimeter" | "centimeters" => "cm",
            "in" | "inch" | "inches" | "\"" => "in",
            "ft" | "foot" | "feet" => "ft",
            "each" | "ea" | "" => "each",
            _ => return None,
        })
    };
    canonical(s)
        .or_else(|| if s.len() > 1 { canonical(&s.to_ascii_lowercase()) } else { None })
        .and_then(find)
}

/// A measured quantity: a value in a specific [`Unit`].
#[derive(Clone, Copy, Debug)]
pub struct Quantity {
    pub value: f64,
    pub unit: &'static Unit,
}

impl Quantity {
    pub fn new(value: f64, unit: &'static Unit) -> Self {
        Quantity { value, unit }
    }

    /// Parse `"2 tbsp"`, `"1 1/2 cups"`, `"500g"`, `".25 in"`, `"2T"`, or a bare
    /// number (`"8"` → 8 `each`).
    pub fn parse(s: &str) -> Option<Quantity> {
        let s = s.trim();
        let split = s.find(|c: char| c.is_ascii_alphabetic() || c == '"');
        let (num, unit_str) = match split {
            Some(0) => return None, // a bare unit with no number
            Some(i) => (s[..i].trim(), s[i..].trim()),
            None => (s, ""),
        };
        let value = parse_number(num)?;
        let unit = lookup(unit_str)?;
        Some(Quantity { value, unit })
    }

    /// The value in the system's base unit (ml / g / each / mm).
    pub fn base(&self) -> f64 {
        self.value * self.unit.base
    }

    pub fn scale(self, factor: f64) -> Quantity {
        Quantity { value: self.value * factor, unit: self.unit }
    }

    /// Convert to another unit in the same system.
    pub fn convert_to(&self, target: &'static Unit) -> Option<Quantity> {
        if target.system != self.unit.system {
            return None;
        }
        Some(Quantity { value: self.base() / target.base, unit: target })
    }

    /// Re-express in the "nicest" unit of the same system, rolling small units
    /// up into larger ones. Length has no rollup ladder, so it is returned as-is.
    pub fn normalize(&self) -> Quantity {
        let ladder = ladder(self.unit.system);
        if ladder.is_empty() {
            return *self;
        }
        let base = self.base();
        for (sym, threshold) in ladder {
            let u = find(sym).unwrap();
            let v = base / u.base;
            if v >= *threshold - 1e-6 && acceptable(sym, v, base) {
                return Quantity { value: v, unit: u };
            }
        }
        let (sym, _) = ladder.last().unwrap();
        let u = find(sym).unwrap();
        Quantity { value: base / u.base, unit: u }
    }

    /// A recipe-style rendering: normalized, with the value rounded to a common
    /// cooking fraction, e.g. `"1 1/2 cups"`, `"2 tsp"`, `"1.6 kg"`.
    pub fn humanize(&self) -> String {
        let n = self.normalize();
        let num = format_value(n.value, n.unit.system);
        let sym = pluralize(n.unit.symbol, n.value);
        if sym.is_empty() {
            num
        } else {
            format!("{num} {sym}")
        }
    }
}

/// Format a value for a system: metric reads as a decimal (`1.6`), US cooking
/// and count/length read as common fractions (`1 1/2`).
pub fn format_value(v: f64, system: System) -> String {
    match system {
        System::MetricVolume | System::MetricMass => format_decimal(v),
        _ => format_number(v),
    }
}

fn format_decimal(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Output ladders (largest → smallest) with a minimum "at least this many"
/// threshold for choosing a unit. `cup` at 0.25 lets `3/4 cup` show as cups;
/// `tsp`/`g`/`ml`/`oz` at 0 are the floor.
fn ladder(system: System) -> &'static [(&'static str, f64)] {
    match system {
        // Capped at cup: recipes stay in cups/tbsp/tsp. quart/gallon remain
        // parseable and convertible, just not rollup targets.
        System::UsVolume => &[("cup", 0.25), ("tbsp", 1.0), ("tsp", 0.0)],
        System::MetricVolume => &[("l", 1.0), ("ml", 0.0)],
        System::UsMass => &[("lb", 1.0), ("oz", 0.0)],
        System::MetricMass => &[("kg", 1.0), ("g", 0.0)],
        System::Count => &[("each", 0.0)],
        System::Length => &[],
    }
}

/// Whether a value reads well in the given unit. Rolling up to `cup` is only
/// allowed when the amount rounds to a standard cup measure (1/4, 1/3, 1/2,
/// 2/3, 3/4 of a cup); otherwise it stays in tbsp/tsp. Other units accept any
/// value once they clear the ladder threshold.
fn acceptable(sym: &str, v: f64, base: f64) -> bool {
    let allowed: &[f64] = match sym {
        "cup" => &[0.0, 0.25, 1.0 / 3.0, 0.5, 2.0 / 3.0, 0.75, 1.0],
        _ => return true,
    };
    let frac = v - v.floor();
    let nearest = allowed
        .iter()
        .cloned()
        .min_by(|a, b| (frac - a).abs().partial_cmp(&(frac - b).abs()).unwrap())
        .unwrap();
    let err = ((v.floor() + nearest) - v).abs() * find(sym).unwrap().base;
    err <= 0.03 * base + 1e-9
}

fn pluralize(symbol: &str, value: f64) -> String {
    match symbol {
        "each" => String::new(),
        "cup" | "pint" | "quart" | "gallon" if value > 1.0 + 1e-4 => format!("{symbol}s"),
        _ => symbol.to_string(),
    }
}

/// Nice cooking fractions (denominators 2, 3, 4, 8) plus whole numbers.
const FRACTIONS: &[(f64, &str)] = &[
    (0.0, ""),
    (0.125, "1/8"),
    (0.25, "1/4"),
    (1.0 / 3.0, "1/3"),
    (0.375, "3/8"),
    (0.5, "1/2"),
    (0.625, "5/8"),
    (2.0 / 3.0, "2/3"),
    (0.75, "3/4"),
    (0.875, "7/8"),
    (1.0, ""),
];

/// Format a value rounded to the nearest cooking fraction: `1.5` → `"1 1/2"`,
/// `0.75` → `"3/4"`, `2.0` → `"2"`, `4.0/3.0` → `"1 1/3"`.
pub fn format_number(v: f64) -> String {
    if v < 0.0 {
        return format!("-{}", format_number(-v));
    }
    let mut whole = v.floor();
    let frac = v - whole;
    let (best, mut label) = FRACTIONS
        .iter()
        .min_by(|a, b| (frac - a.0).abs().partial_cmp(&(frac - b.0).abs()).unwrap())
        .unwrap();
    if (*best - 1.0).abs() < 1e-9 {
        whole += 1.0;
        label = "";
    }
    let whole = whole as i64;
    match (whole, label.is_empty()) {
        (_, false) if whole == 0 => label.to_string(),
        (_, false) => format!("{whole} {label}"),
        (_, true) => whole.to_string(),
    }
}

/// Parse an int / decimal / fraction / mixed-fraction (`2`, `1.5`, `.5`, `1/2`,
/// `1 1/2`). Kept local so the crate stays dependency-free.
pub fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some((whole, frac)) = s.split_once(' ') {
        if let (Ok(w), Some(f)) = (whole.trim().parse::<f64>(), parse_fraction(frac.trim())) {
            return Some(w + f);
        }
    }
    if let Some(f) = parse_fraction(s) {
        return Some(f);
    }
    s.parse::<f64>().ok()
}

fn parse_fraction(s: &str) -> Option<f64> {
    let (n, d) = s.split_once('/')?;
    let n: f64 = n.trim().parse().ok()?;
    let d: f64 = d.trim().parse().ok()?;
    if d == 0.0 {
        return None;
    }
    Some(n / d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> String {
        Quantity::parse(s).unwrap().humanize()
    }

    #[test]
    fn parses_forms() {
        assert_eq!(Quantity::parse("2 tbsp").unwrap().unit.symbol, "tbsp");
        assert_eq!(Quantity::parse("2T").unwrap().unit.symbol, "tbsp");
        assert_eq!(Quantity::parse(".5t").unwrap().unit.symbol, "tsp");
        assert_eq!(Quantity::parse("1c").unwrap().unit.symbol, "cup");
        assert_eq!(Quantity::parse("500g").unwrap().value, 500.0);
        assert_eq!(Quantity::parse("1 1/2 cups").unwrap().value, 1.5);
        assert_eq!(Quantity::parse("8").unwrap().unit.symbol, "each");
    }

    #[test]
    fn case_sensitive_short_forms() {
        assert_eq!(Quantity::parse("1T").unwrap().unit.symbol, "tbsp");
        assert_eq!(Quantity::parse("1t").unwrap().unit.symbol, "tsp");
    }

    #[test]
    fn conversions() {
        let tsp = lookup("tsp").unwrap();
        let cup = lookup("cup").unwrap();
        assert!((Quantity::new(48.0, tsp).convert_to(cup).unwrap().value - 1.0).abs() < 1e-6);
    }

    #[test]
    fn volume_rollup() {
        assert_eq!(h("3 tsp"), "1 tbsp");
        assert_eq!(h("4 tsp"), "1 1/3 tbsp");
        assert_eq!(h("16 tbsp"), "1 cup");
        assert_eq!(h("48 tsp"), "1 cup");
        assert_eq!(h("2 tsp"), "2 tsp"); // stays: less than 1 tbsp
        assert_eq!(h("0.5 cup"), "1/2 cup");
        assert_eq!(h("0.75 cup"), "3/4 cup");
    }

    #[test]
    fn scaling_rolls_up() {
        assert_eq!(Quantity::parse("0.5 cup").unwrap().scale(3.0).humanize(), "1 1/2 cups");
        assert_eq!(Quantity::parse("1 tsp").unwrap().scale(4.0).humanize(), "1 1/3 tbsp");
        assert_eq!(Quantity::parse("2 tbsp").unwrap().scale(8.0).humanize(), "1 cup");
    }

    #[test]
    fn mass_rollup() {
        // metric formats as a decimal, not a fraction
        assert_eq!(h("1600 g"), "1.6 kg");
        assert_eq!(Quantity::parse("500 g").unwrap().scale(2.0).humanize(), "1 kg");
        assert_eq!(h("250 g"), "250 g");
        // US mass rolls oz -> lb with fractions
        assert_eq!(Quantity::parse("8 oz").unwrap().scale(2.0).humanize(), "1 lb");
        assert_eq!(h("1500 ml"), "1.5 l");
    }

    #[test]
    fn count_and_length() {
        assert_eq!(Quantity::parse("8").unwrap().scale(2.0).humanize(), "16");
        assert_eq!(Quantity::parse(".25 in").unwrap().scale(2.0).humanize(), "1/2 in");
    }

    #[test]
    fn format_number_fractions() {
        assert_eq!(format_number(1.5), "1 1/2");
        assert_eq!(format_number(0.75), "3/4");
        assert_eq!(format_number(2.0), "2");
        assert_eq!(format_number(4.0 / 3.0), "1 1/3");
    }
}
