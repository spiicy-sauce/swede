//! `swede-syntax` — parse Swede source into a typed AST.
//!
//! The tree-sitter grammar (`tree-sitter-swede`) produces a concrete parse
//! tree; this crate lowers it into [`ast`] and classifies amounts, timers, and
//! temperatures. Semantic validation lives in `swede-semantics`.

pub mod amount;
pub mod ast;
mod lower;

pub use ast::*;
pub use lower::{lower, Lowered, SyntaxError};

/// Parse Swede source, returning the AST (if any) and syntax errors.
pub fn parse(source: &str) -> Lowered {
    lower(source)
}
