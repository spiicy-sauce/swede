# Swede — Zed extension

Syntax highlighting and a language server (`swede-lsp`) for `.swede` recipe
files.

## What you get

- **Highlighting** from the bundled tree-sitter queries
  (`languages/swede/highlights.scm`): verbs, node names, ingredients, equipment,
  bases, metadata, amounts, timers/temps, flags, temporal links, and strings.
- **Live diagnostics** — syntax errors plus the coded semantic errors and
  warnings (`E001`–`E006`/`E008`, `W001`–`W004`) via `swede-lsp`.
- **Document outline** of the recipe/menu name, its bases, and its named nodes.
- **Go to definition** — jump from a node reference to the binding that
  produces it.
- **Formatting** — `swede-lsp` aligns the `=` columns; run **`editor: Format`**
  or enable format-on-save for Swede in your Zed settings:

  ```json
  "languages": { "Swede": { "format_on_save": "on" } }
  ```

- **Bracket matching / autoclose** for `[...]` and `(...)`.

## Setup

### Install the language server

`swede-lsp` must be on your `PATH` (the extension locates it with
`worktree.which("swede-lsp")`):

```sh
cargo install --path crates/swede-lsp
```

### The grammar

Zed fetches and compiles tree-sitter grammars from git. The Swede grammar lives
in the [`tree-sitter-swede/`](../../tree-sitter-swede) subdirectory of this repo,
referenced from [`extension.toml`](./extension.toml):

```toml
[grammars.swede]
repository = "https://github.com/spiicy-sauce/swede"
commit = "<full commit sha>"
path = "tree-sitter-swede"
```

- `repository` is this repo's public **clone URL** (HTTPS, anonymous). Update it
  if you host the repo elsewhere.
- `commit` must be a **full, immutable SHA that is pushed to the remote**. A
  branch name won't work: Zed caches the compiled grammar keyed by this value,
  so it would never pick up later pushes. `close-grammar-drift.sh` bumps it to
  the current HEAD for you.
- `path` points Zed at the grammar's subdirectory. If your Zed predates monorepo
  grammar `path` support, publish `tree-sitter-swede/` as its own repo and drop
  the `path` line.

So this extension works, the repo must be pushed somewhere Zed can fetch it, and
the pinned `commit` must exist on that remote.

### Install as a dev extension

In Zed: **Extensions → Install Dev Extension…** and choose this `editors/zed`
directory. Open any `.swede` file.

## Development

Keep the queries in sync with the grammar and refresh the pinned commit:

```sh
editors/zed/close-grammar-drift.sh
```

It validates every `languages/swede/*.scm` against the grammar (Zed silently
drops a language whose queries don't compile) and rewrites the `[grammars.swede]`
commit to the repo's current HEAD, warning if that SHA isn't pushed yet.

You can also validate a query by hand:

```sh
tree-sitter query editors/zed/languages/swede/highlights.scm fixtures/valid/miso_chicken_and_rice.swede
```

## Note on capture names

`languages/swede/highlights.scm` uses **Zed's** capture vocabulary
(`@variable.special`, `@constant`, `@keyword`, …), which differs slightly from
the standard-tree-sitter captures in
[`tree-sitter-swede/queries/highlights.scm`](../../tree-sitter-swede/queries/highlights.scm)
(`@variable.parameter`, `@constant.builtin`, `@keyword.directive`). Keep the two
in step when you change highlighting.
