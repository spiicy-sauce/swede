# Swede — build one-pager

**What:** a working implementation of the Swede graph-first recipe language, built
top-to-bottom from the v0.2 spec. Rust workspace, tree-sitter grammar, mirroring
the sibling `neep` project's stack. ~3,500 hand-written lines, 20 tests green,
clippy clean.

## What's built and tested (all four planned stages)

| Stage | Deliverable | Status | Acceptance test |
|-------|-------------|--------|-----------------|
| **0** | Parser + validator | ✅ done | Golden corpus: §6–8 validate exactly as annotated (only `W002` on the salad's `cooled`); negative fixtures flag `E001`–`E005`, `W003`, `W004` |
| **1** | Table renderer | ✅ done | Flow table + depth-column grid; basis `%` resolves (`50%* flour → 400g`, `72% → 576g`); split fork band, preheat header strip |
| **2** | Scheduler stab + timeline | ✅ done | `preheat` back-placed to finish by `assembled` (starts at +1m, not T=0) |
| **3** | Menu + cross-recipe scheduling | ✅ done | `tuesday` menu aligns both recipes to serve at T=0; walnuts proposed inside the chicken's oven (`'baked' and 'toasted' share !oven at 350°`) |

Run any of it:

```bash
cargo test
cargo run -p swede-cli -- validate fixtures/valid/snow_pea_salad.swede
cargo run -p swede-cli -- render   fixtures/valid/miso_chicken_and_rice.swede --format grid
cargo run -p swede-cli -- schedule fixtures/valid/tuesday.menu.swede
```

## Diagnostics implemented (SPEC §4)

Static (v1): `E001` unknown ref · `E002` reuse · `E003` ingredient redeclared ·
`E004` forward data ref · `E005` link contradiction · `E006` menu alias ·
`E008` rebinding / reserved keyword (added; the spec's "single-assignment" rule
had no code) · `W001` orphan · `W002` unbounded passive · `W003` split sum ·
`W004` constituent sum. Every diagnostic carries a code, a `row:col` span, and a
message. `E000` wraps syntax errors.

Deferred: `E007` (sub-recipe basis unit) needs a sub-recipe loader. `W005`/`W006`
are scheduler-era; `W005` is effectively delivered as the co-residency check, and
`W006` awaits critical-path analysis.

## The C1 fix works

The bread fixture (`fifty_fifty_loaf`) validates **clean**: ingredient
constituents `50%* + 50%*` sum to 100%, and the `<+bread/poolish>[20%* flour]`
sub-recipe constituent is excluded from that sum, exactly as we agreed. The
`w004_constituents` negative fixture (`60%* + 50%*`) correctly fires `W004`.

## Decisions made while building (beyond the agreed spec edits)

1. **`&lag(0)` vs `duration = number unit`** — the §7 fixture uses `&lag(0)` but
   the EBNF requires a unit. Resolved: bare `0` is a legal zero-duration; every
   other duration needs a unit. (Spec inconsistency; flagged for a §3 tweak.)
2. **`ice_water` in the salad** — the spec fixture wrote `soak(...; ice_water)`
   with a bare, undeclared name; the validator correctly flagged `E001`. An ice
   bath is equipment, so I changed it to `!ice_water` and synced SPEC §7. This is
   the strict, no-implicit-equipment reading.
3. **Equipment in arg position** — `preheat(!oven {350F})` puts the oven in arg
   position (no `;`). The grammar allows `equip` as an arg; the model unifies
   arg-position and `;`-position equipment.
4. **Timer ranges** — fixtures write `6-10m` (unit once); the EBNF implies
   `6m-10m`. I support the unit-once form the fixtures actually use.

## Known limitations / honest gaps

- **Scheduler is a stab, as scoped.** ASAP + `&by` back-placement + menu
  alignment work. Attention-capacity serialization of overlapping `@` blocks is
  *reported* (active vs elapsed totals) but not yet *enforced*; `&during`
  containment and `&lag` tightening are not solved. This is the deferred power
  feature (your G2).
- **Co-residency** works because equipment is propagated through equipment-state
  nodes (`bake` inherits `!oven` from the `hot` it consumes). It's a proposal
  detector, not a packing solver.
- **Grid renderer** is correct and deterministic but plain (no box borders); the
  fork band reads as repeated leaf rows rather than drawn connectors. The flow
  table is the polished projection.
- **Sub-recipes aren't loaded**, so `<+bread/poolish>` is treated as an opaque
  leaf (no `E007`, no blended-constituent subtraction). Needs a file loader.
- **Basis resolution** resolves `%`/`%*` against the basis total; full Neep-style
  sub-recipe flour subtraction is deferred with the loader.
- **Prose renderer**: not built (non-goal per your G3).

## Where the code lives

`SPEC.md` (v0.2, with §12 changelog) · `README.md` (build/run) · `fixtures/valid`
(the three spec recipes + menu) · `fixtures/invalid` (negative corpus) · crates as
in the README table.

## Suggested next steps

1. Sub-recipe loader → unlocks `E007` + true blended-constituent bread math.
2. Enforce single-cook attention (serialize overlapping `@` blocks) so the
   timeline is a real plan, not just dependency-earliest times.
3. Grid renderer box-drawing pass (borders + drawn fork connectors) to match the
   §9 reference layout precisely.
4. An LSP crate (`swede-lsp`) for editor diagnostics, mirroring `neep-lsp`.
