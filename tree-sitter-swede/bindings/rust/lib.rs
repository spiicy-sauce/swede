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

/// The syntax-highlighting query for Swede.
pub const HIGHLIGHTS_QUERY: &str = include_str!("../../queries/highlights.scm");

/// The language-injection query for Swede.
pub const INJECTIONS_QUERY: &str = include_str!("../../queries/injections.scm");

#[cfg(test)]
mod tests {
    #[test]
    fn queries_compile_against_the_grammar() {
        let lang: tree_sitter::Language = super::LANGUAGE.into();
        tree_sitter::Query::new(&lang, super::HIGHLIGHTS_QUERY).expect("highlights.scm compiles");
        tree_sitter::Query::new(&lang, super::INJECTIONS_QUERY).expect("injections.scm compiles");
    }
}
