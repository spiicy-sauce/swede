//! Rust bindings for the Swede tree-sitter grammar.
//!
//! ```
//! let mut parser = tree_sitter::Parser::new();
//! parser.set_language(&tree_sitter_swede::LANGUAGE.into()).unwrap();
//! ```

use tree_sitter_language::LanguageFn;

extern "C" {
    fn tree_sitter_swede() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for Swede.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_swede) };
