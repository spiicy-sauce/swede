; Highlight queries for Swede, using Zed's capture names.
;
; Later patterns override earlier ones for the same node, so specific
; field-qualified captures follow the generic ones.

; ── Comments ─────────────────────────────────────────────────────────
(comment) @comment

; ── Keywords ─────────────────────────────────────────────────────────
"recipe" @keyword
"menu" @keyword
"basis" @keyword

; ── Metadata ─────────────────────────────────────────────────────────
(meta_line ":" @punctuation.special)
(meta_line key: (identifier) @property)
(meta_value) @string

; ── Bindings ─────────────────────────────────────────────────────────
(output name: (identifier) @variable)
(call verb: (identifier) @function)

; ── Ingredients (a distinct variable shade) and strings ──────────────
(ingredient name: (identifier) @variable.special)
(string) @string

; ── References ───────────────────────────────────────────────────────
(node_ref name: (identifier) @variable)
(node_ref member: (identifier) @variable)

; ── Equipment and bases (types) ──────────────────────────────────────
(equip "!" @punctuation.special)
(equip name: (identifier) @type)
(basis_decl name: (identifier) @type)

; ── Sub-recipes ──────────────────────────────────────────────────────
(sub_ref path: (path (identifier) @function))
(sub_ref anchor: (identifier) @label)

; ── Remap ────────────────────────────────────────────────────────────
(mapping from: (identifier) @variable)
(mapping to: (identifier) @variable)
"->" @operator

; ── Amounts ──────────────────────────────────────────────────────────
(amount_text) @number

; ── Temps, timers, durations ─────────────────────────────────────────
(temp) @constant
(timer_active) @constant
(timer_passive) @constant
(timer_unbounded) @constant
(duration) @constant

; ── Flags ────────────────────────────────────────────────────────────
(flag name: (identifier) @attribute)

; ── Temporal links ───────────────────────────────────────────────────
"&by" @keyword
"&during" @keyword
"&lag" @keyword

; ── Menu ─────────────────────────────────────────────────────────────
(menu_stmt alias: (identifier) @variable)
(menu_stmt recipe: (identifier) @function)

; ── Operators and punctuation ────────────────────────────────────────
"=" @operator
["[" "]"] @punctuation.bracket
["(" ")"] @punctuation.bracket
"<+" @punctuation.special
">" @punctuation.special
"#" @punctuation.special
[";" "," "."] @punctuation.delimiter
