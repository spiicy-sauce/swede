#!/usr/bin/env bash
#
# close-grammar-drift.sh — keep the Zed extension in sync with the
# tree-sitter-swede grammar (which lives in the `tree-sitter-swede/`
# subdirectory of this same repo). Two jobs, in order:
#
#   1. Validate the extension's tree-sitter query files
#      (`languages/swede/*.scm`) compile against the grammar. A query that
#      references a node type the grammar no longer defines fails loudly here
#      rather than silently breaking the editor: Zed drops a language whose
#      queries will not compile.
#
#   2. Sync the [grammars.swede] `commit` in extension.toml to the repo's
#      current HEAD. Zed fetches the grammar by this exact SHA and caches the
#      compiled result, so it must be bumped (and pushed) whenever the grammar
#      changes.
#
# Run after committing (and pushing) grammar changes.
# Usage: editors/zed/close-grammar-drift.sh
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
grammar_dir="$repo_root/tree-sitter-swede"
toml="$script_dir/extension.toml"
lang_dir="$script_dir/languages/swede"

# Find a tree-sitter CLI: one on PATH, else the copy vendored in the sibling
# neep grammar's node_modules (used during development).
if command -v tree-sitter >/dev/null 2>&1; then
  ts="tree-sitter"
elif [ -x "$repo_root/../neep/tree-sitter-neep/node_modules/.bin/tree-sitter" ]; then
  ts="$repo_root/../neep/tree-sitter-neep/node_modules/.bin/tree-sitter"
else
  echo "error: no tree-sitter CLI found (install tree-sitter-cli, or run \`npm i tree-sitter-cli\`)" >&2
  exit 1
fi

# ── 1. Validate the extension's queries against the grammar ─────────────
# `tree-sitter query` resolves the grammar from the grammar directory and exits
# non-zero if a query references a node type the grammar does not define. It
# only needs a file to run against; an empty one is enough to compile.
empty="$(mktemp)"
trap 'rm -f "$empty"' EXIT

drift=0
for query in "$lang_dir"/*.scm; do
  if out="$(cd "$grammar_dir" && "$ts" query "$query" "$empty" 2>&1 >/dev/null)"; then
    echo "ok: $(basename "$query") compiles against the grammar"
  else
    echo "DRIFT: $(basename "$query") does not compile against the grammar:" >&2
    printf '%s\n' "$out" \
      | grep -vE 'not configured any parser|Please run|configuration file|language grammars' \
      | sed '/^[[:space:]]*$/d; s/^/    /' >&2
    drift=1
  fi
done

if [ "$drift" -ne 0 ]; then
  echo >&2
  echo "error: fix the query file(s) above to match the grammar, then re-run" >&2
  exit 1
fi

# ── 2. Sync the pinned grammar commit ──────────────────────────────────
sha="$(git -C "$repo_root" rev-parse HEAD)"

# HEAD only reflects committed work: warn if the tree is dirty.
if ! git -C "$repo_root" diff --quiet -- "$grammar_dir" \
   || ! git -C "$repo_root" diff --cached --quiet -- "$grammar_dir"; then
  echo "warning: tree-sitter-swede has uncommitted changes; commit them first so HEAD reflects the grammar" >&2
fi

# Zed pulls the SHA from the remote, so it must be pushed to be fetchable.
if [ -z "$(git -C "$repo_root" branch -r --contains "$sha" 2>/dev/null)" ]; then
  echo "warning: $sha is not on any remote branch; push so Zed can fetch it" >&2
fi

old="$(awk -F'"' '
  /^\[/ { in_section = ($0 == "[grammars.swede]") }
  in_section && /^commit[[:space:]]*=/ { print $2; exit }
' "$toml")"

if [ "$old" = "$sha" ]; then
  echo "extension.toml already at $sha"
  exit 0
fi

awk -v sha="$sha" '
  /^\[/ { in_section = ($0 == "[grammars.swede]") }
  in_section && /^commit[[:space:]]*=/ { print "commit = \"" sha "\""; next }
  { print }
' "$toml" >"$toml.tmp" && mv "$toml.tmp" "$toml"

echo "updated extension.toml grammar commit: ${old:-<none>} -> $sha"
