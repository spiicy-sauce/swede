//! Monospace depth-column grid (SPEC §9 table).
//!
//! Rows are terminal->leaf paths (a leaf feeding a split appears in one row per
//! product, which reads as the fork band). Columns are node depth. A node that
//! spans several rows is printed once at the top of its run; the rows beneath it
//! stay blank, so a multi-row op reads as a vertical band. Equipment-prep nodes
//! (no ingredient subtree) render as header strips above the grid (rule 6).

use swede_semantics::model::{Graph, NodeId, NodeKind};
use swede_semantics::{build_graph, BasisTable};
use swede_syntax::*;

use crate::{fmt_amount_pub, num, pretty_name};

pub fn grid_table(r: &Recipe) -> String {
    let g = build_graph(r);
    let basis = BasisTable::from_recipe(r);
    let maxd = g.max_depth();
    let ncols = maxd + 1;

    let mut paths: Vec<Vec<NodeId>> = Vec::new();
    dfs(&g, g.terminal, &mut Vec::new(), &mut paths);

    // cell[row][col] = (node id, label)
    let nrows = paths.len();
    let mut cell: Vec<Vec<Option<(NodeId, String)>>> = vec![vec![None; ncols]; nrows];
    for (ri, path) in paths.iter().enumerate() {
        for &id in path {
            let c = g.depth(id);
            cell[ri][c] = Some((id, label_of(&g, id, &basis)));
        }
    }

    let mut w = vec![0usize; ncols];
    for row in &cell {
        for (c, item) in row.iter().enumerate() {
            if let Some((_, t)) = item {
                w[c] = w[c].max(t.chars().count());
            }
        }
    }

    let mut out = String::new();
    out.push_str(&pretty_name(&g.name));
    out.push('\n');

    // equipment-prep header strips
    for n in &g.nodes {
        if n.is_equipment_prep(&g) {
            let eq = n
                .equipment
                .iter()
                .map(|e| match &e.temp {
                    Some(t) => format!("!{} {}{}", e.name, num(t.value), tu(t.unit)),
                    None => format!("!{}", e.name),
                })
                .collect::<Vec<_>>()
                .join(", ");
            let link = n
                .links
                .first()
                .map(|l| match l {
                    Link::By { target, .. } => format!("  (finish by: {})", target.name),
                    _ => String::new(),
                })
                .unwrap_or_default();
            let name = n.name.clone().unwrap_or_default();
            out.push_str(&format!("[ {name}: {} {eq} ]{link}\n", n.label));
        }
    }
    out.push('\n');

    // rows, blanking band continuations
    let mut prev: Vec<Option<NodeId>> = vec![None; ncols];
    for row in &cell {
        let mut line = String::new();
        for c in 0..ncols {
            let text = match &row[c] {
                Some((id, t)) => {
                    if prev[c] == Some(*id) {
                        pad("", w[c]) // band continuation
                    } else {
                        prev[c] = Some(*id);
                        pad(t, w[c])
                    }
                }
                None => {
                    prev[c] = None;
                    pad("", w[c])
                }
            };
            line.push_str(&text);
            if c + 1 < ncols {
                line.push_str("  ");
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn dfs(g: &Graph, id: NodeId, cur: &mut Vec<NodeId>, out: &mut Vec<Vec<NodeId>>) {
    cur.push(id);
    let n = g.node(id);
    // equipment-prep inputs are shown as header strips, not grid rows
    let inputs: Vec<NodeId> = n
        .inputs
        .iter()
        .copied()
        .filter(|&i| !g.node(i).is_equipment_prep(g))
        .collect();
    if inputs.is_empty() {
        out.push(cur.clone());
    } else {
        for i in inputs {
            dfs(g, i, cur, out);
        }
    }
    cur.pop();
}

fn label_of(g: &Graph, id: NodeId, basis: &BasisTable) -> String {
    let n = g.node(id);
    match n.kind {
        NodeKind::Ingredient => {
            let amt = n
                .amount
                .as_ref()
                .map(|a| fmt_amount_pub(a, basis))
                .unwrap_or_default();
            if amt.is_empty() {
                n.label.clone()
            } else {
                format!("{} {}", n.label, amt)
            }
        }
        NodeKind::SubRecipe => {
            let amt = n
                .amount
                .as_ref()
                .map(|a| fmt_amount_pub(a, basis))
                .unwrap_or_default();
            format!("{} {}", n.label, amt).trim().to_string()
        }
        NodeKind::Product => n.name.clone().unwrap_or_else(|| n.label.clone()),
        NodeKind::Op | NodeKind::Passthrough => {
            let mut s = n.label.clone();
            let times: Vec<String> = n.timers.iter().map(time_marker).collect();
            if !times.is_empty() {
                s.push(' ');
                s.push_str(&times.join(" "));
            }
            s
        }
    }
}

fn time_marker(t: &Timer) -> String {
    match t {
        Timer::Active(r) => format!("@{}", rng(r)),
        Timer::Passive(r) => format!("~{}", rng(r)),
        Timer::Unbounded { .. } => "~?".to_string(),
    }
}

fn rng(r: &DurationRange) -> String {
    let u = tsu(r.low.unit);
    match r.high {
        Some(h) => format!("{}-{}{}", num(r.low.value), num(h.value), u),
        None => format!("{}{}", num(r.low.value), u),
    }
}

fn pad(s: &str, w: usize) -> String {
    let len = s.chars().count();
    if len >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - len))
    }
}

fn tu(u: TempUnit) -> &'static str {
    match u {
        TempUnit::F => "F",
        TempUnit::C => "C",
    }
}
fn tsu(u: TimeUnit) -> &'static str {
    match u {
        TimeUnit::S => "s",
        TimeUnit::M => "m",
        TimeUnit::H => "h",
        TimeUnit::D => "d",
    }
}
