// REQ-CMP-01..12: completion item generation and lazy documentation resolve.

use crate::{
    builtins::registry::{Category, Registry},
    workspace::{index::{TemplateIndex, WorkspaceIndex}, symbols::VariableScope},
};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CompletionKind {
    Filter,
    Function,
    Test,
    Variable,
    Keyword,
    /// REQ-CMP-12: leaf template file (no further descent).
    File,
    /// REQ-CMP-12: directory that the user can descend into (triggers re-query).
    Folder,
    /// Kept for back-compat; server maps to File.
    TemplatePath,
    Attribute,
    /// REQ-CMP-08: `name=` keyword argument for a macro/function call.
    KeywordArg,
}

/// A single completion candidate (REQ-CMP-07).
///
/// Ships without `documentation`; callers call `resolve_doc(data, registry)`
/// to fill it on demand (REQ-CMP-05).
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    /// Short human-readable kind label shown inline by editors.
    pub detail: Option<String>,
    /// Always `None` until resolved with `resolve_doc` (REQ-CMP-05).
    pub documentation: Option<String>,
    /// Opaque key for lazy resolve: `"category:name"` (REQ-CMP-05).
    pub data: Option<String>,
}

// ── Trigger characters (REQ-CMP-01) ──────────────────────────────────────────

/// Characters that trigger completion (REQ-CMP-01).
pub const TRIGGER_CHARS: &[char] = &['{', '%', ' ', '|', '.', '(', ',', '"'];

// ── Cursor context ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum CursorContext {
    /// After `|` inside `{{ }}` — offer filters.
    Filter,
    /// Inside `{{ }}` without a preceding `|` — offer variables and functions.
    Expression,
    /// Inside `{% %}` before any keyword — offer statement keywords.
    Statement,
    /// Inside `{{ receiver.` — offer attributes of `receiver`.
    Attribute { parent: String },
    /// Inside `{{ callee(` — offer keyword-argument names (REQ-CMP-08).
    CallArgs { callee: String },
    /// Inside a string that follows `extends`, `include`, `import`, or `from`.
    /// `typed` is the text already typed between the opening quote and the cursor.
    TemplatePath { typed: String },
    /// After `from "path" import ` (or after a comma) — offer macro names from `source_path`.
    ImportName { source_path: String },
    /// Inside `{# #}` — offer nothing (REQ-CMP-06).
    Comment,
    /// Outside any Jinja delimiter — offer nothing (REQ-CMP-06).
    Outside,
}

// ── Context detection ─────────────────────────────────────────────────────────

fn detect_context(source: &str, byte: usize) -> CursorContext {
    // Raw block bodies are literal text — no Jinja completions (REQ-CMP-06/§5.4).
    if inside_raw_block(source, byte) {
        return CursorContext::Outside;
    }

    let before = &source[..super::clamp_to_char_boundary(source, byte)];

    // Find the last position of each delimiter opener and closer.
    let render_open = before.rfind("{{");
    let render_close = before.rfind("}}");
    let stmt_open = before.rfind("{%");
    let stmt_close = before.rfind("%}");
    let comment_open = before.rfind("{#");
    let comment_close = before.rfind("#}");

    // A delimiter type is "active" when its last opener comes after its last closer.
    let render_active = is_active(render_open, render_close);
    let stmt_active = is_active(stmt_open, stmt_close);
    let comment_active = is_active(comment_open, comment_close);

    if comment_active {
        return CursorContext::Comment;
    }

    // Pick the later (innermost) active delimiter.
    // Safety: render_active ↔ render_open.is_some(); stmt_active ↔ stmt_open.is_some().
    match (render_active, stmt_active) {
        (false, false) => CursorContext::Outside,
        (true, false) => classify_render(before, render_open.expect("render_active guarantees render_open is Some")),
        (false, true) => classify_stmt(before, stmt_open.expect("stmt_active guarantees stmt_open is Some")),
        (true, true) => {
            // Both active (unusual but can happen if `{{` appears inside `{% %}`).
            // The later opener wins.
            let r = render_open.expect("render_active guarantees render_open is Some");
            let s = stmt_open.expect("stmt_active guarantees stmt_open is Some");
            if r > s {
                classify_render(before, r)
            } else {
                classify_stmt(before, s)
            }
        }
    }
}

/// True if `byte` falls inside the body of a `{% raw %}...{% endraw %}` block.
/// Reuses `find_innermost_open_block` which already does correct stack-based tag scanning.
fn inside_raw_block(source: &str, byte: usize) -> bool {
    let clamped = super::clamp_to_char_boundary(source, byte);
    find_innermost_open_block(&source[..clamped]) == Some("raw")
}

/// True if the cursor sits inside an unclosed string literal in `inner`
/// (i.e., there is an odd number of unescaped quote delimiters).
fn cursor_in_string(inner: &str) -> bool {
    let mut in_string = false;
    let mut string_char = '"';
    let mut escaped = false;
    for c in inner.chars() {
        if in_string {
            if escaped { escaped = false; continue; }
            if c == '\\' { escaped = true; continue; }
            if c == string_char { in_string = false; }
        } else {
            if c == '"' || c == '\'' { in_string = true; string_char = c; }
        }
    }
    in_string
}

/// True if `inner` (text after `{{`) contains a `|` outside of string literals.
fn contains_pipe_outside_string(inner: &str) -> bool {
    let mut in_string = false;
    let mut string_char = '"';
    let mut escaped = false;
    for c in inner.chars() {
        if in_string {
            if escaped { escaped = false; continue; }
            if c == '\\' { escaped = true; continue; }
            if c == string_char { in_string = false; }
        } else {
            match c {
                '"' | '\'' => { in_string = true; string_char = c; }
                '|' => return true,
                _ => {}
            }
        }
    }
    false
}

/// True if the cursor is in an alias-naming slot: `import "..." as ` or
/// `from "..." import name as `.  The cursor is writing an identifier, not
/// selecting from a completion list.
fn is_in_alias_slot(inner: &str) -> bool {
    inner.trim_end().ends_with(" as")
}

fn is_active(open: Option<usize>, close: Option<usize>) -> bool {
    match (open, close) {
        (Some(o), Some(c)) => o > c,
        (Some(_), None) => true,
        _ => false,
    }
}

/// Classify the cursor inside `{{ … }}`.
fn classify_render(before: &str, open_pos: usize) -> CursorContext {
    // Inner text starts two bytes after `{{`.
    let inner = before.get(open_pos + 2..).unwrap_or("");

    // Cursor inside a string literal — no completions (§5.4).
    if cursor_in_string(inner) {
        return CursorContext::Outside;
    }

    // Attribute context: last non-alphanumeric-underscore char is `.`
    if let Some(dot_pos) = last_dot_before_cursor(inner) {
        let before_dot = &inner[..dot_pos];
        let parent = last_identifier(before_dot);
        if !parent.is_empty() {
            return CursorContext::Attribute { parent: parent.to_owned() };
        }
    }

    // REQ-CMP-08: call-args context — cursor is inside unclosed `callee(`.
    // Must be checked BEFORE the filter context: `{{ x | truncate( }}` has both
    // a `|` and an open paren; the open paren is the innermost, most specific context.
    if let Some(callee) = callee_before_open_paren(inner) {
        return CursorContext::CallArgs { callee };
    }

    // Filter context: there is a `|` OUTSIDE string literals.
    if contains_pipe_outside_string(inner) {
        return CursorContext::Filter;
    }

    CursorContext::Expression
}

/// Classify the cursor inside `{% … %}`.
fn classify_stmt(before: &str, open_pos: usize) -> CursorContext {
    let inner = before.get(open_pos + 2..).unwrap_or("").trim_start_matches(['-', '+', ' ', '\t']);

    // Alias slot: `{% import "..." as ` or `{% from "..." import name as `.
    // Cursor is in an identifier-naming position — no list to offer (§5.4).
    if is_in_alias_slot(inner) {
        return CursorContext::Outside;
    }

    // Template path context: starts with path-yielding keyword and has an unclosed quote.
    let first_word = inner.split_whitespace().next().unwrap_or("");
    if matches!(first_word, "extends" | "include" | "import" | "from") {
        if let Some(typed) = extract_typed_path_prefix(inner) {
            return CursorContext::TemplatePath { typed };
        }
    }

    // REQ-CMP-04: import-name context: `from "path" import` with closed string.
    if first_word == "from" {
        if let Some(source_path) = extract_from_import_source(inner) {
            return CursorContext::ImportName { source_path };
        }
    }

    CursorContext::Statement
}

/// If `inner` (stmt text after `{%`) contains an unclosed string after a path keyword,
/// return the text already typed between the opening quote and the end of `inner`.
fn extract_typed_path_prefix(inner: &str) -> Option<String> {
    let mut in_string = false;
    let mut quote_char = '"';
    let mut string_start = 0usize;
    for (byte_pos, c) in inner.char_indices() {
        if in_string {
            if c == quote_char {
                in_string = false; // closed — keep scanning for another open
            }
        } else if c == '"' || c == '\'' {
            in_string = true;
            quote_char = c;
            string_start = byte_pos + c.len_utf8();
        }
    }
    if in_string {
        Some(inner[string_start..].to_owned())
    } else {
        None
    }
}

/// Scan `source_before` (source up to cursor) and return the keyword of the
/// innermost unclosed block statement (e.g. `"for"`, `"block"`, `"if"`).
fn find_innermost_open_block(source_before: &str) -> Option<&'static str> {
    const PAIRS: &[(&str, &str)] = &[
        ("for", "endfor"), ("if", "endif"), ("block", "endblock"),
        ("macro", "endmacro"), ("call", "endcall"), ("filter", "endfilter"),
        ("set", "endset"), ("with", "endwith"), ("raw", "endraw"),
        ("autoescape", "endautoescape"), ("trans", "endtrans"),
    ];
    let bytes = source_before.as_bytes();
    let mut stack: Vec<&'static str> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'%' {
            let rest = &bytes[i + 2..];
            if let Some(close_rel) = find_close_in_bytes(rest) {
                let inner = source_before[i + 2..i + 2 + close_rel]
                    .trim_matches(|c| c == '-' || c == '+' || c == ' ' || c == '\t');
                let first = inner.split_whitespace().next().unwrap_or("");
                let tag_end = i + 2 + close_rel + 2;
                let mut found_close = false;
                for &(open, close) in PAIRS {
                    if first == close {
                        if stack.last() == Some(&open) { stack.pop(); }
                        found_close = true;
                        break;
                    }
                }
                if !found_close {
                    for &(open, _) in PAIRS {
                        if first == open {
                            stack.push(open);
                            break;
                        }
                    }
                }
                i = tag_end;
                continue;
            }
        }
        i += 1;
    }
    stack.last().copied()
}

/// Find the closing `%}` in `bytes`, skipping string literals. Returns the
/// relative byte offset of `%` within `bytes`, or `None`.
fn find_close_in_bytes(bytes: &[u8]) -> Option<usize> {
    let mut i = 0;
    let mut in_str = false;
    let mut str_char = b'"';
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if escaped { escaped = false; }
            else if b == b'\\' { escaped = true; }
            else if b == str_char { in_str = false; }
        } else if b == b'"' || b == b'\'' {
            in_str = true; str_char = b;
        } else if b == b'%' && i + 1 < bytes.len() && bytes[i + 1] == b'}' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn extract_from_import_source(inner: &str) -> Option<String> {
    let rest = inner.strip_prefix("from")?.trim_start();
    let quote = rest.chars().next().filter(|&c| c == '"' || c == '\'')?;
    let rest = &rest[quote.len_utf8()..];
    let close = rest.find(quote)?;
    let source_path = rest[..close].to_owned();
    let after_close = rest[close + quote.len_utf8()..].trim_start();
    if after_close.starts_with("import") {
        Some(source_path)
    } else {
        None
    }
}

/// Returns the byte position of a `.` that is the last non-ident char reached
/// when scanning right-to-left — i.e. the `obj.` in an attribute access.
fn last_dot_before_cursor(inner: &str) -> Option<usize> {
    for (byte_pos, c) in inner.char_indices().rev() {
        if c == '.' {
            return Some(byte_pos);
        }
        if c.is_alphanumeric() || c == '_' {
            continue;
        }
        // Hit a non-ident, non-dot char before finding `.` → not attribute context.
        return None;
    }
    None
}

/// Extract the identifier that ends at the last non-whitespace byte of `s`.
fn last_identifier(s: &str) -> &str {
    let trimmed = s.trim_end();
    let start = trimmed
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    &trimmed[start..]
}

/// REQ-CMP-08: if `inner` ends with an unclosed `identifier(…`, return the callee name.
///
/// Scans right-to-left for the last unmatched `(`; if an identifier precedes it, returns it.
fn callee_before_open_paren(inner: &str) -> Option<String> {
    let mut depth = 0i32;
    let mut paren_pos = None;
    for (i, c) in inner.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    paren_pos = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let paren_pos = paren_pos?;
    let before_paren = inner[..paren_pos].trim_end();
    let callee = last_identifier(before_paren);
    if callee.is_empty() { None } else { Some(callee.to_owned()) }
}

// ── Completion item builders ───────────────────────────────────────────────────

fn kwarg_item(param_name: &str) -> CompletionItem {
    CompletionItem {
        label: format!("{param_name}="),
        kind: CompletionKind::KeywordArg,
        detail: Some("parameter".to_owned()),
        documentation: None,
        data: None,
    }
}

fn filter_item(name: &str) -> CompletionItem {
    CompletionItem {
        label: name.to_owned(),
        kind: CompletionKind::Filter,
        detail: Some("filter".to_owned()),
        documentation: None,
        data: Some(format!("filter:{name}")),
    }
}

fn function_item(name: &str) -> CompletionItem {
    CompletionItem {
        label: name.to_owned(),
        kind: CompletionKind::Function,
        detail: Some("function".to_owned()),
        documentation: None,
        data: Some(format!("function:{name}")),
    }
}

fn variable_item(name: &str) -> CompletionItem {
    CompletionItem {
        label: name.to_owned(),
        kind: CompletionKind::Variable,
        detail: Some("variable".to_owned()),
        documentation: None,
        data: None,
    }
}

fn keyword_item(name: &str) -> CompletionItem {
    CompletionItem {
        label: name.to_owned(),
        kind: CompletionKind::Keyword,
        detail: Some("keyword".to_owned()),
        documentation: None,
        data: None,
    }
}

fn attr_item(attr: &str, parent: &str) -> CompletionItem {
    CompletionItem {
        label: attr.to_owned(),
        kind: CompletionKind::Attribute,
        detail: Some("attribute".to_owned()),
        documentation: None,
        data: Some(format!("attr:{parent}.{attr}")),
    }
}

fn template_path_item(path: &str) -> CompletionItem {
    CompletionItem {
        label: path.to_owned(),
        kind: CompletionKind::File,
        detail: Some("template".to_owned()),
        documentation: None,
        data: None,
    }
}

fn folder_item(name: &str) -> CompletionItem {
    CompletionItem {
        label: format!("{name}/"),
        kind: CompletionKind::Folder,
        detail: Some("directory".to_owned()),
        documentation: None,
        data: None,
    }
}

fn macro_name_item(name: &str) -> CompletionItem {
    CompletionItem {
        label: name.to_owned(),
        kind: CompletionKind::Function,
        detail: Some("macro".to_owned()),
        documentation: None,
        data: None,
    }
}

// ── Statement keyword list (REQ-CMP-09) ───────────────────────────────────────

static STATEMENT_KEYWORDS: &[&str] = &[
    "for", "endfor", "if", "elif", "else", "endif",
    "block", "endblock", "macro", "endmacro",
    "call", "endcall", "filter", "endfilter",
    "set", "endset", "include", "extends",
    "import", "from", "with", "endwith",
    "do", "raw", "endraw",
    "autoescape", "endautoescape",
    "trans", "endtrans",
];

// ── Public API ────────────────────────────────────────────────────────────────

/// Return completion candidates at (`line`, `col`) in `source` (REQ-CMP-02).
///
/// Returns `(items, is_incomplete)` where `is_incomplete = true` means the editor
/// should re-query as the user types (used for directory descent, REQ-CMP-12).
/// Items ship without documentation (REQ-CMP-05).  Nothing is returned outside
/// Jinja delimiters or inside comments (REQ-CMP-06).
pub fn complete(
    source: &str,
    line: u32,
    col: u32,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> (Vec<CompletionItem>, bool) {
    let byte = line_col_to_byte(source, line, col);
    let items = match detect_context(source, byte) {
        CursorContext::Outside | CursorContext::Comment => vec![],

        // REQ-CMP-02: filter context → all registry filters.
        CursorContext::Filter => registry
            .iter_by_category(Category::Filter)
            .into_iter()
            .map(|e| filter_item(&e.name))
            .collect(),

        // REQ-CMP-02: expression context → functions + variables + scope-locals.
        CursorContext::Expression => {
            let mut items: Vec<CompletionItem> = vec![];
            // Determine which scope-gated specials are in scope (REQ-CMP-10).
            let in_for = index.variables.iter().any(|v| {
                v.scope == VariableScope::ForLoop
                && v.valid_range.start_byte <= byte && byte <= v.valid_range.end_byte
            });
            let in_block = index.blocks.iter().any(|b| {
                b.body.start_byte <= byte && byte <= b.body.end_byte
            });
            let in_macro = index.macros.iter().any(|m| {
                m.body.start_byte <= byte && byte <= m.body.end_byte
            });
            // Global functions (range, dict, …)
            for e in registry.iter_by_category(Category::Function) {
                items.push(function_item(&e.name));
            }
            // Built-in variables — scope-gate loop/self/super/caller/varargs/kwargs (REQ-CMP-10).
            for e in registry.iter_by_category(Category::Variable) {
                let out_of_scope = match e.name.as_str() {
                    "loop" => !in_for,
                    "self" | "super" => !in_block,
                    "caller" | "varargs" | "kwargs" => !in_macro,
                    _ => false,
                };
                if !out_of_scope {
                    items.push(variable_item(&e.name));
                }
            }
            // Context-hinted variables (REQ-HINT-01)
            for e in registry.iter_by_category(Category::ContextVariable) {
                items.push(variable_item(&e.name));
            }
            // Scope-local variables from the current template (REQ-CMP-11).
            // Only include variables whose valid_range contains the cursor byte
            // so macro/block locals don't bleed into outer scopes.
            for var in &index.variables {
                if var.valid_range.start_byte <= byte && byte <= var.valid_range.end_byte {
                    items.push(variable_item(&var.name));
                }
            }
            items
        }

        // REQ-CMP-08: call-args context → macro parameter names as `name=` completions.
        CursorContext::CallArgs { callee } => {
            let params = resolve_callee_params(&callee, index, registry, workspace);
            params.iter().map(|p| kwarg_item(p)).collect()
        }

        // REQ-CMP-09: statement context — if typing `end`, offer only the matching end-tag.
        CursorContext::Statement => {
            let before = &source[..byte];
            let stmt_start = before.rfind("{%").unwrap_or(0);
            let inner_raw = before.get(stmt_start + 2..).unwrap_or("")
                .trim_start_matches(|c| c == '-' || c == '+' || c == ' ' || c == '\t');
            if inner_raw.starts_with("end") {
                // Block-aware: offer only the innermost unclosed block's end-tag.
                if let Some(open_kw) = find_innermost_open_block(before) {
                    let end_tag = match open_kw {
                        "for" => "endfor", "if" => "endif", "block" => "endblock",
                        "macro" => "endmacro", "call" => "endcall", "filter" => "endfilter",
                        "set" => "endset", "with" => "endwith", "raw" => "endraw",
                        "autoescape" => "endautoescape", "trans" => "endtrans",
                        other => other, // fallback — should not happen
                    };
                    vec![keyword_item(end_tag)]
                } else {
                    vec![] // no open block — nothing to close
                }
            } else {
                // Normal statement context: offer all statement keywords.
                STATEMENT_KEYWORDS.iter().map(|kw| keyword_item(kw)).collect()
            }
        }

        // REQ-CMP-03: attribute context → attrs for the receiver, or empty.
        CursorContext::Attribute { parent } => {
            let attrs = registry.attrs_for(&parent);
            if attrs.is_empty() {
                return (vec![], false); // REQ-CMP-03: unknown receiver → no completions
            }
            attrs.iter().map(|a| attr_item(&a.attr, &a.parent)).collect()
        }

        // REQ-CMP-12: template path context → one directory level at a time.
        CursorContext::TemplatePath { typed } => {
            return complete_template_path(&typed, workspace);
        }

        // REQ-CMP-04: import-name context → macro names from the source template.
        CursorContext::ImportName { source_path } => workspace
            .templates
            .get(&source_path)
            .map(|src_idx| {
                src_idx.macros.iter().map(|m| macro_name_item(&m.name)).collect()
            })
            .unwrap_or_default(),
    };
    (items, false)
}

/// Fill documentation for a completion item from its `data` field (REQ-CMP-05).
///
/// `data` format: `"category:name"` where category is one of
/// `filter`, `function`, `test`, `variable`.
pub fn resolve_doc(data: &str, registry: &Registry) -> Option<String> {
    let (cat_str, name) = data.split_once(':')?;
    if cat_str == "attr" {
        let (parent, attr_name) = name.split_once('.')?;
        let attr = registry.get_attr(parent, attr_name)?;
        let mut parts = vec![format!("**{}.{}**", parent, attr_name)];
        if let Some(ty) = &attr.ty {
            parts.push(format!("Type: `{ty}`"));
        }
        return Some(parts.join("\n\n"));
    }
    let category = match cat_str {
        "filter" => Category::Filter,
        "function" => Category::Function,
        "test" => Category::Test,
        "variable" => Category::Variable,
        "context_variable" => Category::ContextVariable,
        _ => return None,
    };
    let entry = registry.get(category, name)?;
    let mut parts = vec![format!("**{}** — {}", entry.name, cat_str)];
    if let Some(sig) = &entry.signature {
        parts.push(format!("```jinja\n{sig}\n```"));
    }
    if !entry.body.trim().is_empty() {
        parts.push(entry.body.trim().to_owned());
    }
    Some(parts.join("\n\n"))
}

/// REQ-CMP-08: Resolve parameter names for `callee` from local macros, from-imports, or registry.
fn resolve_callee_params(callee: &str, index: &TemplateIndex, registry: &Registry, workspace: &WorkspaceIndex) -> Vec<String> {
    // 1. Local macro.
    if let Some(m) = index.macros.iter().find(|m| m.name == callee) {
        return m.parameters.iter().map(|p| p.name.clone()).collect();
    }

    // 2. From-imported macro.
    for fi in &index.from_imports {
        for n in &fi.names {
            let matches = n.name == callee || n.alias.as_deref() == Some(callee);
            if matches {
                if let Some(src_idx) = workspace.get_by_ref(&fi.source) {
                    if let Some(m) = src_idx.macros.iter().find(|m| m.name == n.name) {
                        return m.parameters.iter().map(|p| p.name.clone()).collect();
                    }
                }
            }
        }
    }

    // 3. Registry filter — params exclude the implicit receiver (same as signature_help).
    for cat in [Category::Filter, Category::Test, Category::Function] {
        if let Some(entry) = registry.get(cat, callee) {
            return entry.params.iter().map(|p| p.name.clone()).collect();
        }
    }

    vec![]
}

/// REQ-CMP-12: return one directory level of template-path completions.
///
/// `typed` is the text already typed between the opening quote and the cursor.
/// Returns `(items, is_incomplete)` where `is_incomplete = true` when directory
/// items are included (signals the editor to re-query after the user types `/`).
fn complete_template_path(typed: &str, workspace: &WorkspaceIndex) -> (Vec<CompletionItem>, bool) {
    let prefix = typed;
    let mut dirs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut files: Vec<CompletionItem> = Vec::new();

    for path in workspace.templates.keys() {
        let candidate = path.as_str();
        if !candidate.starts_with(prefix) {
            continue;
        }
        let rest = &candidate[prefix.len()..];
        // If `rest` contains a `/`, the next component is a directory.
        if let Some(slash_pos) = rest.find('/') {
            dirs.insert(rest[..slash_pos].to_owned());
        } else {
            // Leaf file at this directory level.
            files.push(template_path_item(candidate));
        }
    }

    let has_dirs = !dirs.is_empty();
    let mut items: Vec<CompletionItem> = dirs.into_iter().map(|d| folder_item(&d)).collect();
    items.extend(files);
    (items, has_dirs)
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn line_col_to_byte(source: &str, target_line: u32, target_col: u32) -> usize {
    let mut byte = 0usize;
    for (i, line) in source.split('\n').enumerate() {
        if i == target_line as usize {
            return byte + (target_col as usize).min(line.len());
        }
        byte += line.len() + 1;
    }
    byte
}
