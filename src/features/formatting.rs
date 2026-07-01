// REQ-FMT-07: LSP textDocument/formatting and textDocument/rangeFormatting.
//
// Both delegate to the same `src/format/` engine; the LSP wrappers translate
// between LSP TextEdit types and line/col coordinates.

use crate::edit::TextEdit;
use crate::format::format_with_options;

pub use crate::format::FormatOptions;

pub fn layer_name() -> &'static str {
    "formatting"
}

/// Format the entire document. Returns a minimal Vec<TextEdit>:
/// - Empty if the source is already formatted.
/// - A single edit per changed line replacing it with the formatted version.
pub fn format_document(source: &str, opts: FormatOptions) -> Vec<TextEdit> {
    let formatted = format_with_options(source, opts);
    if formatted == source {
        return vec![];
    }
    line_edits(source, &formatted, 0, u32::MAX)
}

/// Format the document, returning only edits that fall within [start_line, end_line] (inclusive).
///
/// REQ-FMT-07: the range is snapped outward to whole Jinja constructs so partial-tag edits
/// are never produced: if the selection begins inside a tag body, start_line is expanded to
/// include the opening tag; if it ends inside a construct, end_line is expanded to the closer.
pub fn format_range(source: &str, start_line: u32, end_line: u32, opts: FormatOptions) -> Vec<TextEdit> {
    let (snapped_start, snapped_end) = snap_range_to_constructs(source, start_line, end_line);
    let formatted = format_with_options(source, opts);
    if formatted == source {
        return vec![];
    }
    line_edits(source, &formatted, snapped_start, snapped_end)
}

/// Expand [start_line, end_line] outward so neither edge splits a Jinja tag.
///
/// Scans upward from start_line for the nearest line containing `{%` (without a matching
/// closing `%}` before start_line), and downward from end_line for the nearest `%}`.
fn snap_range_to_constructs(source: &str, start_line: u32, end_line: u32) -> (u32, u32) {
    let lines: Vec<&str> = source.split('\n').collect();
    let total = lines.len() as u32;

    // Snap start: walk backward from start_line; if any line has `{%` without `%}` on the
    // same line (opener), and start_line is strictly after it (inside a construct), expand.
    let snapped_start = {
        let mut s = start_line;
        // Scan upward for an unclosed `{%` tag.
        let mut depth: i32 = 0;
        for i in (0..start_line.min(total)).rev() {
            let line = lines[i as usize];
            if line.contains("{%") && !line.contains("%}") {
                // An opening tag without its close on the same line.
                depth += 1;
                if depth > 0 {
                    s = i;
                    break;
                }
            }
        }
        s
    };

    // Snap end: walk forward from end_line; if any line has `%}` without `{%` on the
    // same line (closer), expand to include it.
    let snapped_end = {
        let mut e = end_line;
        let mut depth: i32 = 0;
        for i in end_line.min(total - 1) + 1..total {
            let line = lines[i as usize];
            if line.contains("%}") && !line.contains("{%") {
                depth += 1;
                if depth > 0 {
                    e = i;
                    break;
                }
            }
        }
        e
    };

    (snapped_start, snapped_end)
}

/// Compute per-line TextEdits between `original` and `formatted` within [start_line, end_line].
///
/// Falls back to a single whole-document replace when line counts differ, because per-line
/// diffing is only correct when the formatter never adds or removes lines.
fn line_edits(original: &str, formatted: &str, start_line: u32, end_line: u32) -> Vec<TextEdit> {
    let orig_lines: Vec<&str> = original.split('\n').collect();
    let fmt_lines: Vec<&str> = formatted.split('\n').collect();

    if orig_lines.len() != fmt_lines.len() {
        let last_line = (orig_lines.len().saturating_sub(1)) as u32;
        let last_col = orig_lines.last().map(|l| l.len()).unwrap_or(0) as u32;
        return vec![TextEdit {
            start_line: 0,
            start_col: 0,
            end_line: last_line,
            end_col: last_col,
            new_text: formatted.to_owned(),
        }];
    }

    let mut edits = Vec::new();
    for (line_no, (orig, fmt)) in orig_lines.iter().zip(fmt_lines.iter()).enumerate() {
        let line_no = line_no as u32;
        if line_no < start_line || line_no > end_line {
            continue;
        }
        if orig != fmt {
            edits.push(TextEdit {
                start_line: line_no,
                start_col: 0,
                end_line: line_no,
                end_col: orig.len() as u32,
                new_text: fmt.to_string(),
            });
        }
    }

    edits
}
