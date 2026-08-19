use super::*;

// ── W203: unused-import ───────────────────────────────────────────────────────

pub(super) fn check_w203(
    source: &str,
    path: &str,
    index: &TemplateIndex,
    out: &mut Vec<Diagnostic>,
) {
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
                    let before_ok = pos == 0
                        || !(src_bytes[pos - 1].is_ascii_alphanumeric()
                            || src_bytes[pos - 1] == b'_');
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
