use super::*;

// ── W202: unused-macro ────────────────────────────────────────────────────────

pub(super) fn check_w202(
    path: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
    out: &mut Vec<Diagnostic>,
) {
    // jinja-lsp-md8e: a template with no macros can never produce a W202, so skip the
    // O(workspace) scan entirely for the common case.
    if index.macros.is_empty() {
        return;
    }

    // Pass 2 (cross-file): collect every macro name referenced anywhere in the workspace.
    // A macro is "used" if called locally OR imported/called from any other template.
    // jinja-lsp-md8e: borrow &str keys instead of cloning every reference/import name.
    let mut used: HashSet<&str> = HashSet::new();

    // Own references (local calls inside the macro library itself).
    for r in &index.references {
        if matches!(r.kind, ReferenceKind::Function | ReferenceKind::Identifier) {
            used.insert(r.name.as_str());
        }
    }

    // Workspace-wide scan: other templates that call or import from `path`.
    for tmpl in workspace.templates.values() {
        // Direct function calls and references in any template.
        for r in &tmpl.references {
            if r.kind == ReferenceKind::Function {
                used.insert(r.name.as_str());
            }
        }
        // from-imports that source from the current template count as "exporting" the macro.
        for fi in &tmpl.from_imports {
            if (workspace.resolve_key(&fi.source) == Some(path)) || fi.source == path {
                for n in &fi.names {
                    used.insert(n.name.as_str());
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
                message: format!(
                    "macro '{}' is defined but never used anywhere in the workspace",
                    m.name
                ),
            });
        }
    }
}
