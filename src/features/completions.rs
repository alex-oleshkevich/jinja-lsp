// REQ-CMP-01..12: completion item generation and lazy documentation resolve.

use crate::{
    builtins::registry::{Category, Registry},
    workspace::index::{TemplateIndex, WorkspaceIndex},
};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CompletionKind {
    Filter,
    Function,
    Test,
    Variable,
    Keyword,
    TemplatePath,
    Attribute,
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
    /// Inside a string that follows `extends`, `include`, `import`, or `from`.
    TemplatePath,
    /// Inside `{# #}` — offer nothing (REQ-CMP-06).
    Comment,
    /// Outside any Jinja delimiter — offer nothing (REQ-CMP-06).
    Outside,
}

// ── Context detection ─────────────────────────────────────────────────────────

fn detect_context(source: &str, byte: usize) -> CursorContext {
    let before = &source[..byte.min(source.len())];

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
    match (render_active, stmt_active) {
        (false, false) => CursorContext::Outside,
        (true, false) => classify_render(before, render_open.unwrap()),
        (false, true) => classify_stmt(before, stmt_open.unwrap()),
        (true, true) => {
            // Both active (unusual but can happen if `{{` appears inside `{% %}`).
            // The later opener wins.
            if render_open.unwrap() > stmt_open.unwrap() {
                classify_render(before, render_open.unwrap())
            } else {
                classify_stmt(before, stmt_open.unwrap())
            }
        }
    }
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

    // Attribute context: last non-alphanumeric-underscore char is `.`
    if let Some(dot_pos) = last_dot_before_cursor(inner) {
        let before_dot = &inner[..dot_pos];
        let parent = last_identifier(before_dot);
        if !parent.is_empty() {
            return CursorContext::Attribute { parent: parent.to_owned() };
        }
    }

    // Filter context: there is a `|` in the inner text.
    if inner.contains('|') {
        return CursorContext::Filter;
    }

    CursorContext::Expression
}

/// Classify the cursor inside `{% … %}`.
fn classify_stmt(before: &str, open_pos: usize) -> CursorContext {
    let inner = before.get(open_pos + 2..).unwrap_or("").trim_start_matches(['-', '+', ' ', '\t']);

    // Template path context: starts with path-yielding keyword and has an unclosed quote.
    let first_word = inner.split_whitespace().next().unwrap_or("");
    if matches!(first_word, "extends" | "include" | "import" | "from") {
        if has_unclosed_string(inner) {
            return CursorContext::TemplatePath;
        }
    }

    CursorContext::Statement
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

/// True when `text` contains an odd number of `"` characters (meaning the last
/// string literal is not yet closed).
fn has_unclosed_string(text: &str) -> bool {
    text.chars().filter(|&c| c == '"').count() % 2 == 1
}

// ── Completion item builders ───────────────────────────────────────────────────

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
        kind: CompletionKind::TemplatePath,
        detail: Some("template".to_owned()),
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
/// Items ship without documentation (REQ-CMP-05).  Nothing is returned outside
/// Jinja delimiters or inside comments (REQ-CMP-06).
pub fn complete(
    source: &str,
    line: u32,
    col: u32,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Vec<CompletionItem> {
    let byte = line_col_to_byte(source, line, col);
    match detect_context(source, byte) {
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
            // Global functions (range, dict, …)
            for e in registry.iter_by_category(Category::Function) {
                items.push(function_item(&e.name));
            }
            // Built-in variables (loop, self, …)
            for e in registry.iter_by_category(Category::Variable) {
                items.push(variable_item(&e.name));
            }
            // Context-hinted variables (REQ-HINT-01)
            for e in registry.iter_by_category(Category::ContextVariable) {
                items.push(variable_item(&e.name));
            }
            // Scope-local variables from the current template (REQ-CMP-11)
            for var in &index.variables {
                items.push(variable_item(&var.name));
            }
            items
        }

        // REQ-CMP-02: statement context → keyword list.
        CursorContext::Statement => STATEMENT_KEYWORDS
            .iter()
            .map(|kw| keyword_item(kw))
            .collect(),

        // REQ-CMP-03: attribute context → attrs for the receiver, or empty.
        CursorContext::Attribute { parent } => {
            let attrs = registry.attrs_for(&parent);
            if attrs.is_empty() {
                return vec![]; // REQ-CMP-03: unknown receiver → no completions
            }
            attrs.iter().map(|a| attr_item(&a.attr, &a.parent)).collect()
        }

        // REQ-CMP-12: template path context → workspace template list.
        CursorContext::TemplatePath => workspace
            .templates
            .keys()
            .map(|p| template_path_item(p))
            .collect(),
    }
}

/// Fill documentation for a completion item from its `data` field (REQ-CMP-05).
///
/// `data` format: `"category:name"` where category is one of
/// `filter`, `function`, `test`, `variable`.
pub fn resolve_doc(data: &str, registry: &Registry) -> Option<String> {
    let (cat_str, name) = data.split_once(':')?;
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
