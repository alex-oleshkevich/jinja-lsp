// REQ-ACT-08: Wrap selection in block, if, or for.

use crate::edit::{TextEdit, WorkspaceEdit};
use std::collections::HashMap;

pub fn layer_name() -> &'static str {
    "wrap"
}

/// The kind of wrapper to insert around the selection.
#[derive(Debug, Clone)]
pub enum WrapKind {
    /// Wrap in `{% if condition %}…{% endif %}` (placeholder condition).
    If,
    /// Wrap in `{% for item in items %}…{% endfor %}` (placeholder loop).
    For,
    /// Wrap in `{% block <name> %}…{% endblock %}`.
    Block(String),
}

/// Produce a WorkspaceEdit that wraps [start_line, end_line] (inclusive) in the given wrapper.
///
/// Inserts the opening tag as a new line before start_line and the closing tag
/// as a new line after end_line. Host-language lines within the selection are not modified (P5).
pub fn wrap_selection(
    source: &str,
    file: &str,
    start_line: u32,
    end_line: u32,
    kind: WrapKind,
) -> Option<WorkspaceEdit> {
    let (open_tag, close_tag) = match &kind {
        WrapKind::If => ("{% if condition %}".to_owned(), "{% endif %}".to_owned()),
        WrapKind::For => ("{% for item in items %}".to_owned(), "{% endfor %}".to_owned()),
        WrapKind::Block(name) => (
            format!("{{% block {name} %}}"),
            "{% endblock %}".to_owned(),
        ),
    };

    // Insert the opening tag before start_line by prepending "<tag>\n" at (start_line, 0).
    let open_edit = TextEdit {
        start_line,
        start_col: 0,
        end_line: start_line,
        end_col: 0,
        new_text: format!("{open_tag}\n"),
    };

    // Insert the closing tag AFTER end_line by appending "\n<tag>" at the END of end_line.
    // Using end-of-line avoids the middle-of-file bug where inserting at (end_line+1, 0)
    // would produce "{% endif %}existing_content" on one line.
    let end_line_len = source
        .split('\n')
        .nth(end_line as usize)
        .map(|l| l.len() as u32)
        .unwrap_or(0);
    let close_edit = TextEdit {
        start_line: end_line,
        start_col: end_line_len,
        end_line,
        end_col: end_line_len,
        new_text: format!("\n{close_tag}"),
    };

    let mut changes = HashMap::new();
    changes.insert(file.to_owned(), vec![open_edit, close_edit]);
    Some(WorkspaceEdit { changes, create_files: vec![] })
}
