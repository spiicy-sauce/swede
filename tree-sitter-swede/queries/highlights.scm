; Syntax highlighting for the Swede recipe language.
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
; A binding's output is the node it produces.
(output name: (identifier) @variable)

; A verb is the operation applied.
(call verb: (identifier) @function)

; ── Ingredients ──────────────────────────────────────────────────────
(ingredient name: (identifier) @variable.parameter)

; ── Strings (ingredient qualifiers and doneness annotations) ─────────
(string) @string

; ── References ───────────────────────────────────────────────────────
(node_ref name: (identifier) @variable)
(node_ref member: (identifier) @variable)

; ── Equipment ────────────────────────────────────────────────────────
(equip "!" @punctuation.special)
(equip name: (identifier) @type)

; ── Bases ────────────────────────────────────────────────────────────
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
(temp) @constant.builtin
(timer_active) @constant.builtin
(timer_passive) @constant.builtin
(timer_unbounded) @constant.builtin
(duration) @constant.builtin

; ── Flags ────────────────────────────────────────────────────────────
(flag name: (identifier) @attribute)

; ── Temporal links ───────────────────────────────────────────────────
"&by" @keyword.directive
"&during" @keyword.directive
"&lag" @keyword.directive

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
