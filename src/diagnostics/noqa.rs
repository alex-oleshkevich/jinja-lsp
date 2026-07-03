// REQ-DIAG-04, REQ-DIAG-05, REQ-DIAG-06: noqa directive parsing and suppression.

use crate::diagnostic::Diagnostic;
use crate::diagnostics::DiagCode;

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
    let mut search = line_text;
    while let Some(start) = search.find("{#") {
        let rest = &search[start + 2..];
        if let Some(end_rel) = rest.find("#}") {
            let raw = rest[..end_rel].trim();
            // Strip whitespace-control '-' markers ({#- ... -#} → "- ... -")
            let raw = raw.strip_prefix('-').unwrap_or(raw).trim();
            let content = raw.strip_suffix('-').unwrap_or(raw).trim();
            if let Some(dir) = parse_comment(content, line_number) {
                directives.push(dir);
            }
            search = &search[start + 2 + end_rel + 2..];
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
    // jinja-lsp-rm5r: individual codes are derived from DiagCode::ALL (the single
    // source of truth next to the enum) instead of a hand-duplicated string list —
    // adding a new DiagCode variant can no longer silently leave it unsuppressable
    // and its noqa usage falsely flagged as W107. Class prefixes remain an explicit,
    // hand-curated set (not every possible prefix is a meaningful suppression class).
    let known_codes: Vec<&str> = DiagCode::ALL.iter().map(|c| c.code_str()).collect();
    const CLASS_PREFIXES: &[&str] = &[
        "JINJA-E", "JINJA-W", "JINJA-E1", "JINJA-W1", "JINJA-W2",
        "JINJA-W3", "JINJA-E4", "JINJA-W4", "JINJA-E5", "JINJA-E6",
    ];
    let all_known_codes: Vec<&str> = known_codes.iter().copied().chain(CLASS_PREFIXES.iter().copied()).collect();
    let all_known_codes: &[&str] = &all_known_codes;

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
                                code: DiagCode::W107.code_str().to_owned(),
                                slug: DiagCode::W107.slug().to_owned(),
                                severity: DiagCode::W107.severity(),
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
            if !file_suppress_codes.is_empty()
                && file_suppress_codes.iter().any(|f| is_valid_noqa_id(f, all_known_codes) && diag.code.starts_with(f.as_str())) {
                return false;
            }
            // REQ-DIAG-05: line-level suppression — check the diagnostic's own line
            // and (for multi-line tags) the tag's opening-delimiter line.
            let tag_open_line = find_enclosing_tag_open_line(&lines, diag.line as usize);
            let suppression_lines: &[u32] = &match tag_open_line {
                Some(ol) if ol != diag.line => [diag.line, ol],
                _ => [diag.line, diag.line],
            };
            for dir in &all_directives {
                if !suppression_lines.contains(&dir.line()) {
                    continue;
                }
                match dir {
                    NoqaDirective::All { .. } => return false,
                    NoqaDirective::Codes { codes, .. }
                        if codes.iter().any(|f| {
                            is_valid_noqa_id(f, all_known_codes) && diag.code.starts_with(f.as_str())
                        }) =>
                    {
                        return false;
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

/// A valid noqa ID must be an exact member of the known-codes list (full codes or class prefixes).
fn is_valid_noqa_id(id: &str, known: &[&str]) -> bool {
    known.contains(&id)
}

/// REQ-DIAG-05: find the opening-delimiter line of the multi-line tag that encloses
/// `diag_line`, if any.  A `{%` that opens on line K but has no matching `%}` on the
/// same line is considered "multi-line"; its line K is the opening delimiter.
///
/// Returns `None` if `diag_line` is 0 or if no unclosed `{%` is found.
fn find_enclosing_tag_open_line(lines: &[&str], diag_line: usize) -> Option<u32> {
    if diag_line == 0 {
        return None;
    }
    for line_no in (0..diag_line).rev() {
        let line = lines[line_no];
        if let Some(open_pos) = line.find("{%") {
            let after_open = &line[open_pos..];
            if !after_open.contains("%}") {
                // {%…} opens here but doesn't close on this line → multi-line tag
                return Some(line_no as u32);
            }
        }
        // A line that closes a tag without opening one (e.g. leading `%}`)
        // means we've left the span of any tag that opened before it.
        if line.contains("%}") && !line.contains("{%") {
            break;
        }
    }
    None
}
