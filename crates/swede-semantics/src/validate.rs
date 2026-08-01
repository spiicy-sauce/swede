//! Static validation of a Swede recipe (SPEC §4).
//!
//! Rules implemented here are all static (no schedule): E001-E006, E008,
//! W001-W004. E007 (sub-recipe basis unit) needs a sub-recipe loader and is
//! deferred; W005/W006 are scheduler-era.

use crate::diagnostic::Diagnostic;
use std::collections::{HashMap, HashSet};
use swede_syntax::*;

/// Collected uses within a single binding's expression.
#[derive(Default)]
struct Uses {
    refs: Vec<(String, Option<String>, Span)>,
    ingredients: Vec<(String, Amount, Span)>,
    equips: Vec<String>,
    basis_refs: Vec<(String, Span, bool)>, // (basis, span, is_constituent) — from ingredient amounts
    sub_basis_refs: Vec<(String, Span, bool)>, // from sub_ref amounts (excluded from W004)
}

fn collect_amount_basis(amount: &Amount, out: &mut Vec<(String, Span, bool)>) {
    if let Amount::Quantity { terms, span, .. } = amount {
        for t in terms {
            if let Term::Basis {
                basis, constituent, ..
            } = t
            {
                out.push((basis.clone(), *span, *constituent));
            }
        }
    }
}

fn walk_call(c: &Call, u: &mut Uses) {
    for arg in &c.args {
        match arg {
            Arg::Call(cc) => walk_call(cc, u),
            Arg::Ingredient(i) => {
                u.ingredients
                    .push((i.name.clone(), i.amount.clone(), i.span));
                collect_amount_basis(&i.amount, &mut u.basis_refs);
            }
            Arg::Equip(e) => u.equips.push(e.name.clone()),
            Arg::NodeRef(n) => u.refs.push((n.name.clone(), n.member.clone(), n.span)),
        }
    }
    for ei in &c.equipment {
        match ei {
            EquipItem::Equip(e) => u.equips.push(e.name.clone()),
            EquipItem::NodeRef(n) => u.refs.push((n.name.clone(), n.member.clone(), n.span)),
        }
    }
}

fn collect_uses(value: &Expr) -> Uses {
    let mut u = Uses::default();
    match value {
        Expr::Call(c) => walk_call(c, &mut u),
        Expr::SubRef(s) => {
            if let Some(a) = &s.amount {
                collect_amount_basis(a, &mut u.sub_basis_refs);
            }
        }
        Expr::NodeRef(n) => u.refs.push((n.name.clone(), n.member.clone(), n.span)),
    }
    u
}

/// The v2 keywords reserved by SPEC §10.
const RESERVED: &[&str] = &[
    "alt", "repeat", "foreach", "cooks", "recipe", "menu", "basis",
];

pub fn validate_recipe(r: &Recipe) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // ── Pass A: definitions ──────────────────────────────────────────
    let mut node_def: HashMap<String, usize> = HashMap::new();
    let basis_names: HashSet<String> = r.bases.iter().map(|b| b.name.clone()).collect();
    let mut equip_names: HashSet<String> = HashSet::new();
    // ingredient name -> (count, first_index)
    let mut ing: HashMap<String, (usize, usize)> = HashMap::new();
    let uses: Vec<Uses> = r.bindings.iter().map(|b| collect_uses(&b.value)).collect();

    for (i, b) in r.bindings.iter().enumerate() {
        for o in &b.outputs {
            if RESERVED.contains(&o.name.as_str()) {
                diags.push(Diagnostic::error(
                    "E008",
                    format!(
                        "'{}' is a reserved keyword and cannot be a node name",
                        o.name
                    ),
                    o.span,
                ));
            }
            if node_def.contains_key(&o.name) {
                diags.push(Diagnostic::error(
                    "E008",
                    format!(
                        "rebinding: '{}' is bound more than once (names are single-assignment)",
                        o.name
                    ),
                    o.span,
                ));
            } else {
                node_def.insert(o.name.clone(), i);
            }
        }
        for e in &uses[i].equips {
            equip_names.insert(e.clone());
        }
    }

    // ingredient literal counts + E003
    for (i, u) in uses.iter().enumerate() {
        for (name, _amt, span) in &u.ingredients {
            let entry = ing.entry(name.clone()).or_insert((0, i));
            entry.0 += 1;
            if entry.0 > 1 {
                diags.push(Diagnostic::error(
                    "E003",
                    format!("ingredient '{name}' is given an amount literal more than once; declare it once and split it (e.g. `a[..], b[..] = divide({name}[..])`)"),
                    *span,
                ));
            }
        }
    }

    // basis constituents referencing an undeclared basis -> collect for basis names
    // (bases are declared explicitly; nothing to add here).

    let terminal: HashSet<String> = r
        .bindings
        .last()
        .map(|b| b.outputs.iter().map(|o| o.name.clone()).collect())
        .unwrap_or_default();

    // ── Pass B: reference resolution, consumption, E004/E001 ──────────
    let mut ref_count: HashMap<String, usize> = HashMap::new();
    let mut ancestors: HashMap<String, HashSet<String>> = HashMap::new();

    for (i, b) in r.bindings.iter().enumerate() {
        let u = &uses[i];
        let mut dep_nodes: HashSet<String> = HashSet::new();

        for (name, member, span) in &u.refs {
            if member.is_some() {
                diags.push(Diagnostic::error(
                    "E001",
                    format!(
                        "dotted reference '{name}.{}' is only valid in a menu",
                        member.as_ref().unwrap()
                    ),
                    *span,
                ));
                continue;
            }
            if let Some(&j) = node_def.get(name) {
                if j >= i {
                    diags.push(Diagnostic::error(
                        "E004",
                        format!("forward data reference: '{name}' is defined later; data inputs may only reference earlier bindings"),
                        *span,
                    ));
                } else {
                    *ref_count.entry(name.clone()).or_default() += 1;
                    dep_nodes.insert(name.clone());
                }
            } else if let Some(&(_, first)) = ing.get(name) {
                if first > i {
                    diags.push(Diagnostic::error(
                        "E004",
                        format!("forward data reference: ingredient '{name}' is declared later"),
                        *span,
                    ));
                } else {
                    *ref_count.entry(name.clone()).or_default() += 1;
                }
            } else if basis_names.contains(name) || equip_names.contains(name) {
                // resolves (SPEC E001 allows basis/equipment); not a consumable.
            } else {
                diags.push(Diagnostic::error(
                    "E001",
                    format!("unknown reference '{name}'"),
                    *span,
                ));
            }
        }

        // basis-relative amounts must reference a declared basis
        for (basis, span, _c) in &u.basis_refs {
            if !basis_names.contains(basis) {
                diags.push(Diagnostic::error(
                    "E001",
                    format!("unknown basis '{basis}'"),
                    *span,
                ));
            }
        }
        for (basis, span, _c) in &u.sub_basis_refs {
            if !basis_names.contains(basis) {
                diags.push(Diagnostic::error(
                    "E001",
                    format!("unknown basis '{basis}'"),
                    *span,
                ));
            }
        }

        // ancestry (for E005)
        let mut anc: HashSet<String> = dep_nodes.clone();
        for d in &dep_nodes {
            if let Some(a) = ancestors.get(d) {
                anc.extend(a.iter().cloned());
            }
        }
        for o in &b.outputs {
            ancestors.insert(o.name.clone(), anc.clone());
        }

        // links: resolve targets, E005 contradiction
        for ann in &b.annotations {
            if let Annotation::Link(link) = ann {
                match link {
                    Link::By { target, span } | Link::During { target, span } => {
                        if target.member.is_none() && !node_def.contains_key(&target.name) {
                            diags.push(Diagnostic::error(
                                "E001",
                                format!(
                                    "link target '{}' is not a node in this recipe",
                                    target.name
                                ),
                                *span,
                            ));
                        } else if anc.contains(&target.name) {
                            diags.push(Diagnostic::error(
                                "E005",
                                format!("link contradiction: '{}' is a data ancestor of this node, so the ordering constraint is unsatisfiable", target.name),
                                *span,
                            ));
                        }
                    }
                    Link::Lag { .. } => {}
                }
            }
        }

        // W002: unbounded passive on a node that reaches the terminal
        let has_unbounded = b
            .annotations
            .iter()
            .any(|a| matches!(a, Annotation::Timer(Timer::Unbounded { .. })));
        if has_unbounded {
            // a node reaches the terminal if it is consumed or is itself terminal
            let reaches = b.outputs.iter().any(|o| {
                terminal.contains(&o.name) || ref_count_will_have(&r.bindings, &o.name, i)
            });
            if reaches {
                let span = b.outputs.first().map(|o| o.span).unwrap_or(b.span);
                diags.push(Diagnostic::warning(
                    "W002",
                    format!("unbounded passive (`~?`) on '{}' is on a path to the terminal; the scheduler cannot bound the plan", b.outputs.first().map(|o| o.name.as_str()).unwrap_or("")),
                    span,
                ));
            }
        }

        // W003: split amounts don't sum
        check_split_sum(b, &mut diags);
    }

    // ── Consumption: W001 orphan, E002 reuse ─────────────────────────
    for (i, b) in r.bindings.iter().enumerate() {
        for o in &b.outputs {
            let refs = ref_count.get(&o.name).copied().unwrap_or(0);
            let is_terminal = terminal.contains(&o.name) && i == r.bindings.len() - 1;
            if refs == 0 && !is_terminal {
                diags.push(Diagnostic::warning(
                    "W001",
                    format!(
                        "orphan: '{}' is never consumed and is not the terminal",
                        o.name
                    ),
                    o.span,
                ));
            } else if refs > 1 {
                diags.push(Diagnostic::error(
                    "E002",
                    format!("'{}' is consumed {refs} times; a node may be used once. Insert an explicit split.", o.name),
                    o.span,
                ));
            }
        }
    }
    // ingredients: 1 structural consumption (the literal) + bare refs
    for (name, &(count, _)) in &ing {
        let refs = ref_count.get(name).copied().unwrap_or(0);
        if count == 1 && refs >= 1 {
            // find a span for the message: first ref span
            if let Some(span) = find_ref_span(&uses, name) {
                diags.push(Diagnostic::error(
                    "E002",
                    format!("ingredient '{name}' is consumed more than once; declare it once and split it"),
                    span,
                ));
            }
        }
    }

    // ── W004: ingredient constituents per basis must sum to 100% ──────
    check_constituents(r, &uses, &mut diags);

    diags
}

fn find_ref_span(uses: &[Uses], name: &str) -> Option<Span> {
    for u in uses {
        for (n, _m, s) in &u.refs {
            if n == name {
                return Some(*s);
            }
        }
    }
    None
}

/// Whether some later binding references `name` as a data input.
fn ref_count_will_have(bindings: &[Binding], name: &str, from: usize) -> bool {
    for b in bindings.iter().skip(from + 1) {
        let u = collect_uses(&b.value);
        if u.refs.iter().any(|(n, m, _)| n == name && m.is_none()) {
            return true;
        }
    }
    false
}

fn check_split_sum(b: &Binding, diags: &mut Vec<Diagnostic>) {
    // per-output amounts present?
    let out_amounts: Vec<f64> = b
        .outputs
        .iter()
        .filter_map(|o| o.amount.as_ref().and_then(single_measure))
        .collect();
    if out_amounts.len() != b.outputs.len() || b.outputs.iter().any(|o| o.amount.is_none()) {
        return; // not all outputs carry a comparable amount
    }
    // the single input ingredient amount
    if let Expr::Call(c) = &b.value {
        let ing_amounts: Vec<f64> = c
            .args
            .iter()
            .filter_map(|a| match a {
                Arg::Ingredient(i) => single_measure(&i.amount),
                _ => None,
            })
            .collect();
        if ing_amounts.len() == 1 {
            let out_sum: f64 = out_amounts.iter().sum();
            let input = ing_amounts[0];
            if (out_sum - input).abs() > 1e-6 {
                diags.push(Diagnostic::warning(
                    "W003",
                    format!("split amounts sum to {out_sum} but the input is {input}"),
                    b.span,
                ));
            }
        }
    }
}

/// A single measure term with a numeric value and unit (for arithmetic).
/// Returns the value only when the amount is a single measure whose unit we can
/// compare; compound / basis / to-taste amounts return None.
fn single_measure(a: &Amount) -> Option<f64> {
    match a {
        Amount::Quantity { terms, .. } if terms.len() == 1 => match &terms[0] {
            Term::Measure { value, .. } => *value,
            _ => None,
        },
        _ => None,
    }
}

fn check_constituents(r: &Recipe, _uses: &[Uses], diags: &mut Vec<Diagnostic>) {
    // Sum ingredient constituent percents per basis. Sub-recipe (`%*` on a
    // `<+...>`) constituents are intentionally excluded (SPEC §4 "Bases").
    let mut real: HashMap<String, (f64, Span)> = HashMap::new();
    for b in &r.bindings {
        sum_constituents_expr(&b.value, &mut real);
    }
    for (basis, (sum, span)) in real {
        if !r.bases.iter().any(|bd| bd.name == basis) {
            continue; // undeclared basis already flagged E001
        }
        if (sum - 100.0).abs() > 1e-6 {
            diags.push(Diagnostic::warning(
                "W004",
                format!("constituent percentages of basis '{basis}' sum to {sum}%, not 100%"),
                span,
            ));
        }
    }
}

fn sum_constituents_expr(e: &Expr, out: &mut HashMap<String, (f64, Span)>) {
    match e {
        Expr::Call(c) => {
            for arg in &c.args {
                match arg {
                    Arg::Call(cc) => sum_constituents_expr(&Expr::Call(cc.clone()), out),
                    Arg::Ingredient(i) => sum_constituents_amount(&i.amount, out),
                    _ => {}
                }
            }
        }
        // sub-recipe constituents are intentionally excluded (SPEC §4 "Bases").
        Expr::SubRef(_) | Expr::NodeRef(_) => {}
    }
}

fn sum_constituents_amount(a: &Amount, out: &mut HashMap<String, (f64, Span)>) {
    if let Amount::Quantity { terms, span, .. } = a {
        for t in terms {
            if let Term::Basis {
                percent: Some(p),
                constituent: true,
                basis,
                ..
            } = t
            {
                let e = out.entry(basis.clone()).or_insert((0.0, *span));
                e.0 += p;
            }
        }
    }
}
