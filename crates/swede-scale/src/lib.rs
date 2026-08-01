//! Scale a Swede recipe by a factor, source-to-source.
//!
//! Ingredient and sub-recipe amounts are multiplied and re-expressed with
//! `kitchen-units` rollup (`2 T` x 8 -> `1 c`); the `basis` amount is scaled in
//! its original unit; `%`/`%*` amounts are left untouched (they are relative, so
//! scaling the basis scales them); `serves`/`yields` numbers are scaled too.
//! Everything else passes through byte-for-byte, and a file with syntax errors
//! is returned unchanged.

use kitchen_units::{format_number, format_value, lookup, Quantity, System, Unit};
use swede_syntax::{parse, Amount, Arg, Call, Expr, File, Recipe, Span, Term};

struct Edit {
    span: Span,
    text: String,
}

/// Scale a recipe's source by `factor` (e.g. 2.0 doubles it).
pub fn scale_source(source: &str, factor: f64) -> String {
    let lowered = parse(source);
    if !lowered.errors.is_empty() {
        return source.to_string();
    }
    let Some(File::Recipe(r)) = &lowered.file else {
        return source.to_string(); // menus/unparseable: unchanged
    };

    let mut edits = Vec::new();
    collect_edits(r, factor, &mut edits);
    apply(source, edits)
}

/// The recipe's current serving count, parsed from `serves` (or `yields`).
/// A range like `3-4` yields its midpoint. `None` if there is no such number.
pub fn current_serves(source: &str) -> Option<f64> {
    let lowered = parse(source);
    let File::Recipe(r) = lowered.file? else {
        return None;
    };
    let meta = r.meta.iter().find(|m| m.key == "serves").or_else(|| r.meta.iter().find(|m| m.key == "yields"))?;
    let nums: Vec<f64> = number_spans(&meta.value).iter().map(|(_, _, v)| *v).collect();
    match nums.as_slice() {
        [] => None,
        [a] => Some(*a),
        [a, b, ..] => Some((a + b) / 2.0),
    }
}

fn collect_edits(r: &Recipe, factor: f64, edits: &mut Vec<Edit>) {
    // basis: scale in its original unit (no rollup)
    for b in &r.bases {
        if let Some(text) = scale_amount(&b.amount, factor, false) {
            edits.push(Edit { span: b.amount.span(), text });
        }
    }
    // ingredient / sub-recipe amounts: scale with rollup
    for binding in &r.bindings {
        walk_expr(&binding.value, factor, edits);
    }
    // serves / yields numbers
    for m in &r.meta {
        if m.key == "serves" || m.key == "yields" {
            let scaled = scale_numbers(&m.value, factor);
            if scaled != m.value {
                edits.push(Edit { span: m.span, text: format!(":{} {scaled}", m.key) });
            }
        }
    }
}

fn walk_expr(e: &Expr, factor: f64, edits: &mut Vec<Edit>) {
    match e {
        Expr::Call(c) => walk_call(c, factor, edits),
        Expr::SubRef(s) => {
            if let Some(a) = &s.amount {
                if let Some(text) = scale_amount(a, factor, true) {
                    edits.push(Edit { span: a.span(), text });
                }
            }
        }
        Expr::NodeRef(_) => {}
    }
}

fn walk_call(c: &Call, factor: f64, edits: &mut Vec<Edit>) {
    for arg in &c.args {
        match arg {
            Arg::Call(cc) => walk_call(cc, factor, edits),
            Arg::Ingredient(i) => {
                if let Some(text) = scale_amount(&i.amount, factor, true) {
                    edits.push(Edit { span: i.amount.span(), text });
                }
            }
            Arg::Equip(_) | Arg::NodeRef(_) => {}
        }
    }
}

fn system_floor(sys: System) -> &'static Unit {
    let sym = match sys {
        System::UsVolume => "tsp",
        System::MetricVolume => "ml",
        System::UsMass => "oz",
        System::MetricMass => "g",
        System::Count => "each",
        System::Length => "mm",
    };
    lookup(sym).unwrap()
}

/// Map a kitchen-units canonical symbol to Swede's canonical unit token.
fn swede_token(symbol: &str) -> &'static str {
    match symbol {
        "tsp" => "t",
        "tbsp" => "T",
        "cup" => "c",
        "pint" => "pt",
        "quart" => "qt",
        "gallon" => "gal",
        "each" => "",
        other => match other {
            "floz" => "floz",
            "ml" => "ml",
            "dl" => "dl",
            "l" => "l",
            "mg" => "mg",
            "g" => "g",
            "kg" => "kg",
            "oz" => "oz",
            "lb" => "lb",
            "mm" => "mm",
            "cm" => "cm",
            "in" => "in",
            "ft" => "ft",
            _ => "",
        },
    }
}

fn format_qty(q: &Quantity, roll_up: bool) -> String {
    let q = if roll_up { q.normalize() } else { *q };
    let num = format_value(q.value, q.unit.system);
    let tok = swede_token(q.unit.symbol);
    if tok.is_empty() {
        num
    } else {
        format!("{num} {tok}")
    }
}

/// A kitchen quantity for a scalable measure term (known unit, or a bare count).
fn measure_qty(t: &Term) -> Option<Quantity> {
    if let Term::Measure { value: Some(v), unit, .. } = t {
        let sym = unit.as_deref().unwrap_or("");
        if let Some(u) = lookup(sym) {
            return Some(Quantity::new(*v, u));
        }
    }
    None
}

/// Scale an amount's inner text; `None` when nothing is scalable (pure `%`,
/// to-taste, unquantified).
fn scale_amount(a: &Amount, factor: f64, roll_up: bool) -> Option<String> {
    let Amount::Quantity { terms, .. } = a else {
        return None; // ToTaste / Unquantified
    };
    let qtys: Vec<Option<Quantity>> = terms.iter().map(measure_qty).collect();

    let any_measure = terms.iter().any(|t| matches!(t, Term::Measure { .. }));
    if !any_measure {
        return None; // all basis-relative
    }

    // Fast path: 2+ measure terms, all mappable and in one system — sum them.
    // (A single term keeps its own unit via the general path below, so a length
    // like `.25 in` is not flattened into the system's floor unit.)
    let all_mapped =
        terms.len() >= 2 && terms.iter().zip(&qtys).all(|(t, q)| matches!(t, Term::Measure { .. }) && q.is_some());
    if all_mapped {
        let sys = qtys[0].as_ref().unwrap().unit.system;
        if qtys.iter().all(|q| q.as_ref().unwrap().unit.system == sys) {
            let total: f64 = qtys.iter().map(|q| q.as_ref().unwrap().base()).sum();
            let floor = system_floor(sys);
            let q = Quantity::new(total / floor.base, floor).scale(factor);
            return Some(format_qty(&q, roll_up));
        }
    }

    // General path: scale each term independently, keeping basis terms verbatim.
    let parts: Vec<String> = terms
        .iter()
        .zip(&qtys)
        .map(|(t, q)| match (t, q) {
            (Term::Measure { .. }, Some(q)) => format_qty(&q.scale(factor), roll_up),
            (Term::Measure { value: Some(v), unit, .. }, None) => {
                // known value, unrecognized unit: scale the number, keep the unit
                format!("{} {}", format_number(v * factor), unit.clone().unwrap_or_default()).trim().to_string()
            }
            (Term::Measure { raw, .. }, _) => raw.trim().to_string(), // e.g. `pinch`
            (Term::Basis { raw, .. }, _) => raw.trim().to_string(),
        })
        .collect();
    Some(parts.join(" + "))
}

/// Find `(start, len, value)` for each number token in a string.
fn number_spans(s: &str) -> Vec<(usize, usize, f64)> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_digit() || (c == '.' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit()) {
            let start = i;
            while i < bytes.len() {
                let d = bytes[i] as char;
                if d.is_ascii_digit() || d == '.' {
                    i += 1;
                } else {
                    break;
                }
            }
            if let Ok(v) = s[start..i].parse::<f64>() {
                out.push((start, i - start, v));
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Scale every number in a free-text string (e.g. `serves`), leaving the rest.
fn scale_numbers(s: &str, factor: f64) -> String {
    let spans = number_spans(s);
    if spans.is_empty() {
        return s.to_string();
    }
    let mut out = String::new();
    let mut cursor = 0;
    for (start, len, v) in spans {
        out.push_str(&s[cursor..start]);
        // servings are whole numbers
        out.push_str(&((v * factor).round() as i64).max(1).to_string());
        cursor = start + len;
    }
    out.push_str(&s[cursor..]);
    out
}

fn apply(source: &str, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|e| e.span.start_byte);
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0;
    for e in edits {
        if e.span.start_byte < cursor {
            continue;
        }
        out.push_str(&source[cursor..e.span.start_byte]);
        // Preserve leading whitespace of the replaced span. The unbracketed
        // `basis` amount grabs the space after `=`; bracketed amounts have none.
        let orig = &source[e.span.start_byte..e.span.end_byte];
        let lead = orig.len() - orig.trim_start().len();
        out.push_str(&orig[..lead]);
        out.push_str(&e.text);
        cursor = e.span.end_byte;
    }
    out.push_str(&source[cursor..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale(src: &str, f: f64) -> String {
        scale_source(src, f)
    }

    #[test]
    fn scales_measures_with_rollup() {
        let src = "recipe r\nx = mix(soy[2T], rice[1c], oil[1t])\nplate = f(x)\n";
        let out = scale(src, 8.0);
        // 2T*8 = 16T = 1 cup; 1c*8 = 8 cups; 1t*8 = 8t = 2 T + 2 t -> "2 2/3 T"
        assert!(out.contains("soy[1 c]"), "{out}");
        assert!(out.contains("rice[8 c]"), "{out}");
        assert!(out.contains("oil[2 2/3 T]"), "{out}");
    }

    #[test]
    fn leaves_percentages_but_scales_basis() {
        let src = "recipe loaf\n:serves 1\n\nbasis flour = 800g\nblend = mix(w[50%* flour], water[72% flour])\nplate = bake(blend)\n";
        let out = scale(src, 2.0);
        assert!(out.contains("basis flour = 1600 g"), "{out}");
        assert!(out.contains("w[50%* flour]"), "percentages untouched: {out}");
        assert!(out.contains("water[72% flour]"), "{out}");
        assert!(out.contains(":serves 2"), "{out}");
    }

    #[test]
    fn scales_compound_and_counts() {
        let src = "recipe r\nx = layer(broth[1c + 2T], scallions[2])\nplate = f(x)\n";
        let out = scale(src, 2.0);
        // 1c + 2T = 18T -> *2 = 36T = 2.25 cups
        assert!(out.contains("broth[2 1/4 c]"), "{out}");
        assert!(out.contains("scallions[4]"), "{out}");
    }

    #[test]
    fn to_taste_untouched_and_syntax_errors_passthrough() {
        let src = "recipe r\nx = mix(salt[~], pepper[?])\nplate = f(x)\n";
        assert_eq!(scale(src, 3.0), src);
        assert_eq!(scale("recipe r\nx = f(\n", 2.0), "recipe r\nx = f(\n");
    }

    #[test]
    fn scaled_fixtures_reparse_cleanly() {
        for f in ["miso_chicken_and_rice", "snow_pea_salad", "fifty_fifty_loaf"] {
            let path = format!("{}/../../fixtures/valid/{}.swede", env!("CARGO_MANIFEST_DIR"), f);
            let src = std::fs::read_to_string(&path).unwrap();
            for factor in [2.0, 0.5, 2.2857] {
                let out = scale(&src, factor);
                let errs = swede_syntax::parse(&out).errors;
                assert!(errs.is_empty(), "{f} x{factor} reparse errors: {errs:?}\n{out}");
            }
        }
    }

    #[test]
    fn current_serves_midpoint() {
        assert_eq!(current_serves("recipe r\n:serves 3-4\nx = f(a[1g])\n"), Some(3.5));
        assert_eq!(current_serves("recipe r\n:serves 4\nx = f(a[1g])\n"), Some(4.0));
    }
}
