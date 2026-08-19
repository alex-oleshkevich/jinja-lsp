use super::*;

// ── E103: undefined-function ──────────────────────────────────────────────────

pub(super) fn check_e103(
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
        if !matches!(
            index.resolve_reference(r, workspace),
            ResolvedBinding::HostOwned
        ) {
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
