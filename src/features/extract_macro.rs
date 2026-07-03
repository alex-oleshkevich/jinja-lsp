// REQ-ACT-07: Extract selection to a macro.

use std::collections::HashMap;

use crate::edit::{TextEdit, WorkspaceEdit};
use crate::features::wrap::selection_is_well_formed;

/// Compute a WorkspaceEdit that extracts lines [start_line, end_line] (inclusive) into a macro.
///
/// The edit:
/// (a) Replaces the selection with `{{ <name>() }}`.
/// (b) Appends `{% macro <name>() %}…body…{% endmacro %}` at the end of the file.
pub fn compute_extract_macro(
    source: &str,
    file: &str,
    start_line: u32,
    end_line: u32,
    macro_name: &str,
) -> Option<WorkspaceEdit> {
    // jinja-lsp-ifrq: the server executes this command with client-supplied line
    // numbers and no other validation. Reject an inverted or out-of-bounds range
    // (rather than silently producing an empty-body macro plus an edit addressing
    // nonexistent lines), and reuse the same well-formedness check the code-action
    // path already gates on so a selection that splits a Jinja tag is rejected too.
    //
    // The inverted-range check must be explicit: `Vec::get` on an inverted
    // `RangeInclusive` (e.g. `1..=0`) returns `Some(&[])`, not `None`, so
    // selection_is_well_formed alone would treat it as a vacuously "balanced"
    // (empty) selection rather than rejecting it.
    if start_line > end_line || !selection_is_well_formed(source, start_line, end_line) {
        return None;
    }

    let source_lines: Vec<&str> = source.split('\n').collect();

    // Extract the selected lines as the macro body.
    let body_lines: Vec<&str> = (start_line as usize..=end_line as usize)
        .filter_map(|i| source_lines.get(i).copied())
        .collect();
    let body = body_lines.join("\n");

    // Edit 1: replace the selection range with a call `{{ name() }}`.
    // Covers start_line:0 to end_line:len(last_line).
    let last_line_len = source_lines
        .get(end_line as usize)
        .map(|l| l.len() as u32)
        .unwrap_or(0);

    let replacement_edit = TextEdit {
        start_line,
        start_col: 0,
        end_line,
        end_col: last_line_len,
        new_text: format!("{{{{ {macro_name}() }}}}"),
    };

    // Edit 2: append the macro definition at the end of the file.
    // Insert after the last line.
    let total_lines = source_lines.len() as u32;
    let last_file_line = total_lines.saturating_sub(1);
    let last_file_col = source_lines.last().map(|l| l.len() as u32).unwrap_or(0);

    let macro_def = format!("\n{{% macro {macro_name}() %}}\n{body}\n{{% endmacro %}}");

    let append_edit = TextEdit {
        start_line: last_file_line,
        start_col: last_file_col,
        end_line: last_file_line,
        end_col: last_file_col,
        new_text: macro_def,
    };

    let mut changes = HashMap::new();
    changes.insert(file.to_owned(), vec![replacement_edit, append_edit]);
    Some(WorkspaceEdit {
        changes,
        create_files: vec![],
    })
}
