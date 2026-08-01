//! Lower a tree-sitter parse tree into the typed [`crate::ast`].

use crate::amount::{parse_amount, parse_lag_duration, parse_temp, parse_timer};
use crate::ast::*;
use tree_sitter::Node;

/// A syntax-level error (an ERROR or MISSING node from the parser).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyntaxError {
    pub message: String,
    pub span: Span,
}

pub struct Lowered {
    pub file: Option<File>,
    pub errors: Vec<SyntaxError>,
}

struct Ctx<'a> {
    src: &'a [u8],
}

impl<'a> Ctx<'a> {
    fn span(&self, n: Node) -> Span {
        let p = n.start_position();
        Span {
            start_byte: n.start_byte(),
            end_byte: n.end_byte(),
            start_row: p.row,
            start_col: p.column,
        }
    }

    fn text(&self, n: Node) -> String {
        n.utf8_text(self.src).unwrap_or("").to_string()
    }

    fn named_of_kind<'t>(&self, n: Node<'t>, kind: &str) -> Vec<Node<'t>> {
        let mut cur = n.walk();
        n.named_children(&mut cur)
            .filter(|c| c.kind() == kind)
            .collect()
    }

    fn field<'t>(&self, n: Node<'t>, name: &str) -> Option<Node<'t>> {
        n.child_by_field_name(name)
    }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let inner = s
        .strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .unwrap_or(s);
    inner.replace("\\\"", "\"")
}

/// Parse Swede source into an AST plus any syntax errors.
pub fn lower(source: &str) -> Lowered {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_swede::LANGUAGE.into())
        .expect("load swede grammar");
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            return Lowered {
                file: None,
                errors: vec![SyntaxError {
                    message: "parse failed".into(),
                    span: Span::EMPTY,
                }],
            }
        }
    };
    let ctx = Ctx {
        src: source.as_bytes(),
    };
    let root = tree.root_node();

    let mut errors = Vec::new();
    collect_errors(&ctx, root, &mut errors);

    let file = root.named_child(0).and_then(|n| match n.kind() {
        "recipe" => Some(File::Recipe(lower_recipe(&ctx, n))),
        "menu" => Some(File::Menu(lower_menu(&ctx, n))),
        _ => None,
    });

    Lowered { file, errors }
}

fn collect_errors(ctx: &Ctx, node: Node, out: &mut Vec<SyntaxError>) {
    if node.is_error() || node.is_missing() {
        out.push(SyntaxError {
            message: if node.is_missing() {
                format!("missing {}", node.kind())
            } else {
                "syntax error".to_string()
            },
            span: ctx.span(node),
        });
        return; // don't descend into an error subtree
    }
    let mut cur = node.walk();
    for child in node.children(&mut cur) {
        collect_errors(ctx, child, out);
    }
}

fn lower_recipe(ctx: &Ctx, n: Node) -> Recipe {
    let name_node = ctx.field(n, "name");
    let mut meta = Vec::new();
    let mut bases = Vec::new();
    let mut bindings = Vec::new();

    let mut cur = n.walk();
    for child in n.named_children(&mut cur) {
        match child.kind() {
            "meta_line" => meta.push(lower_meta(ctx, child)),
            "basis_decl" => bases.push(lower_basis(ctx, child)),
            "binding" => bindings.push(lower_binding(ctx, child)),
            _ => {}
        }
    }

    Recipe {
        name: name_node.map(|x| ctx.text(x)).unwrap_or_default(),
        name_span: name_node.map(|x| ctx.span(x)).unwrap_or(Span::EMPTY),
        meta,
        bases,
        bindings,
        span: ctx.span(n),
    }
}

fn lower_menu(ctx: &Ctx, n: Node) -> Menu {
    let name_node = ctx.field(n, "name");
    let mut meta = Vec::new();
    let mut entries = Vec::new();
    let mut cur = n.walk();
    for child in n.named_children(&mut cur) {
        match child.kind() {
            "meta_line" => meta.push(lower_meta(ctx, child)),
            "menu_stmt" => entries.push(lower_menu_entry(ctx, child)),
            _ => {}
        }
    }
    Menu {
        name: name_node.map(|x| ctx.text(x)).unwrap_or_default(),
        name_span: name_node.map(|x| ctx.span(x)).unwrap_or(Span::EMPTY),
        meta,
        entries,
        span: ctx.span(n),
    }
}

fn lower_menu_entry(ctx: &Ctx, n: Node) -> MenuEntry {
    let alias = ctx
        .field(n, "alias")
        .map(|x| ctx.text(x))
        .unwrap_or_default();
    let recipe = match ctx.field(n, "recipe") {
        Some(r) if r.kind() == "sub_ref" => MenuTarget::SubRef(lower_sub_ref(ctx, r)),
        Some(r) => MenuTarget::Name(ctx.text(r)),
        None => MenuTarget::Name(String::new()),
    };
    let mut links = Vec::new();
    let mut cur = n.walk();
    for ann in n.children_by_field_name("annotation", &mut cur) {
        if ann.kind() == "link" {
            if let Some(l) = lower_link(ctx, ann) {
                links.push(l);
            }
        }
    }
    MenuEntry {
        alias,
        recipe,
        links,
        span: ctx.span(n),
    }
}

fn lower_meta(ctx: &Ctx, n: Node) -> Meta {
    // `:key value` — key and value are now distinct fields.
    let key = ctx.field(n, "key").map(|x| ctx.text(x)).unwrap_or_default();
    let value = ctx.field(n, "value").map(|x| ctx.text(x).trim().to_string()).unwrap_or_default();
    Meta { key, value, span: ctx.span(n) }
}

fn lower_basis(ctx: &Ctx, n: Node) -> Basis {
    let name = ctx
        .field(n, "name")
        .map(|x| ctx.text(x))
        .unwrap_or_default();
    let amount = ctx
        .field(n, "amount")
        .map(|a| parse_amount(&ctx.text(a), ctx.span(a)))
        .unwrap_or(Amount::Unquantified { span: ctx.span(n) });
    Basis {
        name,
        amount,
        span: ctx.span(n),
    }
}

fn lower_binding(ctx: &Ctx, n: Node) -> Binding {
    let mut outputs = Vec::new();
    if let Some(ol) = ctx.field(n, "outputs") {
        for o in ctx.named_of_kind(ol, "output") {
            outputs.push(lower_output(ctx, o));
        }
    }
    let value = ctx
        .field(n, "value")
        .map(|v| lower_expr(ctx, v))
        .unwrap_or(Expr::NodeRef(NodeRef {
            name: String::new(),
            member: None,
            span: ctx.span(n),
        }));

    let mut annotations = Vec::new();
    let mut cur = n.walk();
    for ann in n.children_by_field_name("annotation", &mut cur) {
        if let Some(a) = lower_annotation(ctx, ann) {
            annotations.push(a);
        }
    }
    Binding {
        outputs,
        value,
        annotations,
        span: ctx.span(n),
    }
}

fn lower_output(ctx: &Ctx, n: Node) -> Output {
    let name = ctx
        .field(n, "name")
        .map(|x| ctx.text(x))
        .unwrap_or_default();
    let amount = ctx
        .field(n, "amount")
        .map(|a| parse_amount(&ctx.text(a), ctx.span(a)));
    Output {
        name,
        amount,
        span: ctx.span(n),
    }
}

fn lower_expr(ctx: &Ctx, n: Node) -> Expr {
    match n.kind() {
        "call" => Expr::Call(lower_call(ctx, n)),
        "sub_ref" => Expr::SubRef(lower_sub_ref(ctx, n)),
        "node_ref" => Expr::NodeRef(lower_node_ref(ctx, n)),
        _ => Expr::NodeRef(NodeRef {
            name: ctx.text(n),
            member: None,
            span: ctx.span(n),
        }),
    }
}

fn lower_call(ctx: &Ctx, n: Node) -> Call {
    let verb_node = ctx.field(n, "verb");
    let args = ctx
        .named_of_kind(n, "arg")
        .into_iter()
        .map(|a| lower_arg(ctx, a))
        .collect();
    let equipment = ctx
        .field(n, "equipment")
        .map(|el| {
            ctx.named_of_kind(el, "equip_item")
                .into_iter()
                .map(|e| lower_equip_item(ctx, e))
                .collect()
        })
        .unwrap_or_default();
    Call {
        verb: verb_node.map(|x| ctx.text(x)).unwrap_or_default(),
        verb_span: verb_node.map(|x| ctx.span(x)).unwrap_or(Span::EMPTY),
        args,
        equipment,
        span: ctx.span(n),
    }
}

fn lower_arg(ctx: &Ctx, n: Node) -> Arg {
    let inner = n.named_child(0).unwrap_or(n);
    match inner.kind() {
        "call" => Arg::Call(lower_call(ctx, inner)),
        "ingredient" => Arg::Ingredient(lower_ingredient(ctx, inner)),
        "equip" => Arg::Equip(lower_equip(ctx, inner)),
        "node_ref" => Arg::NodeRef(lower_node_ref(ctx, inner)),
        _ => Arg::NodeRef(lower_node_ref(ctx, inner)),
    }
}

fn lower_ingredient(ctx: &Ctx, n: Node) -> Ingredient {
    let name = ctx
        .field(n, "name")
        .map(|x| ctx.text(x))
        .unwrap_or_default();
    let amount = ctx
        .field(n, "amount")
        .map(|a| parse_amount(&ctx.text(a), ctx.span(a)))
        .unwrap_or(Amount::Unquantified { span: ctx.span(n) });
    let qualifier = ctx.field(n, "qualifier").map(|q| unquote(&ctx.text(q)));
    Ingredient {
        name,
        amount,
        qualifier,
        span: ctx.span(n),
    }
}

fn lower_equip(ctx: &Ctx, n: Node) -> Equip {
    let name = ctx
        .field(n, "name")
        .map(|x| ctx.text(x))
        .unwrap_or_default();
    let temp = ctx
        .field(n, "temp")
        .and_then(|t| parse_temp(&ctx.text(t), ctx.span(t)));
    Equip {
        name,
        temp,
        span: ctx.span(n),
    }
}

fn lower_equip_item(ctx: &Ctx, n: Node) -> EquipItem {
    let inner = n.named_child(0).unwrap_or(n);
    match inner.kind() {
        "equip" => EquipItem::Equip(lower_equip(ctx, inner)),
        _ => EquipItem::NodeRef(lower_node_ref(ctx, inner)),
    }
}

fn lower_node_ref(ctx: &Ctx, n: Node) -> NodeRef {
    let name = ctx
        .field(n, "name")
        .map(|x| ctx.text(x))
        .unwrap_or_else(|| ctx.text(n));
    let member = ctx.field(n, "member").map(|x| ctx.text(x));
    NodeRef {
        name,
        member,
        span: ctx.span(n),
    }
}

fn lower_sub_ref(ctx: &Ctx, n: Node) -> SubRef {
    let path = ctx
        .field(n, "path")
        .map(|p| {
            ctx.named_of_kind(p, "identifier")
                .into_iter()
                .map(|i| ctx.text(i))
                .collect()
        })
        .unwrap_or_default();
    let anchor = ctx.field(n, "anchor").map(|x| ctx.text(x));
    let amount = ctx
        .field(n, "amount")
        .map(|a| parse_amount(&ctx.text(a), ctx.span(a)));
    let remap = ctx
        .field(n, "remap")
        .map(|r| {
            ctx.named_of_kind(r, "mapping")
                .into_iter()
                .map(|m| {
                    let from = ctx
                        .field(m, "from")
                        .map(|x| ctx.text(x))
                        .unwrap_or_default();
                    let to = ctx.field(m, "to").map(|x| ctx.text(x)).unwrap_or_default();
                    (from, to)
                })
                .collect()
        })
        .unwrap_or_default();
    SubRef {
        path,
        anchor,
        amount,
        remap,
        span: ctx.span(n),
    }
}

fn lower_annotation(ctx: &Ctx, n: Node) -> Option<Annotation> {
    let inner = n.named_child(0)?;
    match inner.kind() {
        "temp" => parse_temp(&ctx.text(inner), ctx.span(inner)).map(Annotation::Temp),
        "timer" => parse_timer(&ctx.text(inner), ctx.span(inner)).map(Annotation::Timer),
        "string" => Some(Annotation::Doc {
            text: unquote(&ctx.text(inner)),
            span: ctx.span(inner),
        }),
        "flag" => Some(Annotation::Flag {
            name: ctx
                .field(inner, "name")
                .map(|x| ctx.text(x))
                .unwrap_or_default(),
            span: ctx.span(inner),
        }),
        "link" => lower_link(ctx, inner).map(Annotation::Link),
        _ => None,
    }
}

fn lower_link(ctx: &Ctx, n: Node) -> Option<Link> {
    let span = ctx.span(n);
    let head = n.child(0).map(|c| c.kind());
    match head {
        Some("&by") => Some(Link::By {
            target: lower_node_ref(ctx, ctx.field(n, "target")?),
            span,
        }),
        Some("&during") => Some(Link::During {
            target: lower_node_ref(ctx, ctx.field(n, "target")?),
            span,
        }),
        Some("&lag") => {
            let d = ctx.field(n, "duration")?;
            Some(Link::Lag {
                duration: parse_lag_duration(&ctx.text(d))?,
                span,
            })
        }
        _ => None,
    }
}
