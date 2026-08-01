//! The recipe graph: a shared model built from a validated AST, consumed by the
//! renderer and the scheduler.
//!
//! Linear consumption makes this a tree rooted at the terminal, except at split
//! points (one op with several named products), which are DAG forks.

use std::collections::HashMap;
use swede_syntax::*;

pub type NodeId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// An ingredient leaf.
    Ingredient,
    /// An operation (a verb applied to inputs).
    Op,
    /// A projected product of a split op.
    Product,
    /// A sub-recipe reference leaf.
    SubRecipe,
    /// A passthrough binding (`x = y`).
    Passthrough,
}

#[derive(Debug, Clone)]
pub struct EquipUse {
    pub name: String,
    pub temp: Option<Temp>,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub name: Option<String>,
    /// Display label: the verb, ingredient name, or `<+path>`.
    pub label: String,
    pub kind: NodeKind,
    pub inputs: Vec<NodeId>,
    pub amount: Option<Amount>,
    pub qualifier: Option<String>,
    pub timers: Vec<Timer>,
    pub temp: Option<Temp>,
    pub flags: Vec<String>,
    pub docs: Vec<String>,
    pub links: Vec<Link>,
    pub equipment: Vec<EquipUse>,
    pub is_terminal: bool,
}

impl Node {
    /// Whether this op has any ingredient leaf in its subtree (rule 6: nodes
    /// with none render as equipment-prep header strips).
    pub fn is_equipment_prep(&self, g: &Graph) -> bool {
        matches!(self.kind, NodeKind::Op) && g.subtree_leaves(self.id).is_empty()
    }

    pub fn has_passive(&self) -> bool {
        self.timers
            .iter()
            .any(|t| matches!(t, Timer::Passive(_) | Timer::Unbounded { .. }))
    }

    pub fn has_unbounded(&self) -> bool {
        self.timers
            .iter()
            .any(|t| matches!(t, Timer::Unbounded { .. }))
    }
}

#[derive(Debug, Clone)]
pub struct Graph {
    pub name: String,
    pub nodes: Vec<Node>,
    pub terminal: NodeId,
}

impl Graph {
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    /// The longest path from any leaf under `id` (its table entry column).
    pub fn depth(&self, id: NodeId) -> usize {
        let n = &self.nodes[id];
        if n.inputs.is_empty() {
            0
        } else {
            1 + n.inputs.iter().map(|&i| self.depth(i)).max().unwrap_or(0)
        }
    }

    pub fn max_depth(&self) -> usize {
        (0..self.nodes.len())
            .map(|i| self.depth(i))
            .max()
            .unwrap_or(0)
    }

    /// Ingredient leaves under `id`, in stable order.
    pub fn subtree_leaves(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.collect_leaves(id, &mut out);
        out
    }

    fn collect_leaves(&self, id: NodeId, out: &mut Vec<NodeId>) {
        let n = &self.nodes[id];
        if matches!(n.kind, NodeKind::Ingredient | NodeKind::SubRecipe) && n.inputs.is_empty() {
            out.push(id);
        } else {
            for &i in &n.inputs {
                self.collect_leaves(i, out);
            }
        }
    }
}

struct Builder {
    nodes: Vec<Node>,
    by_name: HashMap<String, NodeId>,
}

impl Builder {
    fn add(&mut self, mut n: Node) -> NodeId {
        let id = self.nodes.len();
        n.id = id;
        if let Some(name) = &n.name {
            self.by_name.insert(name.clone(), id);
        }
        self.nodes.push(n);
        id
    }

    fn blank(&self, kind: NodeKind, label: String) -> Node {
        Node {
            id: 0,
            name: None,
            label,
            kind,
            inputs: Vec::new(),
            amount: None,
            qualifier: None,
            timers: Vec::new(),
            temp: None,
            flags: Vec::new(),
            docs: Vec::new(),
            links: Vec::new(),
            equipment: Vec::new(),
            is_terminal: false,
        }
    }

    /// Lower an expression, returning the id of its root op/leaf and its
    /// equipment uses (bubbled up from arg-position `!equip`).
    fn lower_expr(&mut self, e: &Expr) -> (NodeId, Vec<EquipUse>) {
        match e {
            Expr::Call(c) => self.lower_call(c),
            Expr::SubRef(s) => {
                let label = format!("<+{}>", s.path.join("/"));
                let mut n = self.blank(NodeKind::SubRecipe, label);
                n.amount = s.amount.clone();
                (self.add(n), Vec::new())
            }
            Expr::NodeRef(r) => {
                if let Some(&id) = self.by_name.get(&r.name) {
                    (id, Vec::new())
                } else {
                    // dangling ref (invalid recipe); represent as a leaf
                    let n = self.blank(NodeKind::Ingredient, r.name.clone());
                    (self.add(n), Vec::new())
                }
            }
        }
    }

    fn lower_call(&mut self, c: &Call) -> (NodeId, Vec<EquipUse>) {
        let mut inputs = Vec::new();
        let mut equipment = Vec::new();
        for arg in &c.args {
            match arg {
                Arg::Call(cc) => {
                    let (id, eq) = self.lower_call(cc);
                    inputs.push(id);
                    equipment.extend(eq);
                }
                Arg::Ingredient(i) => {
                    let mut n = self.blank(NodeKind::Ingredient, i.name.clone());
                    n.amount = Some(i.amount.clone());
                    n.qualifier = i.qualifier.clone();
                    inputs.push(self.add(n));
                }
                Arg::Equip(e) => equipment.push(EquipUse {
                    name: e.name.clone(),
                    temp: e.temp,
                }),
                Arg::NodeRef(r) => {
                    if let Some(&id) = self.by_name.get(&r.name) {
                        inputs.push(id);
                    }
                }
            }
        }
        for ei in &c.equipment {
            match ei {
                EquipItem::Equip(e) => equipment.push(EquipUse {
                    name: e.name.clone(),
                    temp: e.temp,
                }),
                EquipItem::NodeRef(r) => {
                    if let Some(&id) = self.by_name.get(&r.name) {
                        inputs.push(id); // an equipment-state node is a dependency
                    }
                }
            }
        }
        let mut n = self.blank(NodeKind::Op, c.verb.clone());
        n.inputs = inputs;
        n.equipment = equipment;
        (self.add(n), Vec::new())
    }
}

/// Build the graph from a recipe AST (assumed valid).
pub fn build(r: &Recipe) -> Graph {
    let mut b = Builder {
        nodes: Vec::new(),
        by_name: HashMap::new(),
    };
    let last = r.bindings.len().saturating_sub(1);
    let mut terminal = 0;

    for (idx, binding) in r.bindings.iter().enumerate() {
        let (root, _eq) = b.lower_expr(&binding.value);

        // attach annotations
        let (timers, temp, flags, docs, links) = split_annotations(&binding.annotations);

        if binding.outputs.len() == 1 {
            let o = &binding.outputs[0];
            let n = &mut b.nodes[root];
            n.name = Some(o.name.clone());
            n.timers = timers;
            n.temp = temp;
            n.flags = flags;
            n.docs = docs;
            n.links = links;
            b.by_name.insert(o.name.clone(), root);
            if idx == last {
                n.is_terminal = true;
                terminal = root;
            }
        } else {
            // split: op stays anonymous, products project from it
            {
                let n = &mut b.nodes[root];
                n.timers = timers;
                n.temp = temp;
                n.flags = flags;
                n.docs = docs;
                n.links = links;
            }
            for o in &binding.outputs {
                let mut p = b.blank(NodeKind::Product, o.name.clone());
                p.name = Some(o.name.clone());
                p.inputs = vec![root];
                if idx == last {
                    p.is_terminal = true;
                }
                let pid = b.add(p);
                if idx == last {
                    terminal = pid;
                }
            }
        }
    }

    Graph {
        name: r.name.clone(),
        nodes: b.nodes,
        terminal,
    }
}

type SplitAnns = (
    Vec<Timer>,
    Option<Temp>,
    Vec<String>,
    Vec<String>,
    Vec<Link>,
);

fn split_annotations(anns: &[Annotation]) -> SplitAnns {
    let mut timers = Vec::new();
    let mut temp = None;
    let mut flags = Vec::new();
    let mut docs = Vec::new();
    let mut links = Vec::new();
    for a in anns {
        match a {
            Annotation::Timer(t) => timers.push(t.clone()),
            Annotation::Temp(t) => temp = Some(*t),
            Annotation::Flag { name, .. } => flags.push(name.clone()),
            Annotation::Doc { text, .. } => docs.push(text.clone()),
            Annotation::Link(l) => links.push(l.clone()),
        }
    }
    (timers, temp, flags, docs, links)
}
