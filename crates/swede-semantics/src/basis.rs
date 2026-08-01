//! Basis resolution: turn `%`/`%*` amounts into absolute quantities against a
//! declared basis (SPEC §4 "Bases"). Used by the renderer.
//!
//! Note: full Neep-style blended sub-recipe subtraction requires loading the
//! sub-recipe and is deferred. This resolves plain and constituent percentages
//! against the basis total, which is what the table projection needs to show
//! constituent shares summing to 100%.

use std::collections::HashMap;
use swede_syntax::amount::parse_number;
use swede_syntax::*;

#[derive(Debug, Clone)]
pub struct BasisTable {
    /// basis name -> (value, unit)
    map: HashMap<String, (f64, String)>,
}

impl BasisTable {
    pub fn from_recipe(r: &Recipe) -> Self {
        let mut map = HashMap::new();
        for b in &r.bases {
            if let Some((v, u)) = measure_value_unit(&b.amount) {
                map.insert(b.name.clone(), (v, u));
            }
        }
        BasisTable { map }
    }

    pub fn get(&self, name: &str) -> Option<(f64, String)> {
        self.map.get(name).cloned()
    }

    /// Resolve a single term to an absolute (value, unit), if possible.
    pub fn resolve_term(&self, t: &Term) -> Option<(f64, String)> {
        match t {
            Term::Measure { value, unit, .. } => {
                value.map(|v| (v, unit.clone().unwrap_or_default()))
            }
            Term::Basis {
                percent: Some(p),
                basis,
                ..
            } => {
                let (bv, bu) = self.map.get(basis)?;
                Some((bv * p / 100.0, bu.clone()))
            }
            Term::Basis { percent: None, .. } => None,
        }
    }

    /// Resolve a whole amount to an absolute (value, unit) when all terms share
    /// a unit; returns None for to-taste / unquantified / mixed-unit amounts.
    pub fn resolve_amount(&self, a: &Amount) -> Option<(f64, String)> {
        if let Amount::Quantity { terms, .. } = a {
            let mut total = 0.0;
            let mut unit = String::new();
            for t in terms {
                let (v, u) = self.resolve_term(t)?;
                if unit.is_empty() {
                    unit = u;
                } else if unit != u && !u.is_empty() {
                    return None;
                }
                total += v;
            }
            Some((total, unit))
        } else {
            None
        }
    }
}

fn measure_value_unit(a: &Amount) -> Option<(f64, String)> {
    if let Amount::Quantity { terms, .. } = a {
        if let Some(Term::Measure {
            value: Some(v),
            unit,
            ..
        }) = terms.first()
        {
            return Some((*v, unit.clone().unwrap_or_default()));
        }
    }
    None
}

/// Best-effort numeric value of an amount's first term (for display helpers).
pub fn amount_number(a: &Amount) -> Option<f64> {
    match a {
        Amount::Quantity { raw, .. } => {
            parse_number(raw.split(|c: char| c.is_alphabetic()).next()?.trim())
        }
        _ => None,
    }
}
