# Swede — a graph-first recipe language

**Status:** draft v0.2 · spec for implementation planning
**Working name:** *swede* (a neep and a swede are both turnips: same vegetable,
different name; same recipe, different representation). Rename freely; nothing
below depends on it. Suggested extension: `.swede`.

Swede is a graph-first recipe language. A Swede file is the dependency graph of a
recipe, written as named bindings. A Cooking-for-Engineers-style table and a
cooking timeline are *projections* of the graph. Swede is the stored, validated
form; the projections are renderers.

Design lineage: RxOL/Cocina (postfix recipe trees, Mundie 1985), Chu's Tabular
Recipe Notation, make-style dependency inference, and PDM precedence links
(finish-to-start is not the only ordering relationship). Swede's contribution is
combining named dataflow bindings, split (multi-)outputs, active vs. passive
time, temporal links (`&by`, `&during`, `&lag`), equipment state as nodes,
baker's-percentage bases, and cross-recipe scheduling at the menu level.

> **What changed from v0.1 (see the changelog in §12 for the reasoning):**
> metadata now uses a leading-`:` sigil per line; the trailing string on a call
> is removed (strings are annotations, or you name the node); the bread fixture
> and W004 now state the constituent-subtraction rule explicitly; the scheduler
> is marked clearly as an informative, later-phase power feature; cross-recipe
> equipment identity is defined (verbatim-equal names are the same unit); Neep
> migration is out of scope (conversions, if any, are manual).

---

## 1. Core model

- A recipe is a set of **bindings**. Each binding names one or more **nodes**
  produced by applying a **verb** to inputs.
- Inputs are **ingredient literals** (leaves, declared at first use), references
  to earlier nodes, nested calls (anonymous nodes), or equipment.
- **Verbs are opaque.** The validator checks flow (existence, ordering, linear
  consumption, amount arithmetic), never culinary semantics. The verb vocabulary
  is open.
- **Linear consumption:** every node is consumed by exactly one later
  expression, or is the recipe's terminal. Consuming twice is an error (insert an
  explicit split); consuming zero times is a warning (orphan). This is what makes
  the graph a tree rooted at the terminal, which is what the table projection
  relies on (§9).
- The **final binding** in a file is the recipe's terminal node (conventionally
  named `plate`, not required).
- Time comes in two kinds: `@` (active, claims the cook's attention) and `~`
  (passive, an unattended window other work can pack into).
- **Temporal links** (`&by`, `&during`, `&lag`) constrain *scheduling*, not
  dataflow. They are the only construct allowed to reference forward.

## 2. Lexical elements

```
identifier   snake_case: [a-z][a-z0-9_]*         (node names, verbs, ingredients, equipment)
path         slash-separated identifiers          bread/poolish
number       int | decimal | fraction             2, 1.5, .5, 1/2, 1 1/2
string       double-quoted, no escapes needed beyond \"   free prose; never tokenized
meta line    a line whose first non-space char is `:`     `:serves 3-4`
comment      // to end of line                     (not scanned inside strings or meta values)
whitespace   insignificant except inside strings and meta values; newlines end
             nothing (bindings end at the start of the next `name =` / keyword,
             so multi-line calls are fine)
```

Unit abbreviations (canonical long forms in parentheses): `g kg ml l t (tsp)
T (tbsp) c (cup) floz oz lb in cm` plus bare counts. `T`/`t` case is significant.

**Metadata sigil.** A line whose first non-whitespace character is `:` is a
metadata line: `:` then a key (`identifier`) then the value, read **verbatim to
end of line**. Because the value is a raw span, characters that would otherwise
tokenize (notably `//` in a `source` URL) are safe inside it, and comments are
not scanned there. This is the only place a raw-to-EOL span appears outside
strings.

## 3. Grammar (EBNF)

```ebnf
file          = recipe_decl , { statement } ;
recipe_decl   = "recipe" , identifier , _nl , { meta_line } ;
meta_line     = ":" , identifier , meta_text , _nl ;
                (* key names reuse the recognized metadata keys:
                   serves, yields, time, source, source_uri, source_author,
                   source_date, tags, rating, ... ; meta_text is verbatim to _nl *)
statement     = basis_decl | binding | meta_line ;
                (* meta_line may also appear later in the file, not only in the header *)
basis_decl    = "basis" , identifier , "=" , amount ;
binding       = output_list , "=" , expr , { annotation } ;
output_list   = output , { "," , output } ;
output        = identifier , [ "[" , amount , "]" ] ;
                (* per-output amounts allowed on splits; see W003 *)
expr          = call | sub_ref | node_ref ;
call          = verb , "(" , [ arg_list ] , [ ";" , equip_list ] , ")" ;
                (* NOTE: a call has no trailing string. To qualify an operation,
                   name its node and annotate it (see §4, "No inline
                   call-strings"). *)
verb          = identifier ;
arg_list      = arg , { "," , arg } ;
arg           = call | node_ref | ingredient | equip ;
equip_list    = equip_item , { "," , equip_item } ;
equip_item    = equip | node_ref ;
                (* node_ref in equipment position = equipment-state node, e.g. `hot` *)
equip         = "!" , identifier , [ temp ] ;
ingredient    = identifier , "[" , amount , "]" , [ string ] ;
                (* first use declares; the string is a purchase/prep qualifier.
                   Unambiguous here because it sits inside the arg list. *)
node_ref      = identifier , [ "." , identifier ] ;
                (* dotted form only valid in menu scope: alias.node *)
sub_ref       = "<+" , path , [ "#" , identifier ] , ">" ,
                [ "[" , amount , "]" ] , [ remap ] ;
                (* sub-recipe reference. #anchor = partial execution through that
                   node. Amount scales the sub-recipe, including basis-relative
                   amounts: <+bread/poolish>[20%* flour] *)
remap         = "(" , identifier , "->" , identifier ,
                { "," , identifier , "->" , identifier } , ")" ;
amount        = amount_term , { "+" , amount_term }
              | "~"                                  (* to taste *)
              | "?" ;                                (* stated but unquantified *)
amount_term   = number , [ unit ]
              | number , "%" , [ "*" ] , identifier ; (* basis-relative; * = constituent *)
annotation    = temp | timer | string | flag | link ;
temp          = "{" , number , ( "F" | "C" ) , "}" ;
timer         = "@" , duration_range                 (* active *)
              | "~" , duration_range                 (* passive *)
              | "~?" ;                               (* passive, unbounded *)
duration_range= duration , [ "-" , duration ] ;
duration      = number , ( "s" | "m" | "h" | "d" ) ;
flag          = "[" , identifier , "]" ;             (* e.g. [covered]; opaque, shown in renders *)
link          = "&by"     , "(" , node_ref , ")"
              | "&during" , "(" , node_ref , ")"
              | "&lag"    , "(" , duration , ")" ;
```

### Menu files

```ebnf
menu_file  = "menu" , identifier , _nl , { meta_line | menu_stmt } ;
menu_stmt  = identifier , "=" , ( sub_ref | identifier ) , { link } ;
             (* the identifier binds an alias; link targets use alias.node *)
```

All top-level binding names of a recipe are addressable from menu scope as
`alias.node`. If a link targets a bare alias, it means that recipe's terminal
node.

## 4. Static semantics (validator rules)

Each rule gets a diagnostic code; severities are Error (rejects) / Warning
(reports). **All rules in this table are static** (computable from the graph
alone, with no schedule) except W005 and W006, which are marked scheduler-era.

| Code | Rule |
|------|------|
| E001 | Unknown reference: a `node_ref` argument doesn't resolve to an earlier binding, ingredient, basis, or equipment name. |
| E002 | Reuse after consumption: a node referenced as a data input more than once. Message must suggest an explicit split binding. A second bare reference to an ingredient name is the only reuse path there is, so this is what catches an un-split ingredient. |
| E003 | Ingredient redeclared: the same ingredient identifier given an amount literal twice. One declaration per ingredient; division is done via split bindings, not repeated literals. |
| E004 | Forward data reference: data inputs (args, equipment) may only reference earlier bindings. Only `&link` targets may reference forward. |
| E005 | Link contradiction: a `&by(N)`/`&during(N)` whose target `N` is a transitive data *ancestor* of this node (the constraint is unsatisfiable given dataflow order). |
| E006 | Menu reference to a non-existent alias or non-exported node. |
| E007 | Sub-recipe basis unit mismatch. |
| W001 | Orphan: a node with no consumer that is not the terminal. |
| W002 | Unbounded passive (`~?`) on a path to the terminal or to any link target: the scheduler cannot bound the plan. (Static: reachability, no schedule needed.) |
| W003 | Split amounts don't sum: `a[x], b[y] = divide(thing[z])` where x + y != z (units permitting comparison). |
| W004 | Ingredient constituent percentages of a basis don't sum to 100% (see "Bases" below for how sub-recipe constituents are handled). |
| W005 | *(scheduler-era)* Equipment temperature conflict: two nodes claim the same `!equipment` at different `{temp}` in windows the scheduler cannot separate. |
| W006 | *(scheduler-era)* Doneness-only step: a binding with neither timer nor doneness string on the critical path (plan quality, not correctness). |

**No inline call-strings.** A call carries no trailing qualifier string. Every
string is either a purchase/prep qualifier on an ingredient literal (inside the
arg list, e.g. `chicken_thighs[1lb] "boneless"`) or a **binding annotation** on a
named node. To qualify an *operation* (what would have been a nested call
qualifier), give the operation a name and annotate it:

```
// instead of:  plate = fold(tossed, slice(avocado[1]) "thin")
sliced_avo = slice(avocado[1]) "thin"
plate      = fold(tossed, sliced_avo) "eat right away"
```

This removes the v0.1 ambiguity where a string after `)` could bind either to
the call or to the node, and it keeps qualifier prose attached to a single,
addressable node (which the renderers want anyway).

**Ingredient totals and division.** An ingredient literal's amount is its full
purchased total. To use an ingredient in two places, split it explicitly:

```
butter_a[1T], butter_b[1T] = divide(butter[2T])
```

`divide` is not special; any verb may produce multiple outputs. W003 checks
amount arithmetic whenever per-output amounts are present.

**Bases.** `basis flour = 800g` declares a scaling basis (the amount on the right
is the basis amount; this is how a basis amount is declared). `%` amounts resolve
against it. `%*` marks **constituents** whose shares must total 100% (W004).

Two kinds of `%*` appear against a basis and they are accounted differently:

1. **Ingredient constituents** (`white_flour[50%* flour]`): these define how the
   basis quantity is split among ingredients. Their percentages must sum to 100%
   (W004).
2. **Sub-recipe constituents** (`<+bread/poolish>[20%* flour]`): a sub-recipe
   that draws part of the basis. Its share is **subtracted** from the ingredient
   constituents rather than added on top of them: the flour inside the poolish is
   the same flour, already counted in the 100%, just routed through the
   sub-recipe. The render shows one blended item per ingredient plus a "for the
   <sub-recipe>" breakdown, and the recipe's share plus the sub-recipe's share of
   any constituent sum to that constituent's 100% line.

So W004 sums the **ingredient** constituents only. A sub-recipe constituent does
not push the total past 100%. `%` (without `*`) is a plain percentage of the
basis total and is not part of the constituent sum. Sub-recipe references may
scale by basis-relative amounts. Bases and baker's percentages are first-class
and are the part of the model held most stable.

**No rebinding.** A name is bound once (SSA-style). Evolving state gets versioned
names (`dough`, `dough_folded`) or, in v2, a loop construct (§10).

## 5. Scheduling semantics (informative, later phase)

> This section describes the eventual scheduler and timeline. It is a
> **future-looking power feature**, not a v1 deliverable, and nothing here is a
> language rule. The only hard requirement it places on the language is that a
> validated Swede graph carry enough structure for a straightforward algorithmic
> scheduler to consume it. v1 ships a small stab (single cook, greedy) to prove
> that out (§11).

- Every data edge is a finish-to-start dependency.
- `@d` blocks claim the attention resource (capacity = number of cooks, default
  1). Two `@` blocks may not overlap under capacity 1.
- `~d` blocks occupy no attention; they open windows other work packs into.
- `&by(N)`: this node's finish <= N's finish; placement preference is
  as-late-as-possible (this is what makes `preheat` start at T-12m instead of
  T=0).
- `&during(N)`: this node's interval is contained in N's interval.
- `&lag(d)`: at most `d` may elapse between this node's finish and its consumer's
  start. `&lag(0)` = "eat right away", whipped-egg-white immediacy, etc.
- **Equipment identity.** Nodes claiming the same `!name` contend for the same
  unit. **Verbatim-equal equipment names are the same physical unit**, including
  across recipes in a menu: `!oven` in one recipe and `!oven` in another are the
  same oven. (A future aliasing / multi-unit scheme, e.g. `!oven_2` or a
  menu-level equipment declaration, can refine this if a kitchen has two; not
  needed for v1.)
- Same `!name` + equal `{temp}` + overlapping windows: the scheduler may
  *propose* co-residency (walnuts riding the chicken's oven), never force it.
  Same `!name` + different `{temp}` in inseparable windows: W005.
- Menu scope: by default, each recipe's terminal aligns to the latest terminal
  among menu entries; explicit `&by(alias.node)` overrides.

The scheduler is a separate component consuming validated graphs. First target:
single cook, greedy list scheduling with backward passes for `&by`/`&lag`.
Optimality is not required; legibility of the emitted plan is.

## 6. Worked example 1 — Miso Chicken and Rice (oven variant)

Conformance fixture. Must parse and validate clean.

```
recipe miso_chicken_and_rice
  :serves 3-4
  :tags chicken, rice, one-pot, weeknight
  :source https://smittenkitchen.com/2026/02/miso-chicken-and-rice/

marinade  = mix(soy_sauce[2T], oyster_sauce[1T], shaoxing_wine[1T], miso[2T],
                kosher_salt[.5t], sugar[1t], sesame_oil[.5t], white_pepper[~]) @3m
strips    = cut(chicken_thighs[1lb]) @2m "1-inch strips"
caps      = slice(shiitake_caps[8]) @1m "thin"
coated    = mix(marinade, strips, caps) @5m
whites, greens = slice(scallions[2])
hot       = preheat(!oven {350F}) ~12m  &by(assembled)
assembled = layer(rice[1c], broth[1c + 2T], ginger[.25in] "grated",
                  coated, whites; !baking_dish) @5m
baked     = bake(assembled; hot) ~35-45m [covered] "rice tender, chicken cooked through"
plate     = top(baked, greens) @1m "eat right away"
```

Features exercised: split outputs (`whites, greens`), equipment-state node (`hot`
produced by `preheat`, consumed in `bake`'s equipment position), forward *link*
target (`&by(assembled)` before `assembled` is bound, legal for links, illegal
for data), compound amount (`1c + 2T`), ingredient qualifier strings inside args
(`ginger[.25in] "grated"`), binding-annotation strings (`"1-inch strips"`,
`"eat right away"`), passive vs. active timers, metadata sigil lines.

## 7. Worked example 2 — Snow Pea Salad + menu

```
recipe snow_pea_salad
  :yields 2 mains | 4 sides
  :tags salad, vegan, quick, summer
  :source Smitten Kitchen

toasted    = toast(walnuts[50g]; !sheet_pan, !oven {350F}) ~6-10m "fragrant, darker"
cooled     = cool(toasted) ~?
soaked     = soak(snow_peas[225g]; !ice_water) ~10-20m
ribbons    = slice(pat_dry(drain(soaked))) @4m "thin, lengthwise"
base       = whisk(chop(cooled), olive_oil[60ml], lemon_juice[20ml],
                   kosher_salt[~], black_pepper[~], chile_flakes[~]) @2m
tossed     = toss(base, ribbons) @1m
sliced_avo = slice(avocado[1]) @1m "thin"
plate      = fold(tossed, sliced_avo) @1m  &lag(0) "eat right away"
```

```
menu tuesday
  chicken = miso_chicken_and_rice
  salad   = snow_pea_salad  &by(chicken.plate)
```

Features exercised: nested anonymous calls (`pat_dry(drain(soaked))`), a named
node standing in for what used to be a nested-call qualifier (`sliced_avo`), `~?`
(expects W002), to-taste amounts, `&lag(0)`, menu aliasing and cross-recipe `&by`.
Expected diagnostics: `W002 cooled: unbounded passive on path to terminal`.

## 8. Worked example 3 — basis fragment (bread)

```
recipe fifty_fifty_loaf
  :yields 1 loaf
  :tags bread, sourdough

basis flour = 800g

blend    = mix(white_flour[50%* flour], whole_wheat_flour[50%* flour])
poolish  = <+bread/poolish>[20%* flour]
autolyse = mix(blend, water[72% flour]) ~1-2h
dough    = knead(autolyse, kosher_salt[2% flour], poolish) "smooth, then slap and fold"
// bulk ferment needs the v2 loop construct, see §10:
// bulk  = repeat(4, every ~30m) { fold(dough) } ~4h
```

**Constituent check (this fixture validates clean).** The *ingredient*
constituents `white_flour[50%* flour]` and `whole_wheat_flour[50%* flour]` sum to
100%, satisfying W004. The `<+bread/poolish>[20%* flour]` is a *sub-recipe*
constituent: its 20% of the flour is drawn from that same 50/50 breakdown and
subtracted, not added, so it does not push the constituent total past 100% and
does not trigger W004. This is the basis resolution rule from §4 ("Bases"),
carried over intact. Get this accounting right or the whole book's bakers revolt.

## 9. Projections

**Table (Tabular Recipe Notation) — the priority projection.** Deterministic
algorithm over the graph:

1. Rows = ingredient leaves, in binding order. A split node forks its row into
   one band per product.
2. A node's **entry column** = its depth (longest path from any of its leaves).
3. A node's **cell** spans the rows of all leaves in its subtree (rowspan = leaf
   count). Because linear consumption makes the graph a tree, "subtree" is
   well-defined everywhere except at split points; see rule 5.
4. An ingredient whose first merge is deep gets a colspan from column 0 to its
   entry column.
5. A split product consumed later than its siblings renders as a bypass band
   running beneath intervening cells until its merge column. (Splits are the one
   place the graph is a DAG rather than a tree; the bypass band is how the table
   linearizes it. Band placement is deterministic: order bands by the binding
   order of their consumers.)
6. Nodes with no ingredient subtree (equipment prep like `preheat`) render as
   full-width header strips, annotated with their link (`finish by: assembled`).
7. Cell styling: `@` nodes solid/tinted, `~` nodes dashed, `~?` dashed +
   warning marker.

**Timeline.** Scheduler output (§5) drawn as lanes per recipe plus resource
lanes; the "what's next" feed is a cursor over this. Later phase, with the
scheduler.

**Prose.** Not a current goal. With opaque verbs there is little to render prose
from beyond the verb string and qualifier annotations, so a faithful prose
render is out of scope for now. If prose rendering becomes a goal later, the
intended approach is to **store the original prose alongside the graph** (verbs
stripped into the graph, sentences kept as an attached, non-graph payload) rather
than to reconstruct sentences from opaque verbs.

## 10. Reserved for v2 (design later, reserve syntax now)

- `alt { ... | ... }` — variant/XOR branches (rice cooker vs. oven). v1: separate
  recipes or pick one.
- `repeat(n, every ~d) { ... }` — timed loops (stretch-and-fold). v1: unroll or
  leave in prose.
- `foreach(<=n) { ... }` — batching under equipment capacity (pierogi, 10 at a
  time).
- Byproduct export across recipes (corn cobs -> corn stock) — needs multi-output
  at the *recipe* interface.
- Feedback (`season to taste` cycles) — stays prose forever, most likely.
- `cooks(n)` — attention capacity > 1.
- Condition-triggered loops keyed to budget consumption (risotto's ladle) —
  likely permanent prose, quarantined in a doneness string.

Parsers must reject these keywords as identifiers so v2 can claim them.

## 11. Implementation plan

Scope note: converting existing recipes into Swede is **out of scope**. There is
no `neep2swede` compiler and no book-wide migration. Any conversions are manual,
one at a time, as needed. The plan below builds Swede as a language in its own
right.

Stack: mirror the adjacent `neep` project (Rust workspace, tree-sitter grammar,
crate-per-concern). Suggested crates: `tree-sitter-swede`, `swede-syntax`
(classify amounts, build AST/IR), `swede-semantics` (validation, basis/constituent
resolution), `swede-render` (table now, timeline later), `swede-cli`, and later
`swede-mcp` / `swede-lsp`.

### Stage 0 — Parser + validator

Grammar §3, static rules §4 (E001–E007, W001–W004), diagnostics as structured
output (code, node, span, message; stable codes, unlike Neep's uncoded
diagnostics). Golden-file suite: §6–8 pass exactly as annotated (W002 on §7's
`cooled`, everything else clean, including §8's bread math), plus negative
fixtures for E001–E005. Acceptance: golden suite green.

### Stage 1 — Table renderer

The §9 table algorithm over the validated graph. Acceptance: table renders of
§6–7 show the scallion fork band, the `preheat` header strip, and dashed passive
cells; §8 shows the blended-constituent breakdown with shares summing to 100%.
This is the projection that proves the graph model pays off, so it comes first.

### Stage 2 — Scheduler stab + single-recipe timeline

Small algorithmic scheduler over one recipe's graph: single cook, greedy list
scheduling, backward passes for `&by`/`&lag`, `~` windows packable under `@`
blocks. Emit W002 dynamically confirmed and W006 (now that a critical path
exists). Draw a single-recipe timeline. Acceptance: §6 schedules with `preheat`
back-placed to finish by `assembled` rather than starting at T=0. Keep it small;
this is a feasibility stab, not the full power feature.

### Stage 3 — Menu + cross-recipe scheduling

Menu grammar, §5 semantics, verbatim equipment identity across recipes, W005.
Acceptance: the `tuesday` menu schedules with the salad's walnuts *proposed*
inside the chicken's bake window (same `!oven`, same `{350F}`) and the salad
back-scheduled against `chicken.plate`.

### Deferred / non-goals for now

- **Prose renderer** — non-goal (§9); revisit only if prose becomes a priority,
  via the store-original-prose approach.
- **Rutabaga / MCP integration** — optional and later. If pursued, mirror a small
  tool surface (`swede_validate`, `swede_render(table|timeline)`,
  `swede_schedule(menu)`).
- **Full scheduler** (multi-cook, optimization, `&during` containment solving) —
  power feature beyond the Stage 2 stab.

### Resolved decisions

1. **Migration:** out of scope; conversions are manual.
2. **Verb vocabulary:** open. An optional style-lint registry may come later for
   renderer quality, but verbs stay open and opaque to the validator.
3. **Doneness strings:** opaque in v1. Resist structuring them until v2 pressure
   (risotto-class recipes) forces it.
4. **Menu `&by` default:** global latest-terminal alignment, per-entry override.

## 12. Changelog (v0.1 -> v0.2)

- **Metadata sigil (was A2).** Metadata moved from `key(value)` attributes on the
  `recipe` line to leading-`:` lines with verbatim-to-EOL values. Removes the
  `//`-in-a-source-URL-vs-comment collision and the meta-vs-binding lookahead.
- **No trailing call-string (was A1).** Removed the optional trailing string from
  `call`. Strings are now only ingredient qualifiers (inside args) or binding
  annotations. Nested-call qualifiers are expressed by naming the node. Removes
  the string-binds-to-call-or-node ambiguity.
- **Constituent subtraction made explicit (was C1).** §8 now validates clean; §4
  and W004 state that ingredient constituents sum to 100% while sub-recipe `%*`
  constituents are subtracted, not added. The v0.1 fixture's "intentionally
  triggers W004 unless subtracted" annotation was self-contradictory and is gone.
- **Dropped the parser-topological-sort claim (was C2).** E004 keeps the rule
  (data refs are backward-only) but no longer claims the parser needs no graph
  algorithm; forward/backward ordering is a scheduler implementation detail, not
  a language guarantee.
- **Equipment identity defined (was G1).** Verbatim-equal `!name` are the same
  physical unit, including across recipes in a menu. Aliasing/multi-unit deferred.
- **Scheduler reframed (was G2).** §5 is explicitly informative and a later-phase
  power feature; v1 ships only a small feasibility stab.
- **Prose de-scoped (was G3).** Prose rendering is a non-goal; if revived, keep
  original prose alongside the graph rather than reconstructing it.
- **Neep migration removed (was §11).** No compiler, no book-wide triage; Swede
  stands on its own.
- **Diagnostics classified.** §4 marks which rules are static vs scheduler-era.
```
