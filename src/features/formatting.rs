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
pub fn format_range(source: &str, start_line: u32, end_line: u32, opts: FormatOptions) -> Vec<TextEdit> {
    let formatted = format_with_options(source, opts);
    if formatted == source {
        return vec![];
    }
    line_edits(source, &formatted, start_line, end_line)
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
