//! `swede-render` — projections of the recipe graph.
//!
//! Two table projections (SPEC §9):
//! - [`flow_table`]: a Markdown table, one row per operation, always correct.
//! - [`grid_table`]: a monospace depth-column grid (ingredients as rows,
//!   operations flowing left-to-right by depth), the Cooking-for-Engineers
//!   layout. Splits are annotated on the op cell.

mod grid;

use swede_semantics::model::{Graph, NodeId, NodeKind};
use swede_semantics::{build_graph, BasisTable};
use swede_syntax::*;

pub use grid::grid_table;

/// Render a recipe to a Markdown flow table.
pub fn flow_table(r: &Recipe) -> String {
    let g = build_graph(r);
    let basis = BasisTable::from_recipe(r);
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", pretty_name(&r.name)));
    for m in &r.meta {
        out.push_str(&format!("- **{}**: {}\n", m.key, m.value));
    }
    if !r.bases.is_empty() {
        out.push('\n');
        for b in &r.bases {
            out.push_str(&format!(
                "- **basis** `{}` = {}\n",
                b.name,
                b.amount.raw().trim()
            ));
        }
    }
    out.push('\n');

    out.push_str("| # | node | verb | inputs | time | equipment | notes |\n");
    out.push_str("|---|------|------|--------|------|-----------|-------|\n");

    let mut step = 0;
    for id in 0..g.nodes.len() {
        let n = g.node(id);
        // one row per named operation / sub-recipe / passthrough (not raw leaves or products)
        match n.kind {
            NodeKind::Op | NodeKind::SubRecipe | NodeKind::Passthrough => {}
            _ => continue,
        }
        let name = match &n.name {
            Some(x) => x.clone(),
            None if n.kind == NodeKind::Op => split_products(&g, id),
            None => continue,
        };
        step += 1;
        let verb = n.label.clone();
        let inputs = fmt_inputs(&g, id, &basis);
        let time = fmt_times(n);
        let equip = fmt_equipment(n);
        let mut notes: Vec<String> = Vec::new();
        for f in &n.flags {
            notes.push(format!("[{f}]"));
        }
        notes.extend(n.docs.iter().cloned());
        for l in &n.links {
            notes.push(fmt_link(l));
        }
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            step,
            md(&name),
            md(&verb),
            md(&inputs),
            md(&time),
            md(&equip),
            md(&notes.join("; ")),
        ));
    }
    out
}

/// The products of an anonymous split op, as `whites, greens`.
fn split_products(g: &Graph, op: NodeId) -> String {
    let names: Vec<String> = g
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Product && n.inputs.contains(&op))
        .filter_map(|n| n.name.clone())
        .collect();
    if names.is_empty() {
        "(anon)".to_string()
    } else {
        names.join(", ")
    }
}

fn fmt_inputs(g: &Graph, id: NodeId, basis: &BasisTable) -> String {
    let n = g.node(id);
    let mut parts = Vec::new();
    for &i in &n.inputs {
        let inp = g.node(i);
        match inp.kind {
            NodeKind::Ingredient => {
                let amt = inp
                    .amount
                    .as_ref()
                    .map(|a| fmt_amount(a, basis))
                    .unwrap_or_default();
                let q = inp
                    .qualifier
                    .as_ref()
                    .map(|q| format!(" \"{q}\""))
                    .unwrap_or_default();
                if amt.is_empty() {
                    parts.push(format!("{}{}", inp.label, q));
                } else {
                    parts.push(format!("{} {}{}", inp.label, amt, q));
                }
            }
            NodeKind::SubRecipe => parts.push(inp.label.clone()),
            _ => parts.push(inp.name.clone().unwrap_or_else(|| inp.label.clone())),
        }
    }
    parts.join(", ")
}

pub(crate) fn fmt_amount_pub(a: &Amount, basis: &BasisTable) -> String {
    fmt_amount(a, basis)
}

fn fmt_amount(a: &Amount, basis: &BasisTable) -> String {
    match a {
        Amount::ToTaste { .. } => "to taste".to_string(),
        Amount::Unquantified { .. } => "?".to_string(),
        Amount::Quantity { raw, .. } => {
            if let Some((v, u)) = basis.resolve_amount(a) {
                // show resolved absolute for basis-relative amounts
                if raw.contains('%') {
                    return format!("{} ({}{})", raw.trim(), num(v), u);
                }
            }
            raw.trim().to_string()
        }
    }
}

fn fmt_times(n: &Node) -> String {
    n.timers.iter().map(fmt_timer).collect::<Vec<_>>().join(" ")
}

use swede_semantics::model::Node;

fn fmt_timer(t: &Timer) -> String {
    match t {
        Timer::Active(r) => format!("@{}", fmt_range(r)),
        Timer::Passive(r) => format!("~{}", fmt_range(r)),
        Timer::Unbounded { .. } => "~? (unbounded)".to_string(),
    }
}

fn fmt_range(r: &DurationRange) -> String {
    let u = unit_str(r.low.unit);
    match r.high {
        Some(h) => format!("{}-{}{}", num(r.low.value), num(h.value), u),
        None => format!("{}{}", num(r.low.value), u),
    }
}

fn fmt_equipment(n: &Node) -> String {
    n.equipment
        .iter()
        .map(|e| match &e.temp {
            Some(t) => format!("!{} {}{}", e.name, num(t.value), temp_unit(t.unit)),
            None => format!("!{}", e.name),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn fmt_link(l: &Link) -> String {
    match l {
        Link::By { target, .. } => format!("finish by: {}", node_ref_str(target)),
        Link::During { target, .. } => format!("during: {}", node_ref_str(target)),
        Link::Lag { duration, .. } => {
            format!("lag ≤ {}{}", num(duration.value), unit_str(duration.unit))
        }
    }
}

fn node_ref_str(r: &NodeRef) -> String {
    match &r.member {
        Some(m) => format!("{}.{}", r.name, m),
        None => r.name.clone(),
    }
}

fn unit_str(u: TimeUnit) -> &'static str {
    match u {
        TimeUnit::S => "s",
        TimeUnit::M => "m",
        TimeUnit::H => "h",
        TimeUnit::D => "d",
    }
}

fn temp_unit(u: TempUnit) -> &'static str {
    match u {
        TempUnit::F => "F",
        TempUnit::C => "C",
    }
}

/// Format a number without a trailing `.0`.
pub(crate) fn num(v: f64) -> String {
    if (v.fract()).abs() < 1e-9 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn md(s: &str) -> String {
    s.replace('|', "\\|")
}

pub(crate) fn pretty_name(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
