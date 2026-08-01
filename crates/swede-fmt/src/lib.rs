//! The canonical Swede formatter.
//!
//! Within each contiguous run of assignments (`basis`/binding/menu entry, not
//! separated by a blank or comment line), the `=` signs are column-aligned and
//! normalized to a single trailing space. Continuation lines of a multi-line
//! call are shifted by the same delta, so wrapped arguments stay aligned under
//! their opening paren. Everything else — comments, blank lines, amounts,
//! strings, the interior of expressions — passes through byte-for-byte.
//!
//! Like a good formatter it is idempotent, and it returns the input unchanged if
//! the file has syntax errors (mirroring `neep-fmt`).

use swede_syntax::{parse, File, Recipe};

/// One `name = value` assignment to align.
struct Assign {
    start: usize, // byte offset of the first significant char (post-indent)
    eq: usize,    // byte offset of the `=`
    end: usize,   // byte offset just past the whole statement
}

/// A disjoint edit: replace `[start, end)` with `text`.
struct Edit {
    start: usize,
    end: usize,
    text: String,
}

pub fn format_source(source: &str) -> String {
    let lowered = parse(source);
    if !lowered.errors.is_empty() {
        return source.to_string();
    }
    let assigns = match &lowered.file {
        Some(File::Recipe(r)) => recipe_assigns(source, r),
        // Menus are short and single-line; align their entries too.
        Some(File::Menu(m)) => m
            .entries
            .iter()
            .filter_map(|e| mk_assign(source, e.span.start_byte, e.span.end_byte))
            .collect(),
        None => return source.to_string(),
    };
    if assigns.is_empty() {
        return source.to_string();
    }

    let line_starts = line_starts(source);
    let mut edits = Vec::new();
    for run in group_runs(&assigns, &line_starts) {
        align_run(source, &line_starts, run, &mut edits);
    }
    apply(source, edits)
}

fn recipe_assigns(source: &str, r: &Recipe) -> Vec<Assign> {
    let mut v = Vec::new();
    for b in &r.bases {
        if let Some(a) = mk_assign(source, b.span.start_byte, b.span.end_byte) {
            v.push(a);
        }
    }
    for b in &r.bindings {
        if let Some(a) = mk_assign(source, b.span.start_byte, b.span.end_byte) {
            v.push(a);
        }
    }
    v.sort_by_key(|a| a.start);
    v
}

/// The first `=` in a statement is always its assignment operator (the output
/// list / basis name / alias before it contains none).
fn mk_assign(source: &str, start: usize, end: usize) -> Option<Assign> {
    let eq = source[start..end].find('=')? + start;
    Some(Assign { start, eq, end })
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut v = vec![0];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v
}

fn row_of(byte: usize, line_starts: &[usize]) -> usize {
    match line_starts.binary_search(&byte) {
        Ok(row) => row,
        Err(next) => next - 1,
    }
}

/// Split assignments into runs of vertically-adjacent lines (a blank or comment
/// line, which widens the row gap past 1, breaks the run).
fn group_runs<'a>(assigns: &'a [Assign], line_starts: &[usize]) -> Vec<&'a [Assign]> {
    let mut runs = Vec::new();
    let mut start = 0;
    for i in 1..assigns.len() {
        let prev_last = row_of(assigns[i - 1].end.saturating_sub(1), line_starts);
        let cur_first = row_of(assigns[i].start, line_starts);
        if cur_first > prev_last + 1 {
            runs.push(&assigns[start..i]);
            start = i;
        }
    }
    if !assigns.is_empty() {
        runs.push(&assigns[start..]);
    }
    runs
}

fn leading_ws(source: &str, line_start: usize) -> usize {
    source[line_start..].bytes().take_while(|b| *b == b' ' || *b == b'\t').count()
}

fn align_run(source: &str, line_starts: &[usize], run: &[Assign], edits: &mut Vec<Edit>) {
    // out_text is the assignment's left side, trimmed of trailing space.
    let out_text = |a: &Assign| source[a.start..a.eq].trim_end();
    let width = run.iter().map(|a| out_text(a).chars().count()).max().unwrap_or(0);

    for a in run {
        let ot = out_text(a);
        let l = ot.chars().count();
        let first_line = line_starts[row_of(a.start, line_starts)];

        // verb / value start: first non-space byte after `=`
        let after_eq = a.eq + 1;
        let verb_start = after_eq + source[after_eq..].bytes().take_while(|b| *b == b' ').count();
        let old_verb_col = verb_start - first_line;

        // Rewrite the first line's prefix: `<out><pad>= ` at column width+1.
        let new_prefix = format!("{ot}{}= ", " ".repeat(width + 1 - l));
        edits.push(Edit { start: first_line, end: verb_start, text: new_prefix });

        // Shift continuation lines by the delta the value moved.
        let new_verb_col = width + 3; // width+1 for `=`, +1 space, +1 to land on the char
        let delta = new_verb_col as isize - old_verb_col as isize;
        if delta != 0 {
            let last_row = row_of(a.end.saturating_sub(1), line_starts);
            let first_row = row_of(a.start, line_starts);
            let continuations = if first_row < last_row {
                &line_starts[(first_row + 1)..=last_row]
            } else {
                &[]
            };
            for &ls in continuations {
                let ws = leading_ws(source, ls);
                // skip fully-blank lines inside the statement
                if ls + ws >= a.end || source.as_bytes().get(ls + ws) == Some(&b'\n') {
                    continue;
                }
                let new_ws = (ws as isize + delta).max(0) as usize;
                edits.push(Edit { start: ls, end: ls + ws, text: " ".repeat(new_ws) });
            }
        }
    }
}

fn apply(source: &str, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|e| e.start);
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0;
    for e in edits {
        if e.start < cursor {
            continue; // safety: skip any accidental overlap
        }
        out.push_str(&source[cursor..e.start]);
        out.push_str(&e.text);
        cursor = e.end;
    }
    out.push_str(&source[cursor..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_idempotent(src: &str) {
        let once = format_source(src);
        let twice = format_source(&once);
        assert_eq!(once, twice, "not idempotent");
    }

    #[test]
    fn aligns_equals_in_a_run() {
        let src = "recipe r\na = f(x[1g])\nbb = g(a)\nplate = h(bb)\n";
        let out = format_source(src);
        // all `=` align to the widest left side ("plate" = 5) at column 6
        for line in out.lines().filter(|l| l.contains('=')) {
            assert_eq!(line.find('=').unwrap(), 6, "line: {line:?}");
        }
        assert_idempotent(src);
    }

    #[test]
    fn preserves_comments_and_blanks() {
        let src = "recipe r\na = f(x[1g])\n\n// note\nplate = h(a)\n";
        let out = format_source(src);
        assert!(out.contains("// note"));
        assert!(out.contains("\n\n// note"), "blank line preserved: {out:?}");
        // the blank+comment breaks the run, so each of the two runs aligns alone
        assert_idempotent(src);
    }

    #[test]
    fn syntax_error_passes_through() {
        let src = "recipe r\na = f(\n"; // unclosed call
        assert_eq!(format_source(src), src);
    }

    #[test]
    fn multiline_continuation_shifts_with_verb() {
        // `=` moves right; the wrapped arg line should move right by the same amount
        let src = "recipe r\nx = mix(a[1g],\n        b[2g])\nplateee = done(x)\n";
        let out = format_source(src);
        let lines: Vec<&str> = out.lines().collect();
        // "plateee" (7) sets the column; x's verb moves right, continuation follows
        let verb_col = lines[1].find("mix(").unwrap();
        let cont_col = lines[2].find("b[2g]").unwrap();
        // b[ was aligned one past `mix(` opening paren originally; stays aligned
        assert_eq!(cont_col, verb_col + 4, "continuation should track the paren: {out:?}");
        assert_idempotent(src);
    }

    #[test]
    fn real_fixtures_are_idempotent() {
        for f in ["miso_chicken_and_rice", "snow_pea_salad", "fifty_fifty_loaf", "tuesday.menu"] {
            let path = format!("{}/../../fixtures/valid/{}.swede", env!("CARGO_MANIFEST_DIR"), f);
            let src = std::fs::read_to_string(&path).unwrap();
            assert_idempotent(&src);
        }
    }
}
