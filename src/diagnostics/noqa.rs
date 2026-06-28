// REQ-DIAG-04, REQ-DIAG-05, REQ-DIAG-06: noqa directive parsing and suppression.

use crate::diagnostic::{Diagnostic, DiagnosticSeverity};

/// A parsed `noqa` directive from a single line of source.
#[derive(Debug, Clone)]
pub enum NoqaDirective {
    /// `{# noqa #}` — suppress all diagnostics on this line.
    All { line: u32 },
    /// `{# noqa: JINJA-E101, JINJA-W2 #}` — suppress listed codes on this line.
    Codes { line: u32, codes: Vec<String> },
    /// `{# noqa-file #}` — suppress all diagnostics in the file.
    FileAll { line: u32 },
    /// `{# noqa-file: JINJA-W2 #}` — suppress listed codes in the file.
    FileCodes { line: u32, codes: Vec<String> },
}

impl NoqaDirective {
    pub fn line(&self) -> u32 {
        match self {
            Self::All { line } | Self::Codes { line, .. } |
            Self::FileAll { line } | Self::FileCodes { line, .. } => *line,
        }
    }
}

/// REQ-DIAG-04: scan one line of source text for noqa directives.
pub fn parse_noqa_directives(line_text: &str, line_number: u32) -> Vec<NoqaDirective> {
    let mut directives = vec![];
    // find all `{# ... #}` comment spans
    let mut search = line_text;
    let mut offset = 0;
    while let Some(start) = search.find("{#") {
        let _comment_start = offset + start;
        let rest = &search[start + 2..];
        // find matching closing `#}`
        if let Some(end_rel) = rest.find("#}") {
            let content = rest[..end_rel].trim();
            if let Some(dir) = parse_comment(content, line_number) {
                directives.push(dir);
            }
            let skip = start + 2 + end_rel + 2;
            offset += skip;
            search = &search[skip..];
        } else {
            break;
        }
    }
    directives
}

fn parse_comment(content: &str, line: u32) -> Option<NoqaDirective> {
    if content == "noqa" {
        return Some(NoqaDirective::All { line });
    }
    if content == "noqa-file" {
        return Some(NoqaDirective::FileAll { line });
    }
    // `noqa: CODE, CODE` or `noqa CODE` (bare space)
    if let Some(rest) = content.strip_prefix("noqa:").or_else(|| {
        let s = content.strip_prefix("noqa")?;
        if s.starts_with(' ') { Some(s) } else { None }
    }) {
        let codes: Vec<String> = rest
            .split([',', ' '])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        if codes.is_empty() {
            return Some(NoqaDirective::All { line });
        }
        return Some(NoqaDirective::Codes { line, codes });
    }
    // `noqa-file: CODE`
    if let Some(rest) = content.strip_prefix("noqa-file:") {
        let codes: Vec<String> = rest
            .split([',', ' '])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        return Some(if codes.is_empty() {
            NoqaDirective::FileAll { line }
        } else {
            NoqaDirective::FileCodes { line, codes }
        });
    }
    None
}

/// REQ-DIAG-05, REQ-DIAG-06: apply noqa suppression to `diags`.
///
/// Returns `(kept, w107s)` where:
/// - `kept` are diagnostics not suppressed
/// - `w107s` are new JINJA-W107 diagnostics for invalid noqa IDs
pub fn suppress_by_noqa(
    diags: &[Diagnostic],
    source: &str,
) -> (Vec<Diagnostic>, Vec<Diagnostic>) {
    let lines: Vec<&str> = source.lines().collect();
    let all_known_codes: &[&str] = &[
        "JINJA-E001", "JINJA-E101", "JINJA-E102", "JINJA-E103", "JINJA-E104",
        "JINJA-W107", "JINJA-W201", "JINJA-W202", "JINJA-W203",
        "JINJA-W301", "JINJA-W302", "JINJA-W303", "JINJA-W304", "JINJA-W305",
        "JINJA-E401", "JINJA-W402", "JINJA-E403", "JINJA-E404",
        "JINJA-E501", "JINJA-E601",
        // class prefixes
        "JINJA-E", "JINJA-W", "JINJA-E1", "JINJA-W1", "JINJA-W2",
        "JINJA-W3", "JINJA-E4", "JINJA-W4", "JINJA-E5", "JINJA-E6",
    ];

    // Collect all directives indexed by line
    let mut all_directives: Vec<NoqaDirective> = vec![];
    let mut w107s: Vec<Diagnostic> = vec![];

    for (line_no, &line_text) in lines.iter().enumerate() {
        let dirs = parse_noqa_directives(line_text, line_no as u32);
        for dir in dirs {
            // REQ-DIAG-06: validate IDs; invalid ones produce W107
            match &dir {
                NoqaDirective::Codes { line, codes } | NoqaDirective::FileCodes { line, codes } => {
                    for code in codes {
                        if !is_valid_noqa_id(code, all_known_codes) {
                            w107s.push(Diagnostic {
                                file: String::new(),
                                line: *line,
                                col: 0,
                                code: "JINJA-W107".to_owned(),
                                slug: "invalid-noqa".to_owned(),
                                severity: DiagnosticSeverity::Warning,
                                message: format!("invalid noqa ID: '{code}'"),
                            });
                        }
                    }
                }
                _ => {}
            }
            all_directives.push(dir);
        }
    }

    // Check for file-level suppression
    let file_suppress_all = all_directives.iter().any(|d| {
        matches!(d, NoqaDirective::FileAll { .. })
    });
    let file_suppress_codes: Vec<String> = all_directives
        .iter()
        .filter_map(|d| match d {
            NoqaDirective::FileCodes { codes, .. } => Some(codes.clone()),
            _ => None,
        })
        .flatten()
        .collect();

    let kept = diags
        .iter()
        .filter(|diag| {
            // file-level suppression
            if file_suppress_all {
                return false;
            }
            if !file_suppress_codes.is_empty() {
                if file_suppress_codes.iter().any(|f| is_valid_noqa_id(f, all_known_codes) && diag.code.starts_with(f.as_str())) {
                    return false;
                }
            }
            // line-level suppression
            for dir in &all_directives {
                if dir.line() != diag.line {
                    continue;
                }
                match dir {
                    NoqaDirective::All { .. } => return false,
                    NoqaDirective::Codes { codes, .. } => {
                        if codes.iter().any(|f| {
                            is_valid_noqa_id(f, all_known_codes) && diag.code.starts_with(f.as_str())
                        }) {
                            return false;
                        }
                    }
                    _ => {}
                }
            }
            true
        })
        .cloned()
        .collect();

    (kept, w107s)
}

/// A valid noqa ID is a full code (`JINJA-E101`) or class prefix (`JINJA-E`).
/// Slugs like `undefined-variable` are NOT valid.
fn is_valid_noqa_id(id: &str, _known: &[&str]) -> bool {
    // Must start with JINJA- and not contain lowercase (slugs have lowercase)
    id.starts_with("JINJA-") && !id.chars().any(|c| c.is_lowercase())
}
