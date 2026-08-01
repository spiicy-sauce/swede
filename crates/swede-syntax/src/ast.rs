//! The typed Swede AST.
//!
//! Produced from the tree-sitter parse tree by [`crate::lower`]. Amounts,
//! timers, and temperatures are classified here (not left opaque), so the
//! semantic layer works with structured values.

use serde::Serialize;

/// A byte/point span into the source, for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_row: usize,
    pub start_col: usize,
}

impl Span {
    pub const EMPTY: Span = Span {
        start_byte: 0,
        end_byte: 0,
        start_row: 0,
        start_col: 0,
    };
}

/// A whole file: a recipe or a menu.
#[derive(Debug, Clone, Serialize)]
pub enum File {
    Recipe(Recipe),
    Menu(Menu),
}

#[derive(Debug, Clone, Serialize)]
pub struct Recipe {
    pub name: String,
    pub name_span: Span,
    pub meta: Vec<Meta>,
    pub bases: Vec<Basis>,
    pub bindings: Vec<Binding>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Menu {
    pub name: String,
    pub name_span: Span,
    pub meta: Vec<Meta>,
    pub entries: Vec<MenuEntry>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct MenuEntry {
    pub alias: String,
    /// The referenced recipe: either a bare name or a sub-recipe path.
    pub recipe: MenuTarget,
    pub links: Vec<Link>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum MenuTarget {
    Name(String),
    SubRef(SubRef),
}

#[derive(Debug, Clone, Serialize)]
pub struct Meta {
    pub key: String,
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Basis {
    pub name: String,
    pub amount: Amount,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Binding {
    pub outputs: Vec<Output>,
    pub value: Expr,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Output {
    pub name: String,
    pub amount: Option<Amount>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum Expr {
    Call(Call),
    SubRef(SubRef),
    NodeRef(NodeRef),
}

#[derive(Debug, Clone, Serialize)]
pub struct Call {
    pub verb: String,
    pub verb_span: Span,
    pub args: Vec<Arg>,
    pub equipment: Vec<EquipItem>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum Arg {
    Call(Call),
    Ingredient(Ingredient),
    Equip(Equip),
    NodeRef(NodeRef),
}

#[derive(Debug, Clone, Serialize)]
pub struct Ingredient {
    pub name: String,
    pub amount: Amount,
    pub qualifier: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Equip {
    pub name: String,
    pub temp: Option<Temp>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum EquipItem {
    Equip(Equip),
    NodeRef(NodeRef),
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeRef {
    pub name: String,
    /// The `.member` half of a dotted `alias.node` reference (menu scope only).
    pub member: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubRef {
    pub path: Vec<String>,
    pub anchor: Option<String>,
    pub amount: Option<Amount>,
    pub remap: Vec<(String, String)>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum Annotation {
    Temp(Temp),
    Timer(Timer),
    /// A free-prose string annotation (qualifier / doneness).
    Doc {
        text: String,
        span: Span,
    },
    Flag {
        name: String,
        span: Span,
    },
    Link(Link),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Temp {
    pub value: f64,
    pub unit: TempUnit,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TempUnit {
    F,
    C,
}

#[derive(Debug, Clone, Serialize)]
pub enum Timer {
    Active(DurationRange),
    Passive(DurationRange),
    /// `~?` — unbounded passive.
    Unbounded {
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct DurationRange {
    pub low: Duration,
    pub high: Option<Duration>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Duration {
    pub value: f64,
    pub unit: TimeUnit,
}

impl Duration {
    /// Seconds, for scheduling arithmetic.
    pub fn seconds(&self) -> f64 {
        self.value * self.unit.seconds()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TimeUnit {
    S,
    M,
    H,
    D,
}

impl TimeUnit {
    pub fn seconds(&self) -> f64 {
        match self {
            TimeUnit::S => 1.0,
            TimeUnit::M => 60.0,
            TimeUnit::H => 3600.0,
            TimeUnit::D => 86400.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Link {
    By { target: NodeRef, span: Span },
    During { target: NodeRef, span: Span },
    Lag { duration: Duration, span: Span },
}

impl Link {
    pub fn span(&self) -> Span {
        match self {
            Link::By { span, .. } | Link::During { span, .. } | Link::Lag { span, .. } => *span,
        }
    }
}

/// A classified amount inside `[...]`.
#[derive(Debug, Clone, Serialize)]
pub enum Amount {
    /// One or more `+`-joined terms (`1c`, `1c + 2T`, `50%* flour`).
    Quantity {
        terms: Vec<Term>,
        raw: String,
        span: Span,
    },
    /// `~` — to taste.
    ToTaste { span: Span },
    /// `?` — stated but unquantified.
    Unquantified { span: Span },
}

impl Amount {
    pub fn span(&self) -> Span {
        match self {
            Amount::Quantity { span, .. }
            | Amount::ToTaste { span, .. }
            | Amount::Unquantified { span, .. } => *span,
        }
    }
    pub fn raw(&self) -> String {
        match self {
            Amount::Quantity { raw, .. } => raw.clone(),
            Amount::ToTaste { .. } => "~".to_string(),
            Amount::Unquantified { .. } => "?".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Term {
    /// A literal measure: `500 g`, `2 T`, `8`, `pinch`.
    Measure {
        value: Option<f64>,
        unit: Option<String>,
        raw: String,
    },
    /// A basis-relative amount: `72% flour` (plain) or `50%* flour` (constituent).
    Basis {
        percent: Option<f64>,
        constituent: bool,
        basis: String,
        raw: String,
    },
}
