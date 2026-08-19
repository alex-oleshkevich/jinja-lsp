use super::*;

// ── W304: duplicate-from-import ───────────────────────────────────────────────

pub(super) fn check_w304(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for fi in &index.from_imports {
        for n in &fi.names {
            let effective = n.alias.as_deref().unwrap_or(n.name.as_str());
            let count = seen.entry(effective).or_insert(0);
            *count += 1;
            if *count >= 2 {
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
