/**
 * @file Tree-sitter grammar for the Swede recipe language (.swede)
 * @license MIT
 *
 * Swede is a graph-first recipe language: a file is a set of named bindings
 * (nodes) produced by applying opaque verbs to inputs. See SPEC.md.
 *
 * Design notes:
 *  - Whitespace INCLUDING newlines is `extras`, so a binding may span multiple
 *    lines freely (SPEC §2: "newlines end nothing"). A binding ends when the
 *    next statement begins (a `name =` / `,` output list, a `:meta` line, or a
 *    `basis` keyword).
 *  - Amounts inside `[...]` are captured as an opaque `amount_text` token and
 *    classified (plain / basis / constituent / to-taste / compound) in
 *    swede-syntax, mirroring how tree-sitter-neep defers amount typing.
 *  - A metadata line is a single self-delimiting token `:key value-to-EOL`, so
 *    a `//` inside a source URL is safe (SPEC §2, decision A2).
 *  - A call carries NO trailing string (SPEC decision A1): strings are only
 *    ingredient qualifiers (inside args) or binding annotations.
 */

module.exports = grammar({
  name: 'swede',

  word: $ => $.identifier,

  extras: $ => [
    /[ \t\r\n]+/,
    $.comment,
  ],

  // `verb`, `ingredient`, and a bare `node_ref` all begin with an identifier;
  // the token after it (`(`, `[`, `.`, `,`, `)`, `;`) settles which. `node_ref`
  // carries a negative precedence so the parser prefers a call/ingredient
  // reading when a `(`/`[` follows, and falls back to a bare reference.
  conflicts: $ => [],

  rules: {
    // ── Top level ────────────────────────────────────────────────────
    source_file: $ => choice($.recipe, $.menu),

    recipe: $ => seq(
      'recipe',
      field('name', $.identifier),
      repeat($._statement),
    ),

    menu: $ => seq(
      'menu',
      field('name', $.identifier),
      repeat(choice($.meta_line, $.menu_stmt)),
    ),

    _statement: $ => choice(
      $.meta_line,
      $.basis_decl,
      $.binding,
    ),

    comment: $ => token(/\/\/[^\r\n]*/),

    // ── Metadata ─────────────────────────────────────────────────────
    // A whole `:key value` line captured as one token (value verbatim to EOL).
    meta_line: $ => $.meta_text,
    meta_text: $ => token(/:[a-z][a-z0-9_]*([ \t][^\r\n]*)?/),

    // ── Basis ────────────────────────────────────────────────────────
    basis_decl: $ => seq(
      'basis',
      field('name', $.identifier),
      '=',
      field('amount', $.amount),
    ),

    // ── Bindings ─────────────────────────────────────────────────────
    binding: $ => seq(
      field('outputs', $.output_list),
      '=',
      field('value', $._expr),
      repeat(field('annotation', $.annotation)),
    ),

    output_list: $ => seq($.output, repeat(seq(',', $.output))),
    output: $ => seq(
      field('name', $.identifier),
      optional(seq('[', field('amount', $.amount), ']')),
    ),

    _expr: $ => choice($.call, $.sub_ref, $.node_ref),

    call: $ => seq(
      field('verb', $.identifier),
      '(',
      optional($._arg_list),
      optional(seq(';', field('equipment', $.equip_list))),
      ')',
    ),

    _arg_list: $ => seq($.arg, repeat(seq(',', $.arg))),
    arg: $ => choice($.call, $.ingredient, $.equip, $.node_ref),

    equip_list: $ => seq($.equip_item, repeat(seq(',', $.equip_item))),
    equip_item: $ => choice($.equip, $.node_ref),

    // !oven  |  !oven {350F}
    equip: $ => seq(
      '!',
      field('name', $.identifier),
      optional(field('temp', $.temp)),
    ),

    // chicken_thighs[1lb]  |  ginger[.25in] "grated"
    ingredient: $ => seq(
      field('name', $.identifier),
      '[',
      field('amount', $.amount),
      ']',
      optional(field('qualifier', $.string)),
    ),

    // a reference to an earlier node/ingredient/basis/equipment; dotted form
    // (alias.node) is only meaningful in menu scope (checked in semantics).
    node_ref: $ => prec(-1, seq(
      field('name', $.identifier),
      optional(seq('.', field('member', $.identifier))),
    )),

    // <+bread/poolish>[20%* flour]  |  <+starter#autolyse>  |  <+p>(a->b)
    //
    // prec.right so a `[...]` immediately after `>` binds as the sub-recipe's
    // amount rather than a following `[flag]` annotation.
    sub_ref: $ => prec.right(seq(
      '<+',
      field('path', $.path),
      optional(seq('#', field('anchor', $.identifier))),
      '>',
      optional(seq('[', field('amount', $.amount), ']')),
      optional(field('remap', $.remap)),
    )),

    path: $ => seq($.identifier, repeat(seq('/', $.identifier))),

    remap: $ => seq(
      '(',
      $.mapping,
      repeat(seq(',', $.mapping)),
      ')',
    ),
    mapping: $ => seq(
      field('from', $.identifier),
      '->',
      field('to', $.identifier),
    ),

    // ── Amounts (opaque; classified in swede-syntax) ─────────────────
    amount: $ => $.amount_text,
    amount_text: $ => token(/[^\]\r\n]+/),

    // ── Annotations ──────────────────────────────────────────────────
    annotation: $ => choice(
      $.temp,
      $.timer,
      $.string,
      $.flag,
      $.link,
    ),

    temp: $ => token(/\{[^}\r\n]*\}/),

    timer: $ => choice(
      $.timer_active,
      $.timer_passive,
      $.timer_unbounded,
    ),
    timer_active: $ => token(/@[0-9]+(\.[0-9]+)?(-[0-9]+(\.[0-9]+)?)?[smhd]/),
    timer_passive: $ => token(/~[0-9]+(\.[0-9]+)?(-[0-9]+(\.[0-9]+)?)?[smhd]/),
    timer_unbounded: $ => token('~?'),

    string: $ => token(/"([^"\\]|\\.)*"/),

    // [covered] — an opaque flag shown in renders.
    flag: $ => seq('[', field('name', $.identifier), ']'),

    link: $ => choice(
      seq('&by', '(', field('target', $.node_ref), ')'),
      seq('&during', '(', field('target', $.node_ref), ')'),
      seq('&lag', '(', field('duration', $.duration), ')'),
    ),
    // A unit is required, except a bare `0` (a zero window, e.g. `&lag(0)`).
    duration: $ => token(/[0-9]+(\.[0-9]+)?[smhd]|0/),

    // ── Menu ─────────────────────────────────────────────────────────
    menu_stmt: $ => seq(
      field('alias', $.identifier),
      '=',
      field('recipe', choice($.sub_ref, $.identifier)),
      repeat(field('annotation', $.link)),
    ),

    // ── Shared ───────────────────────────────────────────────────────
    identifier: $ => token(/[a-z][a-z0-9_]*/),
  },
});
