// REQ-FMT-07: LSP textDocument/formatting and textDocument/rangeFormatting.
//
// Both delegate to the same `src/format/` engine; the LSP wrappers translate
// between LSP TextEdit types and line/col coordinates.

use crate::features::code_actions::TextEdit;
use crate::format::format;

pub fn layer_name() -> &'static str {
    "formatting"
}

/// Format the entire document. Returns a minimal Vec<TextEdit>:
/// - Empty if the source is already formatted.
/// - A single edit per changed line replacing it with the formatted version.
pub fn format_document(source: &str) -> Vec<TextEdit> {
    let formatted = format(source);
    if formatted == source {
        return vec![];
    }
    line_edits(source, &formatted, 0, u32::MAX)
}

/// Format the document, returning only edits that fall within [start_line, end_line] (inclusive).
pub fn format_range(source: &str, start_line: u32, end_line: u32) -> Vec<TextEdit> {
    let formatted = format(source);
    if formatted == source {
        return vec![];
    }
    line_edits(source, &formatted, start_line, end_line)
}

/// Compute per-line TextEdits between `original` and `formatted` within [start_line, end_line].
fn line_edits(original: &str, formatted: &str, start_line: u32, end_line: u32) -> Vec<TextEdit> {
    let orig_lines: Vec<&str> = original.split('\n').collect();
    let fmt_lines: Vec<&str> = formatted.split('\n').collect();
    let mut edits = Vec::new();

    let max_line = orig_lines.len().max(fmt_lines.len()) as u32;
    for line_no in 0..max_line {
        if line_no < start_line || line_no > end_line {
            continue;
        }
        let orig = orig_lines.get(line_no as usize).copied().unwrap_or("");
        let fmt = fmt_lines.get(line_no as usize).copied().unwrap_or("");
        if orig != fmt {
            edits.push(TextEdit {
                start_line: line_no,
                start_col: 0,
                end_line: line_no,
                end_col: orig.len() as u32,
                new_text: fmt.to_owned(),
            });
        }
    }

    edits
}
