// WorkspaceEdit / TextEdit builders shared by code actions and formatting (REQ-FOLD-07).

use std::collections::HashMap;

pub fn layer_name() -> &'static str {
    "edit"
}

/// A line/col range (0-based) within a single file.
#[derive(Debug, Clone, PartialEq)]
pub struct TextEdit {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub new_text: String,
}

/// Accumulated edits and file creations for a single code action (REQ-ACT-09).
#[derive(Debug, Clone)]
pub struct WorkspaceEdit {
    /// file → ordered list of edits (non-overlapping, top-to-bottom).
    pub changes: HashMap<String, Vec<TextEdit>>,
    /// Files to create as (path, initial_content) pairs (REQ-ACT-05).
    pub create_files: Vec<(String, String)>,
}

impl WorkspaceEdit {
    pub fn single(file: &str, edit: TextEdit) -> Self {
        let mut changes = HashMap::new();
        changes.insert(file.to_owned(), vec![edit]);
        WorkspaceEdit {
            changes,
            create_files: vec![],
        }
    }

    pub fn create_file(path: &str) -> Self {
        WorkspaceEdit {
            changes: HashMap::new(),
            create_files: vec![(path.to_owned(), String::new())],
        }
    }
}
