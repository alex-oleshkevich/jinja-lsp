// REQ-ACT-08: Wrap selection in block, if, or for.

use crate::features::code_actions::{TextEdit, WorkspaceEdit};
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
    _source: &str,
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

    // Insert opening tag at start_line (shifts existing content down).
    // Insert closing tag after end_line (which becomes end_line + 1 after the opening insert).
    // Both are zero-length insertions at column 0 of the target line.
    let open_edit = TextEdit {
        start_line,
        start_col: 0,
        end_line: start_line,
        end_col: 0,
        new_text: format!("{open_tag}\n"),
    };

    // After inserting the opener, the original end_line is now end_line + 1.
    // We insert the closer after that, i.e., at the line after end_line + 1.
    let close_edit = TextEdit {
        start_line: end_line + 1,
        start_col: 0,
        end_line: end_line + 1,
        end_col: 0,
        new_text: format!("\n{close_tag}"),
    };

    let mut changes = HashMap::new();
    changes.insert(file.to_owned(), vec![open_edit, close_edit]);
    Some(WorkspaceEdit { changes, create_files: vec![] })
}
