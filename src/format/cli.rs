//! REQ-FOLD-01 / REQ-FMT-08/09: the `format` front-end — file walking, the
//! `--check` / `--diff` modes and unified-diff rendering. The engine itself is
//! `super`; this module is only its command-line orchestration.

use crate::parsing::discover_templates;

/// REQ-FMT-08 / REQ-FMT-09: format command.
/// Returns exit code: 0 = nothing changed, 1 = changed (or would), 2 = I/O error.
pub fn run_format(
    paths: Vec<String>,
    config_path: Option<&str>,
    check: bool,
    diff: bool,
    output: Option<&str>,
) -> i32 {
    use crate::config::JinjaConfig;
    use std::path::Path;

    let cwd = std::env::current_dir().unwrap_or_default();
    let cfg = match config_path {
        Some(p) => match JinjaConfig::from_file(Path::new(p)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: config: {e}");
                return 2;
            }
        },
        None => match JinjaConfig::discover(&cwd) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: config: {e}");
                return 2;
            }
        },
    };
    let ext_strs: Vec<&str> = cfg.extensions.iter().map(|s| s.as_str()).collect();
    let template_exts: &[&str] = &ext_strs;

    let roots: Vec<std::path::PathBuf> = if paths.is_empty() {
        cfg.resolved_template_dirs(&cwd)
    } else {
        paths.iter().map(|p| Path::new(p).to_path_buf()).collect()
    };

    // REQ-FMT-09: silently skip relative paths that escape the templates root via ../ .
    // Absolute paths are always accepted — the user explicitly chose them.
    let template_roots: Vec<std::path::PathBuf> = {
        let config_roots = cfg.resolved_template_dirs(&cwd);
        if config_roots.is_empty() {
            vec![cwd.clone()]
        } else {
            config_roots
        }
    };
    let is_relative_escape = |p: &std::path::Path| -> bool {
        if p.is_absolute() {
            return false;
        }
        // Canonicalize relative to cwd and check if still under a root.
        let canon = cwd.join(p).canonicalize().unwrap_or_else(|_| cwd.join(p));
        !template_roots.iter().any(|r| {
            let r_canon = r.canonicalize().unwrap_or_else(|_| r.clone());
            canon.starts_with(&r_canon)
        })
    };

    // Collect all template files from roots.
    // For explicitly-given file paths the extension filter is skipped — the user
    // chose the file.  For directories we recurse and apply the template extension
    // filter so random non-template files are not accidentally reformatted.
    // Relative paths that escape the templates root via ../ are silently skipped.
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for root in &roots {
        if root.is_file() {
            if !paths.is_empty() && is_relative_escape(root) {
                continue; // silently skip ../-escape paths
            }
            files.push(root.clone());
        } else if root.is_dir() {
            if !paths.is_empty() && is_relative_escape(root) {
                continue; // silently skip ../-escape paths
            }
            files.extend(discover_templates(&[root], template_exts));
        }
    }

    // --output with a non-stdout path only makes sense for a single file.
    if let Some(out) = output {
        if out != "-" && files.len() > 1 {
            eprintln!("error: --output FILE requires a single input file when not '-'");
            return 2;
        }
    }

    let mut changed_count: usize = 0;
    let mut unchanged_count: usize = 0;

    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        };

        let formatted = crate::format::format_with_config(&source, &cfg.format);

        // --output mode: write to stdout or a named file, then stop (no in-place, no check/diff).
        if let Some(out) = output {
            if out == "-" {
                print!("{formatted}");
            } else {
                if let Err(e) = std::fs::write(out, formatted.as_bytes()) {
                    eprintln!("error: {out}: {e}");
                    return 2;
                }
            }
            if formatted != source {
                changed_count += 1;
            } else {
                unchanged_count += 1;
            }
            continue;
        }

        if formatted == source {
            unchanged_count += 1;
            continue;
        }

        changed_count += 1;

        if check {
            // REQ-FMT-08: per-file "would reformat" line in --check mode.
            println!("would reformat: {}", path.display());
        }

        if diff {
            print_unified_diff(path, &source, &formatted);
        }

        if !check && !diff {
            if let Err(e) = std::fs::write(path, formatted.as_bytes()) {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        }
    }

    // --output mode exits without summary.
    if output.is_some() {
        return if changed_count > 0 { 1 } else { 0 };
    }

    // REQ-FMT-08: summary line for --check and --diff modes.
    if check || diff {
        let f = if changed_count == 1 { "file" } else { "files" };
        if check {
            println!("{changed_count} {f} would be reformatted, {unchanged_count} unchanged.");
        } else {
            // diff mode only shows changed count.
            println!("{changed_count} {f} would be reformatted.");
        }
    }

    if changed_count > 0 { 1 } else { 0 }
}

/// Build a unified-diff string for path.  Both tests and the print path share this logic.
fn build_unified_diff(path: &std::path::Path, original: &str, formatted: &str) -> String {
    use similar::{ChangeTag, TextDiff};
    let diff = TextDiff::from_lines(original, formatted);
    let display = path.display();
    let mut out = format!("--- {display}\n+++ {display} (formatted)\n");
    for group in diff.grouped_ops(3) {
        let first = group.first().unwrap();
        let last = group.last().unwrap();
        let old_range = first.old_range().start..last.old_range().end;
        let new_range = first.new_range().start..last.new_range().end;
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            old_range.start + 1,
            old_range.len(),
            new_range.start + 1,
            new_range.len(),
        ));
        for op in &group {
            for change in diff.iter_changes(op) {
                let prefix = match change.tag() {
                    ChangeTag::Delete => '-',
                    ChangeTag::Insert => '+',
                    ChangeTag::Equal => ' ',
                };
                out.push(prefix);
                out.push_str(&change.to_string());
                if change.missing_newline() {
                    out.push('\n');
                }
            }
        }
    }
    out
}

fn print_unified_diff(path: &std::path::Path, original: &str, formatted: &str) {
    print!("{}", build_unified_diff(path, original, formatted));
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn vn6f_insertion_shows_correct_hunk() {
        // A real unified diff should show the inserted line with + prefix and
        // proper @@ hunk coordinates — NOT cascade every subsequent line as changed.
        let original = "line1\nline2\nline3\n";
        let formatted = "line1\nnew_line\nline2\nline3\n";
        let out = build_unified_diff(Path::new("t.html"), original, formatted);
        assert!(out.contains("@@ -1,"), "must have hunk header");
        assert!(
            out.contains("+new_line"),
            "inserted line must appear with +"
        );
        assert!(
            out.contains(" line2"),
            "unchanged line2 must appear as context"
        );
        assert!(
            out.contains(" line3"),
            "unchanged line3 must appear as context"
        );
        // The naive impl would have shown -line2, +new_line, -line3, +line2 — check that doesn't happen.
        assert!(
            !out.contains("-line2\n+new_line"),
            "must not misalign existing lines as deletions"
        );
    }

    #[test]
    fn vn6f_deletion_shows_correct_hunk() {
        let original = "line1\nline2\nline3\n";
        let formatted = "line1\nline3\n";
        let out = build_unified_diff(Path::new("t.html"), original, formatted);
        assert!(out.contains("-line2"), "deleted line must appear with -");
        assert!(
            out.contains(" line3"),
            "unchanged line3 must appear as context, not as changed"
        );
    }

    #[test]
    fn vn6f_identical_files_produce_no_hunks() {
        let src = "a\nb\nc\n";
        let out = build_unified_diff(Path::new("t.html"), src, src);
        assert!(!out.contains("@@"), "identical files must produce no hunks");
    }

    // REQ-LINT-04: rich format tests

    #[test]
    fn vn6f_diff_header_matches_spec() {
        let out = build_unified_diff(
            Path::new("templates/blog/post.html"),
            "{%if%}\n",
            "{% if %}\n",
        );
        assert!(
            out.starts_with("--- templates/blog/post.html\n"),
            "--- header must match spec"
        );
        assert!(
            out.contains("+++ templates/blog/post.html (formatted)\n"),
            "+++ header must match spec"
        );
    }

    // T-18/T-19: color=false must produce no ANSI escape codes
}
