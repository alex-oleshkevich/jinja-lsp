// REQ-DIAG-01..06, F01: check runner — pure reads over TemplateIndex/WorkspaceIndex.
// Each check emits zero or more Diagnostics; the caller applies noqa + config filters.

use std::collections::{HashMap, HashSet};

use crate::{
    builtins::registry::{Category, Registry},
    diagnostic::Diagnostic,
    diagnostics::DiagCode,
    workspace::{
        index::{ResolvedBinding, TemplateIndex, WorkspaceIndex},
        symbols::{MacroDefinition, ReferenceKind, TemplateRefKind},
    },
};

/// Run all Pass-1 (per-file) checks and return the raw findings.
///
/// Checks implemented: E001, W106, E101, E102, E103, E104, W201, W202, W203, W301, W302, W303, W304, W305, W402, E401, E403, E404, E501, E601.
pub fn run_checks(
    source: &str,
    path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    check_e001(path, index, &mut out);
    // F01 §10: when the parse has errors, only E001 fires — all other checks
    // rely on a valid AST and would produce a false-positive cascade.
    if !index.syntax_errors.is_empty() {
        return out;
    }
    check_w106(source, path, index, registry, &mut out);
    check_e101(path, index, registry, workspace, &mut out);
    check_e103(path, index, registry, workspace, &mut out);
    check_e102_e104(path, index, registry, &mut out);
    check_w201(path, index, &mut out);
    check_w202(path, index, workspace, &mut out);
    check_w203(source, path, index, &mut out);
    check_w301(path, index, &mut out);
    check_w302(path, index, &mut out);
    check_w303(path, index, &mut out);
    check_w304(path, index, &mut out);
    check_w305(path, index, &mut out);
    check_e403(path, index, workspace, &mut out);
    check_e404(path, index, workspace, &mut out);
    check_e501(path, index, workspace, &mut out);
    check_w402_e401(path, index, &mut out);
    check_e601(path, index, workspace, &mut out);
    out
}

// ── E001: syntax error ────────────────────────────────────────────────────────

fn check_e001(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    for err in &index.syntax_errors {
        out.push(Diagnostic {
            file: path.to_owned(),
            line: err.span.start_line,
            col: err.span.start_col,
            code: DiagCode::E001.code_str().to_owned(),
            slug: DiagCode::E001.slug().to_owned(),
            severity: DiagCode::E001.severity(),
            message: "syntax error".to_owned(),
        });
    }
}

// ── E101: undefined-variable ──────────────────────────────────────────────────

fn check_e101(
    path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
    out: &mut Vec<Diagnostic>,
) {
    // F01 §10: prevent E101 cascade — tree-sitter captures filter/test names as both
    // @identifier and @filter/@custom_test.  Skip identifiers already captured precisely.
    let filter_test_bytes: std::collections::HashSet<usize> = index
        .references
        .iter()
        .filter(|r| matches!(r.kind, ReferenceKind::Filter | ReferenceKind::Test))
        .map(|r| r.span.start_byte)
        .collect();

    // Names that structurally suppress E101 without a registry lookup.
    let macro_names: std::collections::HashSet<&str> =
        index.macros.iter().map(|m| m.name.as_str()).collect();
    let alias_names: std::collections::HashSet<&str> =
        index.import_aliases.iter().map(|a| a.alias.as_str()).collect();
    let from_names: std::collections::HashSet<&str> = index
        .from_imports
        .iter()
        .flat_map(|fi| {
            fi.names
                .iter()
                .map(|n| n.alias.as_deref().unwrap_or(n.name.as_str()))
        })
        .collect();

    for r in &index.references {
        if r.kind != ReferenceKind::Identifier {
            continue;
        }
        // Multi-level attribute chains (e.g. `request.user` from `{{ request.user.name }}`)
        // are captured as @object with the intermediate path as the name.  They are not
        // bare variable references and must not trigger E101.
        if r.name.contains('.') {
            continue;
        }
        // Skip identifiers that the grammar also captured precisely as a filter or test.
        if filter_test_bytes.contains(&r.span.start_byte) {
            continue;
        }
        // Local variable in scope — resolve_reference handles valid_range containment.
        if !matches!(index.resolve_reference(r, workspace), ResolvedBinding::HostOwned) {
            continue;
        }
        let name = r.name.as_str();
        // Local macro / import alias / from-import name.
        if macro_names.contains(name) || alias_names.contains(name) || from_names.contains(name) {
            continue;
        }
        // Jinja2 built-in global variable (loop, caller, varargs, …).
        if registry.get(Category::Variable, name).is_some() {
            continue;
        }
        // Macro parameter in scope — parameters bind within the macro body.
        let in_macro_param = index.macros.iter().any(|m| {
            m.body.start_byte < m.body.end_byte
                && m.body.start_byte <= r.span.start_byte
                && r.span.end_byte <= m.body.end_byte
                && m.parameters.iter().any(|p| p.name == name)
        });
        if in_macro_param {
            continue;
        }
        // REQ-HINT-04: hinted context_variable suppresses, respecting template scope.
        if let Some(entry) = registry.get(Category::ContextVariable, name) {
            let applies = match &entry.template {
                None => true,
                Some(t) => path == t.as_str() || path.ends_with(&format!("/{t}")),
            };
            if applies {
                continue;
            }
        }
        out.push(Diagnostic {
            file: path.to_owned(),
            line: r.span.start_line,
            col: r.span.start_col,
            code: DiagCode::E101.code_str().to_owned(),
            slug: DiagCode::E101.slug().to_owned(),
            severity: DiagCode::E101.severity(),
            message: format!("'{}' is not defined", name),
        });
    }
}

// ── E103: undefined-function ──────────────────────────────────────────────────

fn check_e103(
    path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
    out: &mut Vec<Diagnostic>,
) {
    for r in &index.references {
        if r.kind != ReferenceKind::Function {
            continue;
        }
        // resolve_reference covers: local macros, from-imports, workspace-wide macros.
        if !matches!(index.resolve_reference(r, workspace), ResolvedBinding::HostOwned) {
            continue;
        }
        let name = r.name.as_str();
        // Jinja2 built-in functions (range, namespace, joiner, …).
        if registry.get(Category::Function, name).is_some() {
            continue;
        }
        // Filters called with args are captured as ReferenceKind::Function by treesitter
        // (grammar emits a function_call node). Check Category::Filter to avoid false positives.
        if registry.get(Category::Filter, name).is_some() {
            continue;
        }
        out.push(Diagnostic {
            file: path.to_owned(),
            line: r.span.start_line,
            col: r.span.start_col,
            code: DiagCode::E103.code_str().to_owned(),
            slug: DiagCode::E103.slug().to_owned(),
            severity: DiagCode::E103.severity(),
            message: format!("'{}' is not defined", name),
        });
    }
}

// ── W106: unknown-attribute ───────────────────────────────────────────────────
// REQ-HINT-05: off by default; only fires against hinted context_variables with declared attrs.

fn check_w106(source: &str, path: &str, index: &TemplateIndex, registry: &Registry, out: &mut Vec<Diagnostic>) {
    // Dotted attribute access: {{ obj.attr }} — captured as ReferenceKind::Attribute.
    for r in &index.references {
        if r.kind != ReferenceKind::Attribute {
            continue;
        }
        let attr = r.name.as_str();
        let Some(parent) = attribute_parent(source, r.span.start_byte) else { continue };
        let Some(entry) = registry.get(Category::ContextVariable, parent) else { continue };
        // REQ-HINT-03: template scope — skip if this hint does not apply to the current file.
        if let Some(t) = &entry.template {
            if path != t.as_str() && !path.ends_with(&format!("/{t}")) {
                continue;
            }
        }
        let declared_attrs = registry.attrs_for(parent);
        if declared_attrs.is_empty() { continue; }
        if declared_attrs.iter().any(|a| a.attr == attr) { continue; }
        out.push(Diagnostic {
            file: path.to_owned(),
            line: r.span.start_line,
            col: r.span.start_col,
            code: DiagCode::W106.code_str().to_owned(),
            slug: DiagCode::W106.slug().to_owned(),
            severity: DiagCode::W106.severity(),
            message: format!("'{}' has no declared attribute '{}'", parent, attr),
        });
    }

    // Subscript attribute access: {{ obj["attr"] }} or {{ obj['attr'] }}.
    // The tree-sitter grammar does not produce Attribute references for subscript nodes,
    // so we scan the source text directly (REQ-HINT-05).
    for (parent, attr, line, col) in subscript_accesses(source) {
        let Some(entry) = registry.get(Category::ContextVariable, parent) else { continue };
        if let Some(t) = &entry.template {
            if path != t.as_str() && !path.ends_with(&format!("/{t}")) {
                continue;
            }
        }
        let declared_attrs = registry.attrs_for(parent);
        if declared_attrs.is_empty() { continue; }
        if declared_attrs.iter().any(|a| a.attr == attr) { continue; }
        out.push(Diagnostic {
            file: path.to_owned(),
            line,
            col,
            code: DiagCode::W106.code_str().to_owned(),
            slug: DiagCode::W106.slug().to_owned(),
            severity: DiagCode::W106.severity(),
            message: format!("'{}' has no declared attribute '{}'", parent, attr),
        });
    }
}

/// Scan source text for `identifier["key"]` and `identifier['key']` patterns.
/// Returns (parent_name, attr_name, line, col) for each match; col points at the key.
fn subscript_accesses(source: &str) -> Vec<(&str, &str, u32, u32)> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Find `[` preceded by an identifier.
        if bytes[i] != b'[' { i += 1; continue; }
        // Find the identifier before `[`.
        let before_bracket = i;
        if before_bracket == 0 { i += 1; continue; }
        let id_end = before_bracket;
        let mut id_start = id_end;
        while id_start > 0 && (bytes[id_start - 1].is_ascii_alphanumeric() || bytes[id_start - 1] == b'_') {
            id_start -= 1;
        }
        if id_start == id_end { i += 1; continue; } // no identifier before `[`
        let parent = match std::str::from_utf8(&bytes[id_start..id_end]) {
            Ok(s) if !s.is_empty() => s,
            _ => { i += 1; continue; }
        };
        // After `[`, expect an optional space then a quote.
        let mut j = i + 1;
        while j < bytes.len() && bytes[j] == b' ' { j += 1; }
        if j >= bytes.len() { i += 1; continue; }
        let quote = bytes[j];
        if quote != b'"' && quote != b'\'' { i += 1; continue; }
        let key_start = j + 1;
        let key_byte = key_start;
        // Find closing quote.
        let mut k = key_start;
        while k < bytes.len() && bytes[k] != quote { k += 1; }
        if k >= bytes.len() { i += 1; continue; }
        let attr = match std::str::from_utf8(&bytes[key_start..k]) {
            Ok(s) if !s.is_empty() => s,
            _ => { i += 1; continue; }
        };
        // Verify closing `]` follows.
        let mut l = k + 1;
        while l < bytes.len() && bytes[l] == b' ' { l += 1; }
        if l >= bytes.len() || bytes[l] != b']' { i += 1; continue; }
        // Compute line/col of the key (points at the opening quote + 1, i.e. the key content).
        let (line, col) = {
            let mut line = 0u32;
            let mut col = 0u32;
            for (idx, &b) in bytes[..key_byte].iter().enumerate() {
                let _ = idx;
                if b == b'\n' { line += 1; col = 0; } else { col += 1; }
            }
            (line, col)
        };
        out.push((parent, attr, line, col));
        i = l + 1;
    }
    out
}

/// Scan backwards from `attr_start_byte` to find the parent identifier name.
fn attribute_parent(source: &str, attr_start_byte: usize) -> Option<&str> {
    if attr_start_byte == 0 {
        return None;
    }
    let before = source.get(..attr_start_byte)?;
    let dot_pos = before.rfind('.')?;
    let before_dot = &before[..dot_pos];
    let end = before_dot.len();
    let start = before_dot
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let parent = &before_dot[start..end];
    if parent.is_empty() { None } else { Some(parent) }
}

// ── E102: undefined filter / E104: undefined test ─────────────────────────────

fn check_e102_e104(path: &str, index: &TemplateIndex, registry: &Registry, out: &mut Vec<Diagnostic>) {
    for r in &index.references {
        match r.kind {
            ReferenceKind::Filter if registry.get(Category::Filter, &r.name).is_none() => {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: r.span.start_line,
                    col: r.span.start_col,
                    code: DiagCode::E102.code_str().to_owned(),
                    slug: DiagCode::E102.slug().to_owned(),
                    severity: DiagCode::E102.severity(),
                    message: format!("undefined filter '{}'", r.name),
                });
            }
            ReferenceKind::Test if registry.get(Category::Test, &r.name).is_none() => {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: r.span.start_line,
                    col: r.span.start_col,
                    code: DiagCode::E104.code_str().to_owned(),
                    slug: DiagCode::E104.slug().to_owned(),
                    severity: DiagCode::E104.severity(),
                    message: format!("undefined test '{}'", r.name),
                });
            }
            _ => {}
        }
    }
}

// ── W301: duplicate block ─────────────────────────────────────────────────────

fn check_w301(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for b in &index.blocks {
        let count = seen.entry(b.name.as_str()).or_insert(0);
        *count += 1;
        if *count == 2 {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: b.span.start_line,
                col: b.span.start_col,
                code: DiagCode::W301.code_str().to_owned(),
                slug: DiagCode::W301.slug().to_owned(),
                severity: DiagCode::W301.severity(),
                message: format!("duplicate block '{}'", b.name),
            });
        }
    }
}

// ── W302: duplicate macro ─────────────────────────────────────────────────────

fn check_w302(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for m in &index.macros {
        let count = seen.entry(m.name.as_str()).or_insert(0);
        *count += 1;
        if *count == 2 {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: m.span.start_line,
                col: m.span.start_col,
                code: DiagCode::W302.code_str().to_owned(),
                slug: DiagCode::W302.slug().to_owned(),
                severity: DiagCode::W302.severity(),
                message: format!("duplicate macro '{}'", m.name),
            });
        }
    }
}

// ── W201: unused-variable ─────────────────────────────────────────────────────

fn check_w201(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let used_names: std::collections::HashSet<&str> =
        index.references.iter().map(|r| r.name.as_str()).collect();
    for v in &index.variables {
        // Skip variables with no valid_range (external/context vars or unpopulated bindings).
        if v.valid_range.start_byte >= v.valid_range.end_byte {
            continue;
        }
        if !used_names.contains(v.name.as_str()) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: v.span.start_line,
                col: v.span.start_col,
                code: DiagCode::W201.code_str().to_owned(),
                slug: DiagCode::W201.slug().to_owned(),
                severity: DiagCode::W201.severity(),
                message: format!("variable '{}' is assigned but never used", v.name),
            });
        }
    }
}

// ── W202: unused-macro ────────────────────────────────────────────────────────

fn check_w202(path: &str, index: &TemplateIndex, workspace: &WorkspaceIndex, out: &mut Vec<Diagnostic>) {
    // Pass 2 (cross-file): collect every macro name referenced anywhere in the workspace.
    // A macro is "used" if called locally OR imported/called from any other template.
    let mut used: HashSet<String> = HashSet::new();

    // Own references (local calls inside the macro library itself).
    for r in &index.references {
        if matches!(r.kind, ReferenceKind::Function | ReferenceKind::Identifier) {
            used.insert(r.name.clone());
        }
    }

    // Workspace-wide scan: other templates that call or import from `path`.
    for tmpl in workspace.templates.values() {
        // Direct function calls and references in any template.
        for r in &tmpl.references {
            if r.kind == ReferenceKind::Function {
                used.insert(r.name.clone());
            }
        }
        // from-imports that source from the current template count as "exporting" the macro.
        for fi in &tmpl.from_imports {
            if (workspace.resolve_key(&fi.source) == Some(path))
                || fi.source == path
            {
                for n in &fi.names {
                    used.insert(n.name.clone());
                }
            }
        }
    }

    for m in &index.macros {
        if !used.contains(m.name.as_str()) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: m.span.start_line,
                col: m.span.start_col,
                code: DiagCode::W202.code_str().to_owned(),
                slug: DiagCode::W202.slug().to_owned(),
                severity: DiagCode::W202.severity(),
                message: format!("macro '{}' is defined but never called in this template", m.name),
            });
        }
    }
}

// ── W203: unused-import ───────────────────────────────────────────────────────

fn check_w203(source: &str, path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let used_names: std::collections::HashSet<&str> =
        index.references.iter().map(|r| r.name.as_str()).collect();

    let src_bytes = source.as_bytes();

    for a in &index.import_aliases {
        // Import alias namespaces (`{% import "m" as alias %}`) are used as `alias.fn()`.
        // The attribute-access query doesn't capture them, so scan the source text directly.
        let name = a.alias.as_bytes();
        let mut found = used_names.contains(a.alias.as_str()); // already captured reference
        if !found {
            let mut pos = 0usize;
            while pos + name.len() <= src_bytes.len() {
                if &src_bytes[pos..pos + name.len()] == name {
                    let before_ok = pos == 0 || !(src_bytes[pos - 1].is_ascii_alphanumeric() || src_bytes[pos - 1] == b'_');
                    let after = pos + name.len();
                    let after_ok = after < src_bytes.len() && src_bytes[after] == b'.'; // alias.method
                    if before_ok && after_ok {
                        found = true;
                        break;
                    }
                }
                pos += 1;
            }
        }
        if !found {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: a.span.start_line,
                col: a.span.start_col,
                code: DiagCode::W203.code_str().to_owned(),
                slug: DiagCode::W203.slug().to_owned(),
                severity: DiagCode::W203.severity(),
                message: format!("import alias '{}' is never used", a.alias),
            });
        }
    }

    for fi in &index.from_imports {
        for n in &fi.names {
            let effective = n.alias.as_deref().unwrap_or(n.name.as_str());
            if !used_names.contains(effective) {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: fi.span.start_line,
                    col: fi.span.start_col,
                    code: DiagCode::W203.code_str().to_owned(),
                    slug: DiagCode::W203.slug().to_owned(),
                    severity: DiagCode::W203.severity(),
                    message: format!("imported name '{}' is never used", effective),
                });
            }
        }
    }
}

// ── W303: duplicate-import-alias ─────────────────────────────────────────────

fn check_w303(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for a in &index.import_aliases {
        let count = seen.entry(a.alias.as_str()).or_insert(0);
        *count += 1;
        if *count == 2 {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: a.span.start_line,
                col: a.span.start_col,
                code: DiagCode::W303.code_str().to_owned(),
                slug: DiagCode::W303.slug().to_owned(),
                severity: DiagCode::W303.severity(),
                message: format!("import alias '{}' defined more than once", a.alias),
            });
        }
    }
}

// ── W304: duplicate-from-import ───────────────────────────────────────────────

fn check_w304(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for fi in &index.from_imports {
        for n in &fi.names {
            let effective = n.alias.as_deref().unwrap_or(n.name.as_str());
            let count = seen.entry(effective).or_insert(0);
            *count += 1;
            if *count == 2 {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: fi.span.start_line,
                    col: fi.span.start_col,
                    code: DiagCode::W304.code_str().to_owned(),
                    slug: DiagCode::W304.slug().to_owned(),
                    severity: DiagCode::W304.severity(),
                    message: format!("'{}' imported more than once", effective),
                });
            }
        }
    }
}

// ── W305: name-shadowing ──────────────────────────────────────────────────────

fn check_w305(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let vars = &index.variables;
    for (i, inner) in vars.iter().enumerate() {
        let inner_start = inner.valid_range.start_byte;
        let inner_end = inner.valid_range.end_byte;
        if inner_start >= inner_end {
            continue;
        }
        for outer in vars[..i].iter() {
            if outer.name != inner.name {
                continue;
            }
            let outer_start = outer.valid_range.start_byte;
            let outer_end = outer.valid_range.end_byte;
            if outer_start >= outer_end {
                continue;
            }
            // Inner is nested within outer.
            if outer_start <= inner_start && inner_end <= outer_end {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: inner.span.start_line,
                    col: inner.span.start_col,
                    code: DiagCode::W305.code_str().to_owned(),
                    slug: DiagCode::W305.slug().to_owned(),
                    severity: DiagCode::W305.severity(),
                    message: format!("'{}' shadows an outer-scope variable", inner.name),
                });
                break; // one diagnostic per shadowed var is enough
            }
        }
    }
}

// ── E403: missing-required-block ─────────────────────────────────────────────

fn check_e403(path: &str, index: &TemplateIndex, workspace: &WorkspaceIndex, out: &mut Vec<Diagnostic>) {
    // Only applies to child templates.
    let extends = index.template_refs.iter().find(|r| matches!(r.kind, TemplateRefKind::Extends));
    let Some(parent_ref) = extends else { return };
    let Some(parent_idx) = workspace.get_by_ref(&parent_ref.path) else { return };

    let child_block_names: std::collections::HashSet<&str> =
        index.blocks.iter().map(|b| b.name.as_str()).collect();

    for pb in &parent_idx.blocks {
        if pb.required && !child_block_names.contains(pb.name.as_str()) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: 0,
                col: 0,
                code: DiagCode::E403.code_str().to_owned(),
                slug: DiagCode::E403.slug().to_owned(),
                severity: DiagCode::E403.severity(),
                message: format!("required block '{}' is not overridden in this template", pb.name),
            });
        }
    }
}

// ── W402: unreachable-content / E401: invalid-super ──────────────────────────

fn check_w402_e401(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    // Only applies to child templates (those that extend a parent).
    let is_child = index.template_refs.iter().any(|r| matches!(r.kind, TemplateRefKind::Extends));
    if !is_child {
        return;
    }

    // Collect block body byte ranges ([body_start, body_end) = content between the tags).
    let block_ranges: Vec<(usize, usize)> = index.blocks.iter()
        .filter(|b| b.body.start_byte < b.body.end_byte)
        .map(|b| (b.body.start_byte, b.body.end_byte))
        .collect();

    // A top-level {% macro %} is valid Jinja (callable from within blocks) — set/for
    // bindings inside its body are exempt from W402 the same way block bodies are.
    let macro_ranges: Vec<(usize, usize)> = index.macros.iter()
        .filter(|m| m.body.start_byte < m.body.end_byte)
        .map(|m| (m.body.start_byte, m.body.end_byte))
        .collect();

    let inside_block = |byte: usize| {
        block_ranges.iter().any(|&(s, e)| s <= byte && byte < e)
            || macro_ranges.iter().any(|&(s, e)| s <= byte && byte < e)
    };

    // W402: variables set outside any block are unreachable in a child template.
    for v in &index.variables {
        if !inside_block(v.span.start_byte) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: v.span.start_line,
                col: v.span.start_col,
                code: DiagCode::W402.code_str().to_owned(),
                slug: DiagCode::W402.slug().to_owned(),
                severity: DiagCode::W402.severity(),
                message: format!("'{}' is outside any block and will not render in this extends template", v.name),
            });
        }
    }

    // E401: {{ super() }} outside any block has no parent block context.
    // Use the grammar-driven Function references (not a raw byte scan) so HTML prose,
    // comments, and other text outside Jinja delimiters can never match, and so the
    // reported span is the tree-sitter byte span every other check already uses.
    for r in &index.references {
        if r.kind == ReferenceKind::Function && r.name == "super" && !inside_block(r.span.start_byte) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: r.span.start_line,
                col: r.span.start_col,
                code: DiagCode::E401.code_str().to_owned(),
                slug: DiagCode::E401.slug().to_owned(),
                severity: DiagCode::E401.severity(),
                message: "super() called outside a block".to_owned(),
            });
        }
    }
}

// ── E404: recursive-import ────────────────────────────────────────────────────

fn check_e404(path: &str, index: &TemplateIndex, workspace: &WorkspaceIndex, out: &mut Vec<Diagnostic>) {
    for tr in &index.template_refs {
        if tr.is_dynamic || tr.ignore_missing {
            continue;
        }
        if !matches!(tr.kind, TemplateRefKind::Extends | TemplateRefKind::Import | TemplateRefKind::From) {
            continue;
        }
        let mut visited = HashSet::new();
        visited.insert(path.to_owned());
        if import_chain_contains(tr.path.as_str(), path, &mut visited, workspace) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: tr.span.start_line,
                col: tr.span.start_col,
                code: DiagCode::E404.code_str().to_owned(),
                slug: DiagCode::E404.slug().to_owned(),
                severity: DiagCode::E404.severity(),
                message: format!("import of '{}' creates a recursive cycle", tr.path),
            });
        }
    }
}

fn import_chain_contains(current: &str, target: &str, visited: &mut HashSet<String>, workspace: &WorkspaceIndex) -> bool {
    // Resolve current to the workspace key (handles relative ref vs absolute key mismatch).
    let current_key = match workspace.resolve_key(current) {
        Some(k) => k.to_owned(),
        None => return false,
    };
    if current_key == target {
        return true;
    }
    if !visited.insert(current_key.clone()) {
        return false;
    }
    let Some(idx) = workspace.templates.get(&current_key) else { return false };
    for tr in &idx.template_refs {
        if tr.is_dynamic || tr.ignore_missing {
            continue;
        }
        if !matches!(tr.kind, TemplateRefKind::Extends | TemplateRefKind::Import | TemplateRefKind::From) {
            continue;
        }
        if import_chain_contains(tr.path.as_str(), target, visited, workspace) {
            return true;
        }
    }
    false
}

// ── E501: wrong-call-args ─────────────────────────────────────────────────────

fn check_e501(path: &str, index: &TemplateIndex, workspace: &WorkspaceIndex, out: &mut Vec<Diagnostic>) {
    for call in &index.macro_calls {
        // Resolve the callee macro definition (local → from-imports → workspace-wide).
        let Some(mac) = resolve_macro(call.callee.as_str(), index, workspace) else { continue };

        let required_count = mac.parameters.iter().filter(|p| p.default.is_none()).count();
        let total_count = mac.parameters.len();
        let given_positional = call.positional_count;
        let given_keywords: HashSet<&str> = call.keyword_names.iter().map(|s| s.as_str()).collect();

        // Check for unknown keyword args.
        for kw in &call.keyword_names {
            if !mac.parameters.iter().any(|p| p.name == *kw) {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: call.span.start_line,
                    col: call.span.start_col,
                    code: DiagCode::E501.code_str().to_owned(),
                    slug: DiagCode::E501.slug().to_owned(),
                    severity: DiagCode::E501.severity(),
                    message: format!("'{}': unexpected keyword argument '{}'", call.callee, kw),
                });
            }
        }

        // Count how many required params are already satisfied by keyword args.
        let required_by_keyword = mac.parameters.iter()
            .filter(|p| p.default.is_none() && given_keywords.contains(p.name.as_str()))
            .count();
        let required_positional_needed = required_count.saturating_sub(required_by_keyword);

        // Too few positional args.
        if given_positional < required_positional_needed {
            let missing = required_positional_needed - given_positional;
            out.push(Diagnostic {
                file: path.to_owned(),
                line: call.span.start_line,
                col: call.span.start_col,
                code: DiagCode::E501.code_str().to_owned(),
                slug: DiagCode::E501.slug().to_owned(),
                severity: DiagCode::E501.severity(),
                message: format!(
                    "'{}': missing {} required argument(s) (expected at least {}, got {})",
                    call.callee, missing, required_count, given_positional
                ),
            });
            continue;
        }

        // Too many positional args.
        if given_positional > total_count {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: call.span.start_line,
                col: call.span.start_col,
                code: DiagCode::E501.code_str().to_owned(),
                slug: DiagCode::E501.slug().to_owned(),
                severity: DiagCode::E501.severity(),
                message: format!(
                    "'{}': too many positional arguments (expected at most {}, got {})",
                    call.callee, total_count, given_positional
                ),
            });
        }
    }
}

fn resolve_macro<'a>(callee: &str, index: &'a TemplateIndex, workspace: &'a WorkspaceIndex) -> Option<&'a MacroDefinition> {
    // Local macro.
    if let Some(m) = index.macros.iter().find(|m| m.name == callee) {
        return Some(m);
    }
    // From-imports.
    for fi in &index.from_imports {
        let Some(orig) = fi.names.iter()
            .find(|n| n.alias.as_deref().unwrap_or(n.name.as_str()) == callee)
            .map(|n| n.name.as_str())
        else { continue };
        if let Some(src_idx) = workspace.get_by_ref(&fi.source) {
            if let Some(m) = src_idx.macros.iter().find(|m| m.name == orig) {
                return Some(m);
            }
        }
    }
    // Workspace-wide.
    workspace.find_macro_workspace_wide(callee)
}

// ── E601: template-does-not-exist ─────────────────────────────────────────────

fn check_e601(path: &str, index: &TemplateIndex, workspace: &WorkspaceIndex, out: &mut Vec<Diagnostic>) {
    for tr in &index.template_refs {
        if tr.is_dynamic || tr.ignore_missing {
            continue;
        }
        if matches!(tr.kind, TemplateRefKind::Extends | TemplateRefKind::Include | TemplateRefKind::Import | TemplateRefKind::From)
            && workspace.get_by_ref(&tr.path).is_none()
        {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: tr.span.start_line,
                col: tr.span.start_col,
                code: DiagCode::E601.code_str().to_owned(),
                slug: DiagCode::E601.slug().to_owned(),
                severity: DiagCode::E601.severity(),
                message: format!("template '{}' does not exist", tr.path),
            });
        }
    }
}
