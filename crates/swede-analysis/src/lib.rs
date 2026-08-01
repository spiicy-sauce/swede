//! Pure Swede analysis for editors: byte-span → LSP position mapping,
//! diagnostics, document symbols, and go-to-definition. No async / server
//! state, so it is unit-testable directly and reusable in-process.

use lsp_types::{
    Diagnostic, DiagnosticSeverity, DocumentSymbol, GotoDefinitionResponse, Location,
    NumberOrString, Position, Range, SymbolKind, TextEdit, Url,
};
use swede_semantics::{validate_source, Severity};
use swede_syntax::*;

/// Maps byte offsets (what spans use) to LSP positions, where `character` is a
/// UTF-16 code-unit offset within the line.
pub struct PositionMapper<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> PositionMapper<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        PositionMapper { text, line_starts }
    }

    pub fn position(&self, byte: usize) -> Position {
        let byte = byte.min(self.text.len());
        let line = match self.line_starts.binary_search(&byte) {
            Ok(line) => line,
            Err(next) => next - 1,
        };
        let line_start = self.line_starts[line];
        let character = self.text.get(line_start..byte).map(|s| s.encode_utf16().count()).unwrap_or(0);
        Position { line: line as u32, character: character as u32 }
    }

    pub fn range(&self, start: usize, end: usize) -> Range {
        Range { start: self.position(start), end: self.position(end) }
    }

    /// The byte offset for an LSP position (the inverse of [`position`]).
    pub fn offset(&self, pos: Position) -> usize {
        let line = pos.line as usize;
        let Some(&line_start) = self.line_starts.get(line) else {
            return self.text.len();
        };
        let mut utf16 = 0u32;
        let mut byte = line_start;
        for ch in self.text[line_start..].chars() {
            if ch == '\n' || utf16 >= pos.character {
                break;
            }
            utf16 += ch.len_utf16() as u32;
            byte += ch.len_utf8();
        }
        byte
    }
}

/// Coded diagnostics as LSP `Diagnostic`s.
pub fn diagnostics(text: &str) -> Vec<Diagnostic> {
    let mapper = PositionMapper::new(text);
    validate_source(text)
        .into_iter()
        .map(|d| Diagnostic {
            range: mapper.range(d.span.start_byte, d.span.end_byte.max(d.span.start_byte + 1)),
            severity: Some(match d.severity {
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
            }),
            code: Some(NumberOrString::String(d.code)),
            source: Some("swede".to_string()),
            message: d.message,
            ..Default::default()
        })
        .collect()
}

/// A single whole-document edit that replaces the file with its formatted form.
/// Empty when the file is already formatted (or has syntax errors).
pub fn formatting(text: &str) -> Vec<TextEdit> {
    let formatted = swede_fmt::format_source(text);
    if formatted == text {
        return Vec::new();
    }
    let mapper = PositionMapper::new(text);
    vec![TextEdit {
        range: Range { start: Position { line: 0, character: 0 }, end: mapper.position(text.len()) },
        new_text: formatted,
    }]
}

fn verb_of(e: &Expr) -> String {
    match e {
        Expr::Call(c) => c.verb.clone(),
        Expr::SubRef(s) => format!("<+{}>", s.path.join("/")),
        Expr::NodeRef(r) => format!("= {}", r.name),
    }
}

#[allow(deprecated)] // the `deprecated` field is itself deprecated but required
fn symbol(name: String, detail: Option<String>, kind: SymbolKind, range: Range, sel: Range) -> DocumentSymbol {
    DocumentSymbol {
        name,
        detail,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range: sel,
        children: None,
    }
}

/// The recipe/menu, its bases, and its named nodes, for the outline panel.
pub fn document_symbols(text: &str) -> Vec<DocumentSymbol> {
    let mapper = PositionMapper::new(text);
    let m = |s: Span| mapper.range(s.start_byte, s.end_byte);
    match parse(text).file {
        Some(File::Recipe(r)) => {
            let mut children = Vec::new();
            for b in &r.bases {
                children.push(symbol(b.name.clone(), Some(b.amount.raw()), SymbolKind::CONSTANT, m(b.span), m(b.span)));
            }
            for binding in &r.bindings {
                let detail = verb_of(&binding.value);
                for o in &binding.outputs {
                    children.push(symbol(
                        o.name.clone(),
                        Some(detail.clone()),
                        SymbolKind::VARIABLE,
                        m(binding.span),
                        m(o.span),
                    ));
                }
            }
            let mut root = symbol(r.name.clone(), Some("recipe".into()), SymbolKind::MODULE, m(r.span), m(r.name_span));
            root.children = Some(children);
            vec![root]
        }
        Some(File::Menu(menu)) => {
            let children = menu
                .entries
                .iter()
                .map(|e| symbol(e.alias.clone(), None, SymbolKind::VARIABLE, m(e.span), m(e.span)))
                .collect();
            let mut root = symbol(menu.name.clone(), Some("menu".into()), SymbolKind::MODULE, m(menu.span), m(menu.name_span));
            root.children = Some(children);
            vec![root]
        }
        None => Vec::new(),
    }
}

/// Jump from a node reference to the binding that produces it.
pub fn definition(uri: &Url, text: &str, pos: Position) -> Option<GotoDefinitionResponse> {
    let mapper = PositionMapper::new(text);
    let byte = mapper.offset(pos);
    let File::Recipe(r) = parse(text).file? else {
        return None;
    };

    // find the node_ref whose span covers the cursor
    let mut refs = Vec::new();
    for b in &r.bindings {
        collect_refs(&b.value, &mut refs);
    }
    let target = refs.iter().find(|(_, s)| byte >= s.start_byte && byte <= s.end_byte)?;

    // find the binding output that defines it
    for b in &r.bindings {
        for o in &b.outputs {
            if o.name == target.0 {
                return Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range: mapper.range(o.span.start_byte, o.span.end_byte),
                }));
            }
        }
    }
    None
}

fn collect_refs(e: &Expr, out: &mut Vec<(String, Span)>) {
    match e {
        Expr::Call(c) => {
            for arg in &c.args {
                match arg {
                    Arg::Call(cc) => collect_refs(&Expr::Call(cc.clone()), out),
                    Arg::NodeRef(n) => out.push((n.name.clone(), n.span)),
                    _ => {}
                }
            }
            for ei in &c.equipment {
                if let EquipItem::NodeRef(n) = ei {
                    out.push((n.name.clone(), n.span));
                }
            }
        }
        Expr::NodeRef(n) => out.push((n.name.clone(), n.span)),
        Expr::SubRef(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALAD: &str = include_str!("../../../fixtures/valid/snow_pea_salad.swede");

    #[test]
    fn diagnostics_surface_w002() {
        let d = diagnostics(SALAD);
        assert!(d.iter().any(|x| x.code == Some(NumberOrString::String("W002".into()))));
        assert!(d.iter().all(|x| x.source.as_deref() == Some("swede")));
    }

    #[test]
    fn symbols_list_nodes() {
        let syms = document_symbols(SALAD);
        assert_eq!(syms.len(), 1);
        let kids = syms[0].children.as_ref().unwrap();
        assert!(kids.iter().any(|s| s.name == "plate"));
        assert!(kids.iter().any(|s| s.name == "cooled"));
    }

    #[test]
    fn goto_definition_finds_producer() {
        // in `tossed = toss(base, ribbons)`, jumping on `base` → the `base` binding
        let uri = Url::parse("file:///x.swede").unwrap();
        let idx = SALAD.find("toss(base").unwrap() + "toss(".len();
        let mapper = PositionMapper::new(SALAD);
        let pos = mapper.position(idx + 1);
        match definition(&uri, SALAD, pos) {
            Some(GotoDefinitionResponse::Scalar(loc)) => {
                let off = mapper.offset(loc.range.start);
                assert!(SALAD[off..].starts_with("base"));
            }
            other => panic!("expected a definition, got {other:?}"),
        }
    }
}
