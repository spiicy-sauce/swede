# Swede

A graph-first recipe language. A `.swede` file is the dependency graph of a
recipe, written as named bindings; tables and cooking timelines are *projections*
of that graph. See [SPEC.md](SPEC.md) for the language design (v0.2).

## Layout

Rust workspace, mirroring the sibling `neep` project (tree-sitter grammar +
crate-per-concern).

| Crate | Role |
|-------|------|
| `tree-sitter-swede` | Grammar (`grammar.js`) + generated parser + Rust binding |
| `swede-syntax` | Parse tree → typed AST; amount/timer/temp classification |
| `swede-semantics` | Static validation (coded diagnostics), basis resolution, shared graph model |
| `swede-render` | Table projections: Markdown flow table + monospace grid |
| `swede-schedule` | Single-cook scheduler stab + timeline; menu scheduling |
| `swede-cli` | `swede` binary: `validate` / `render` / `schedule` / `parse` |

The generated `tree-sitter-swede/src/parser.c` is committed, so `cargo build`
works without the tree-sitter CLI. Regenerate only if you edit `grammar.js`:

```bash
cd tree-sitter-swede && npx tree-sitter-cli generate
```

## Build & test

```bash
cargo test          # 20 tests: parser, validator golden corpus, renderer, scheduler
cargo clippy --workspace --all-targets
```

## Use

```bash
cargo run -p swede-cli -- validate fixtures/valid/snow_pea_salad.swede
cargo run -p swede-cli -- render   fixtures/valid/miso_chicken_and_rice.swede --format flow
cargo run -p swede-cli -- render   fixtures/valid/miso_chicken_and_rice.swede --format grid
cargo run -p swede-cli -- schedule fixtures/valid/tuesday.menu.swede
```

## Status

Stages 0–3 of [SPEC.md](SPEC.md) §11 are implemented and tested. See
[ONE_PAGER.md](ONE_PAGER.md) for what is done, tested, and deferred.
