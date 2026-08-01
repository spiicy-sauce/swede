//! Classification of the opaque `[...]` amount text and of timer/temp tokens
//! into the structured forms in [`crate::ast`].

use crate::ast::*;

/// Parse an int / decimal / fraction / mixed-fraction into an `f64`.
///
/// `2`, `1.5`, `.5`, `1/2`, `1 1/2` all supported.
pub fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Mixed fraction: "1 1/2"
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

/// Classify the raw text between `[` and `]`.
pub fn parse_amount(raw: &str, span: Span) -> Amount {
    let t = raw.trim();
    if t == "~" {
        return Amount::ToTaste { span };
    }
    if t == "?" {
        return Amount::Unquantified { span };
    }
    let terms = t.split('+').map(|p| parse_term(p.trim())).collect();
    Amount::Quantity {
        terms,
        raw: raw.to_string(),
        span,
    }
}

fn parse_term(s: &str) -> Term {
    if let Some(pi) = s.find('%') {
        let num = s[..pi].trim();
        let rest = s[pi + 1..].trim_start();
        let (constituent, basis) = match rest.strip_prefix('*') {
            Some(r) => (true, r.trim()),
            None => (false, rest.trim()),
        };
        Term::Basis {
            percent: parse_number(num),
            constituent,
            basis: basis.to_string(),
            raw: s.to_string(),
        }
    } else {
        match s.find(|c: char| c.is_ascii_alphabetic()) {
            // leading unit (bare unit like "pinch")
            Some(0) => Term::Measure {
                value: None,
                unit: Some(s.trim().to_string()),
                raw: s.to_string(),
            },
            // number then unit ("500g", ".25in")
            Some(i) => Term::Measure {
                value: parse_number(s[..i].trim()),
                unit: Some(s[i..].trim().to_string()),
                raw: s.to_string(),
            },
            // bare number ("8", "1 1/2")
            None => Term::Measure {
                value: parse_number(s.trim()),
                unit: None,
                raw: s.to_string(),
            },
        }
    }
}

/// Parse a `{350F}` / `{180C}` temperature token.
pub fn parse_temp(text: &str, span: Span) -> Option<Temp> {
    let inner = text.trim().strip_prefix('{')?.strip_suffix('}')?.trim();
    let (num, unit) = inner.split_at(inner.len().checked_sub(1)?);
    let unit = match unit {
        "F" | "f" => TempUnit::F,
        "C" | "c" => TempUnit::C,
        _ => return None,
    };
    Some(Temp {
        value: parse_number(num)?,
        unit,
        span,
    })
}

fn parse_time_unit(c: char) -> Option<TimeUnit> {
    match c {
        's' => Some(TimeUnit::S),
        'm' => Some(TimeUnit::M),
        'h' => Some(TimeUnit::H),
        'd' => Some(TimeUnit::D),
        _ => None,
    }
}

/// Parse the body of a timer (after the `@`/`~` sign): `6-10m`, `12m`, `2-3h`.
fn parse_duration_range(body: &str, span: Span) -> Option<DurationRange> {
    let body = body.trim();
    let unit_char = body.chars().last()?;
    let unit = parse_time_unit(unit_char)?;
    let nums = &body[..body.len() - unit_char.len_utf8()];
    let mut parts = nums.split('-');
    let low_v = parse_number(parts.next()?.trim())?;
    let low = Duration { value: low_v, unit };
    let high = match parts.next() {
        Some(h) => Some(Duration {
            value: parse_number(h.trim())?,
            unit,
        }),
        None => None,
    };
    Some(DurationRange { low, high, span })
}

/// Parse a full active/passive timer token (`@3m`, `~6-10m`, `~?`).
pub fn parse_timer(text: &str, span: Span) -> Option<Timer> {
    let text = text.trim();
    if text == "~?" {
        return Some(Timer::Unbounded { span });
    }
    if let Some(body) = text.strip_prefix('@') {
        return Some(Timer::Active(parse_duration_range(body, span)?));
    }
    if let Some(body) = text.strip_prefix('~') {
        return Some(Timer::Passive(parse_duration_range(body, span)?));
    }
    None
}

/// Parse a `&lag(...)` duration token: `5m`, `30s`, or a bare `0`.
pub fn parse_lag_duration(text: &str) -> Option<Duration> {
    let text = text.trim();
    if text == "0" {
        return Some(Duration {
            value: 0.0,
            unit: TimeUnit::S,
        });
    }
    let unit_char = text.chars().last()?;
    let unit = parse_time_unit(unit_char)?;
    let num = &text[..text.len() - unit_char.len_utf8()];
    Some(Duration {
        value: parse_number(num.trim())?,
        unit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(parse_number("2"), Some(2.0));
        assert_eq!(parse_number("1.5"), Some(1.5));
        assert_eq!(parse_number(".5"), Some(0.5));
        assert_eq!(parse_number("1/2"), Some(0.5));
        assert_eq!(parse_number("1 1/2"), Some(1.5));
    }

    #[test]
    fn measures() {
        let a = parse_amount("500g", Span::EMPTY);
        match a {
            Amount::Quantity { terms, .. } => match &terms[0] {
                Term::Measure { value, unit, .. } => {
                    assert_eq!(*value, Some(500.0));
                    assert_eq!(unit.as_deref(), Some("g"));
                }
                _ => panic!(),
            },
            _ => panic!(),
        }
    }

    #[test]
    fn compound() {
        let a = parse_amount("1c + 2T", Span::EMPTY);
        match a {
            Amount::Quantity { terms, .. } => assert_eq!(terms.len(), 2),
            _ => panic!(),
        }
    }

    #[test]
    fn basis_constituent() {
        let a = parse_amount("50%* flour", Span::EMPTY);
        match a {
            Amount::Quantity { terms, .. } => match &terms[0] {
                Term::Basis {
                    percent,
                    constituent,
                    basis,
                    ..
                } => {
                    assert_eq!(*percent, Some(50.0));
                    assert!(*constituent);
                    assert_eq!(basis, "flour");
                }
                _ => panic!(),
            },
            _ => panic!(),
        }
    }

    #[test]
    fn to_taste_and_unquantified() {
        assert!(matches!(
            parse_amount("~", Span::EMPTY),
            Amount::ToTaste { .. }
        ));
        assert!(matches!(
            parse_amount("?", Span::EMPTY),
            Amount::Unquantified { .. }
        ));
    }

    #[test]
    fn timers() {
        assert!(matches!(
            parse_timer("@3m", Span::EMPTY),
            Some(Timer::Active(_))
        ));
        assert!(matches!(
            parse_timer("~?", Span::EMPTY),
            Some(Timer::Unbounded { .. })
        ));
        match parse_timer("~6-10m", Span::EMPTY) {
            Some(Timer::Passive(r)) => {
                assert_eq!(r.low.value, 6.0);
                assert_eq!(r.high.unwrap().value, 10.0);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn temps() {
        let t = parse_temp("{350F}", Span::EMPTY).unwrap();
        assert_eq!(t.value, 350.0);
        assert_eq!(t.unit, TempUnit::F);
    }

    #[test]
    fn lag() {
        assert_eq!(parse_lag_duration("0").unwrap().value, 0.0);
        assert_eq!(parse_lag_duration("5m").unwrap().seconds(), 300.0);
    }
}
