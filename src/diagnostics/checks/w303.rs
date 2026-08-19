use super::*;

// ── W303: duplicate-import-alias ─────────────────────────────────────────────

pub(super) fn check_w303(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for a in &index.import_aliases {
        let count = seen.entry(a.alias.as_str()).or_insert(0);
        *count += 1;
        if *count >= 2 {
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
