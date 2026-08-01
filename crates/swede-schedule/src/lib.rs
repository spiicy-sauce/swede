//! `swede-schedule` — a single-cook greedy scheduler stab (SPEC §5, Stage 2/3).
//!
//! This is a feasibility stab, not the full power feature. It computes an
//! as-soon-as-possible pass over the data graph, then back-places `&by` nodes
//! as-late-as-possible toward their target (so a preheat finishes just in time
//! rather than starting at T=0). Attention-capacity serialization of `@` blocks
//! is reported but not yet enforced.

use std::collections::HashMap;
use std::path::Path;
use swede_semantics::model::{build, Graph, NodeId, NodeKind};
use swede_syntax::*;

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub verb: String,
    pub start: f64,
    pub finish: f64,
    pub active: bool,
    pub equipment: Vec<(String, Option<Temp>)>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub title: String,
    pub lanes: Vec<Lane>,
    pub makespan: f64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Lane {
    pub label: String,
    pub tasks: Vec<Task>,
}

fn node_duration(g: &Graph, id: NodeId) -> f64 {
    g.node(id)
        .timers
        .iter()
        .map(|t| match t {
            Timer::Active(r) | Timer::Passive(r) => r.high.unwrap_or(r.low).seconds(),
            Timer::Unbounded { .. } => 0.0,
        })
        .fold(0.0, f64::max)
}

fn is_active(g: &Graph, id: NodeId) -> bool {
    g.node(id)
        .timers
        .iter()
        .any(|t| matches!(t, Timer::Active(_)))
}

/// ASAP finish/start for every node (id order is topological).
fn asap(g: &Graph) -> (Vec<f64>, Vec<f64>) {
    let n = g.nodes.len();
    let mut start = vec![0.0; n];
    let mut finish = vec![0.0; n];
    for id in 0..n {
        let s = g
            .node(id)
            .inputs
            .iter()
            .map(|&i| finish[i])
            .fold(0.0, f64::max);
        start[id] = s;
        finish[id] = s + node_duration(g, id);
    }
    (start, finish)
}

/// Schedule one recipe. Returns a plan whose times are seconds from recipe start.
pub fn schedule_recipe(r: &Recipe) -> Plan {
    let g = build(r);
    let (mut start, mut finish) = asap(&g);
    let makespan = finish[g.terminal];

    // back-place &by nodes as-late-as-possible toward their target
    let name_to_id: HashMap<&str, NodeId> = g
        .nodes
        .iter()
        .filter_map(|n| n.name.as_deref().map(|nm| (nm, n.id)))
        .collect();
    let mut notes = Vec::new();
    for id in 0..g.nodes.len() {
        for link in &g.node(id).links {
            if let Link::By { target, .. } = link {
                if let Some(&tid) = name_to_id.get(target.name.as_str()) {
                    let dur = node_duration(&g, id);
                    let target_finish = finish[tid];
                    if target_finish >= finish[id] {
                        finish[id] = target_finish;
                        start[id] = target_finish - dur;
                        if let Some(nm) = &g.node(id).name {
                            notes.push(format!(
                                "'{}' back-placed to finish by '{}' (starts at {}, not T=0)",
                                nm,
                                target.name,
                                fmt_min(start[id])
                            ));
                        }
                    }
                }
            }
        }
    }

    let tasks = build_tasks(&g, &start, &finish);
    // attention note
    let active_total: f64 = tasks
        .iter()
        .filter(|t| t.active)
        .map(|t| t.finish - t.start)
        .sum();
    notes.push(format!(
        "single cook: {} active, {} elapsed",
        fmt_dur(active_total),
        fmt_dur(makespan)
    ));

    Plan {
        title: r.name.clone(),
        lanes: vec![Lane {
            label: r.name.clone(),
            tasks,
        }],
        makespan,
        notes,
    }
}

fn build_tasks(g: &Graph, start: &[f64], finish: &[f64]) -> Vec<Task> {
    let mut tasks = Vec::new();
    for id in 0..g.nodes.len() {
        let n = g.node(id);
        // schedule ops / sub-recipes / preheat; skip raw ingredient leaves and thin products
        let interesting = matches!(
            n.kind,
            NodeKind::Op | NodeKind::SubRecipe | NodeKind::Passthrough
        );
        if !interesting {
            continue;
        }
        if node_duration(g, id) == 0.0 && n.equipment.is_empty() && !n.is_terminal {
            // instantaneous, non-equipment intermediate; keep it out of the timeline
            continue;
        }
        let name = n.name.clone().unwrap_or_else(|| n.label.clone());
        // Own equipment, plus equipment inherited from any equipment-state node
        // it consumes (e.g. `bake` consumes `hot`, so it occupies the oven).
        let mut equipment: Vec<(String, Option<Temp>)> = n
            .equipment
            .iter()
            .map(|e| (e.name.clone(), e.temp))
            .collect();
        for &inp in &n.inputs {
            if g.node(inp).is_equipment_prep(g) {
                for e in &g.node(inp).equipment {
                    equipment.push((e.name.clone(), e.temp));
                }
            }
        }
        tasks.push(Task {
            name,
            verb: n.label.clone(),
            start: start[id],
            finish: finish[id],
            active: is_active(g, id),
            equipment,
            note: n.docs.first().cloned(),
        });
    }
    tasks.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    tasks
}

/// Schedule a menu: load each referenced recipe from `dir`, align terminals to a
/// common serve time, and note cross-recipe equipment co-residency.
pub fn schedule_menu(m: &Menu, dir: &Path) -> Result<Plan, String> {
    let mut lanes = Vec::new();
    let mut notes = Vec::new();
    let mut plans: Vec<(String, Plan)> = Vec::new();

    for entry in &m.entries {
        let recipe_name = match &entry.recipe {
            MenuTarget::Name(n) => n.clone(),
            MenuTarget::SubRef(s) => s.path.join("/"),
        };
        let path = dir.join(format!("{recipe_name}.swede"));
        let src = std::fs::read_to_string(&path).map_err(|e| {
            format!(
                "cannot load recipe '{}' ({}): {e}",
                recipe_name,
                path.display()
            )
        })?;
        let lowered = swede_syntax::parse(&src);
        match lowered.file {
            Some(File::Recipe(r)) => {
                let plan = schedule_recipe(&r);
                plans.push((entry.alias.clone(), plan));
            }
            _ => return Err(format!("'{recipe_name}' is not a recipe")),
        }
    }

    // align serve times: every recipe's terminal lands at T=0 (serve), earlier
    // work is negative. This is the default "latest-terminal" alignment.
    let mut all_tasks_by_lane: Vec<Lane> = Vec::new();
    for (alias, plan) in &plans {
        let offset = -plan.makespan; // shift so terminal (finish==makespan) sits at 0
        let tasks: Vec<Task> = plan
            .lanes
            .iter()
            .flat_map(|l| l.tasks.iter())
            .map(|t| Task {
                start: t.start + offset,
                finish: t.finish + offset,
                ..t.clone()
            })
            .collect();
        all_tasks_by_lane.push(Lane {
            label: format!("{alias} ({})", plan.title),
            tasks,
        });
    }

    // cross-recipe equipment co-residency proposals (same !name + same temp,
    // overlapping windows). Verbatim-equal names are the same unit (SPEC §5 G1).
    detect_coresidency(&all_tasks_by_lane, &mut notes);

    lanes.extend(all_tasks_by_lane);
    let makespan = lanes
        .iter()
        .flat_map(|l| l.tasks.iter())
        .map(|t| t.finish)
        .fold(f64::MIN, f64::max);
    let earliest = lanes
        .iter()
        .flat_map(|l| l.tasks.iter())
        .map(|t| t.start)
        .fold(f64::MAX, f64::min);

    notes.push(format!(
        "serve at T=0; earliest start at {}",
        fmt_min(earliest)
    ));

    Ok(Plan {
        title: m.name.clone(),
        lanes,
        makespan,
        notes,
    })
}

fn detect_coresidency(lanes: &[Lane], notes: &mut Vec<String>) {
    let mut equip_windows: Vec<(String, f64, f64, String, f64)> = Vec::new(); // name, start, finish, task, temp
    for lane in lanes {
        for t in &lane.tasks {
            for (name, temp) in &t.equipment {
                let tv = temp.map(|x| x.value).unwrap_or(f64::NAN);
                equip_windows.push((name.clone(), t.start, t.finish, t.name.clone(), tv));
            }
        }
    }
    for i in 0..equip_windows.len() {
        for j in (i + 1)..equip_windows.len() {
            let a = &equip_windows[i];
            let b = &equip_windows[j];
            if a.0 == b.0 && a.4 == b.4 && a.1 < b.2 && b.1 < a.2 {
                notes.push(format!(
                    "co-residency possible: '{}' and '{}' share !{} at {}° in an overlapping window",
                    a.3, b.3, a.0, fmt_num(a.4)
                ));
            }
        }
    }
}

/// Render a plan as a text timeline.
pub fn render_timeline(plan: &Plan) -> String {
    let mut out = String::new();
    out.push_str(&format!("Timeline: {}\n", plan.title));
    for note in &plan.notes {
        out.push_str(&format!("  · {note}\n"));
    }
    out.push('\n');

    // time window
    let min_start = plan
        .lanes
        .iter()
        .flat_map(|l| l.tasks.iter())
        .map(|t| t.start)
        .fold(f64::MAX, f64::min);
    let max_finish = plan
        .lanes
        .iter()
        .flat_map(|l| l.tasks.iter())
        .map(|t| t.finish)
        .fold(f64::MIN, f64::max);
    let span = (max_finish - min_start).max(1.0);
    let width = 48.0;

    for lane in &plan.lanes {
        if plan.lanes.len() > 1 {
            out.push_str(&format!("[{}]\n", lane.label));
        }
        for t in &lane.tasks {
            let s0 = (((t.start - min_start) / span) * width).round() as usize;
            let s1 = (((t.finish - min_start) / span) * width)
                .round()
                .max((s0 + 1) as f64) as usize;
            let mut bar = " ".repeat(s0);
            let ch = if t.active { '█' } else { '░' };
            bar.push_str(&ch.to_string().repeat((s1 - s0).max(1)));
            let equip = if t.equipment.is_empty() {
                String::new()
            } else {
                format!(
                    "  !{}",
                    t.equipment
                        .iter()
                        .map(|(n, _)| n.clone())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            };
            out.push_str(&format!(
                "  {:>7} {:<50} {:<12} {}{}\n",
                fmt_min(t.start),
                bar,
                t.name,
                if t.active { "@active" } else { "~passive" },
                equip
            ));
        }
    }
    out
}

fn fmt_min(sec: f64) -> String {
    let m = sec / 60.0;
    if m.abs() < 0.05 {
        "+0m".to_string()
    } else {
        format!("{:+.0}m", m)
    }
}

fn fmt_dur(sec: f64) -> String {
    let m = (sec / 60.0).round() as i64;
    if m >= 60 {
        format!("{}h{:02}m", m / 60, m % 60)
    } else {
        format!("{m}m")
    }
}

fn fmt_num(v: f64) -> String {
    if v.is_nan() {
        "?".to_string()
    } else {
        format!("{}", v as i64)
    }
}
