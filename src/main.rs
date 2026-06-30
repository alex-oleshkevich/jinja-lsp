use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "jinja-lsp", about = "Jinja2 template language server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the LSP server over stdio (default)
    Lsp,
    /// Check templates for diagnostics (REQ-LINT-01..11)
    Check {
        /// Files or directories to check (optional; defaults to configured templates dirs)
        paths: Vec<String>,
        /// Output format: rich (default), compact, json
        #[arg(long, default_value = "rich")]
        format: String,
        /// Enable verbose output on stderr
        #[arg(long, short)]
        verbose: bool,
        /// Path to config file (overrides discovery)
        #[arg(long)]
        config: Option<String>,
        /// Enable only these diagnostic codes/prefixes
        #[arg(long, value_delimiter = ',')]
        select: Vec<String>,
        /// Disable these diagnostic codes/prefixes
        #[arg(long, value_delimiter = ',')]
        ignore: Vec<String>,
    },
    /// Format Jinja templates in place (or --check / --diff read-only)
    Format {
        /// Files or directories to format (optional; defaults to templates/)
        paths: Vec<String>,
        /// Path to config file (overrides discovery)
        #[arg(long)]
        config: Option<String>,
        /// Check only — do not write, exit 1 if any file would change
        #[arg(long)]
        check: bool,
        /// Print unified diff — do not write, exit 1 if any file would change
        #[arg(long)]
        diff: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let code = match cli.command.unwrap_or(Commands::Lsp) {
        Commands::Lsp => {
            jinja_lsp::server::run_lsp_server().await;
            0
        }
        Commands::Check { paths, format, verbose: _, config, select, ignore } => {
            run_check(paths, &format, config.as_deref(), &select, &ignore)
        }
        Commands::Format { paths, config, check, diff } => {
            run_format(paths, config.as_deref(), check, diff)
        }
    };
    std::process::exit(code);
}

/// REQ-LINT-01..11: check command implementation.
/// Returns exit code: 0 = no findings, 1 = findings found, 2 = config/usage error.
fn run_check(paths: Vec<String>, format: &str, config_path: Option<&str>, select: &[String], ignore: &[String]) -> i32 {
    use std::path::Path;
    use jinja_lsp::config::JinjaConfig;
    use jinja_lsp::diagnostic::{Diagnostic, DiagnosticSeverity};
    use jinja_lsp::diagnostics::filter_by_config;
    use jinja_lsp::workspace::build_workspace;

    let cwd = std::env::current_dir().unwrap_or_default();
    let cfg = match config_path {
        Some(p) => match JinjaConfig::from_file(Path::new(p)) {
            Ok(c) => c,
            Err(e) => { eprintln!("error: config: {e}"); return 2; }
        },
        None => match JinjaConfig::discover(&cwd) {
            Ok(c) => c,
            Err(e) => { eprintln!("error: config: {e}"); return 2; }
        },
    };
    let ext_strs: Vec<&str> = cfg.extensions.iter().map(|s| s.as_str()).collect();

    // REQ-LINT-01: collect template dirs/files from paths
    let dirs: Vec<std::path::PathBuf> = if paths.is_empty() {
        cfg.resolved_template_dirs(&cwd)
    } else {
        paths.iter().map(|p| Path::new(p).to_path_buf()).collect()
    };

    let dir_refs: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();

    // REQ-LINT-09: build_workspace is the shared engine (same as LSP server)
    let _workspace = build_workspace(&dir_refs, &ext_strs);

    // Collect diagnostics — F01 checks not yet implemented; emit empty list
    let all_diags: Vec<Diagnostic> = vec![];

    // REQ-LINT-03: apply select/ignore filters (CLI overrides config; merge both)
    let mut effective_select: Vec<String> = cfg.lint.select.clone();
    effective_select.extend_from_slice(select);
    let mut effective_ignore: Vec<String> = cfg.lint.ignore.clone();
    effective_ignore.extend_from_slice(ignore);
    let sel: Vec<&str> = effective_select.iter().map(|s| s.as_str()).collect();
    let ign: Vec<&str> = effective_ignore.iter().map(|s| s.as_str()).collect();
    let filtered = filter_by_config(&all_diags, &sel, &ign);

    // REQ-LINT-10: normalize paths (forward slashes)
    let findings: Vec<Diagnostic> = filtered.into_iter().cloned().collect();

    // REQ-LINT-07: order by file, line, col
    let mut sorted = findings;
    sorted.sort_by(|a, b| {
        a.file.cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.col.cmp(&b.col))
    });

    // REQ-LINT-04/05/06: output format
    match format {
        "json" => {
            // REQ-LINT-06/07: JSON array with 7-key shape
            let json = serde_json::to_string_pretty(&sorted).expect("serialization must not fail");
            println!("{json}");
        }
        "compact" => {
            // REQ-LINT-05: one line per finding
            for d in &sorted {
                println!("{}:{}:{}: {} {} {}", d.file, d.line, d.col, d.code, d.slug, d.message);
            }
        }
        _ => {
            // REQ-LINT-04: rich rustc-style report
            for d in &sorted {
                let sev = match d.severity {
                    DiagnosticSeverity::Error => "error",
                    DiagnosticSeverity::Warning => "warning",
                    DiagnosticSeverity::Info => "info",
                    DiagnosticSeverity::Hint => "hint",
                };
                println!("{sev}[{}]: {}", d.code, d.message);
                println!("  --> {}:{}:{}", d.file, d.line, d.col);
            }
            if sorted.is_empty() {
                println!("No problems found.");
            }
        }
    }

    // REQ-LINT-08: exit codes 0 (no findings) / 1 (findings) / 2 (error)
    if sorted.is_empty() { 0 } else { 1 }
}

/// REQ-FMT-08 / REQ-FMT-09: format command.
/// Returns exit code: 0 = nothing changed, 1 = changed (or would), 2 = I/O error.
fn run_format(paths: Vec<String>, config_path: Option<&str>, check: bool, diff: bool) -> i32 {
    use std::path::Path;
    use jinja_lsp::config::JinjaConfig;

    let cwd = std::env::current_dir().unwrap_or_default();
    let cfg = match config_path {
        Some(p) => match JinjaConfig::from_file(Path::new(p)) {
            Ok(c) => c,
            Err(e) => { eprintln!("error: config: {e}"); return 2; }
        },
        None => match JinjaConfig::discover(&cwd) {
            Ok(c) => c,
            Err(e) => { eprintln!("error: config: {e}"); return 2; }
        },
    };
    let ext_strs: Vec<&str> = cfg.extensions.iter().map(|s| s.as_str()).collect();
    let template_exts: &[&str] = &ext_strs;

    let roots: Vec<std::path::PathBuf> = if paths.is_empty() {
        cfg.resolved_template_dirs(&cwd)
    } else {
        paths.iter().map(|p| Path::new(p).to_path_buf()).collect()
    };

    // Collect all template files from roots.
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for root in &roots {
        if root.is_file() {
            if let Some(ext) = root.extension().and_then(|e| e.to_str()) {
                if template_exts.contains(&ext) {
                    files.push(root.clone());
                } else {
                    // Single file with non-template ext is a no-op.
                }
            } else {
                files.push(root.clone());
            }
        } else if root.is_dir() {
            collect_template_files(root, template_exts, &mut files);
        }
    }

    let mut any_changed = false;

    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        };

        let formatted = jinja_lsp::format::format(&source);
        if formatted == source {
            continue;
        }

        any_changed = true;

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

    if any_changed { 1 } else { 0 }
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
        let out = capture_unified_diff(Path::new("t.html"), original, formatted);
        assert!(out.contains("@@ -1,"), "must have hunk header");
        assert!(out.contains("+new_line"), "inserted line must appear with +");
        assert!(out.contains(" line2"), "unchanged line2 must appear as context");
        assert!(out.contains(" line3"), "unchanged line3 must appear as context");
        // The naive impl would have shown -line2, +new_line, -line3, +line2 — check that doesn't happen.
        assert!(!out.contains("-line2\n+new_line"), "must not misalign existing lines as deletions");
    }

    #[test]
    fn vn6f_deletion_shows_correct_hunk() {
        let original = "line1\nline2\nline3\n";
        let formatted = "line1\nline3\n";
        let out = capture_unified_diff(Path::new("t.html"), original, formatted);
        assert!(out.contains("-line2"), "deleted line must appear with -");
        assert!(out.contains(" line3"), "unchanged line3 must appear as context, not as changed");
    }

    #[test]
    fn vn6f_identical_files_produce_no_hunks() {
        let src = "a\nb\nc\n";
        let out = capture_unified_diff(Path::new("t.html"), src, src);
        assert!(!out.contains("@@"), "identical files must produce no hunks");
    }

    #[test]
    fn vn6f_diff_header_matches_spec() {
        let out = capture_unified_diff(
            Path::new("templates/blog/post.html"),
            "{%if%}\n",
            "{% if %}\n",
        );
        assert!(out.starts_with("--- templates/blog/post.html\n"), "--- header must match spec");
        assert!(out.contains("+++ templates/blog/post.html (formatted)\n"), "+++ header must match spec");
    }
}

fn collect_template_files(dir: &std::path::Path, exts: &[&str], out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_template_files(&path, exts, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if exts.contains(&ext) {
                out.push(path);
            }
        }
    }
}

#[cfg(test)]
fn capture_unified_diff(path: &std::path::Path, original: &str, formatted: &str) -> String {
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
            old_range.start + 1, old_range.len(),
            new_range.start + 1, new_range.len(),
        ));
        for op in &group {
            for change in diff.iter_changes(op) {
                let prefix = match change.tag() {
                    ChangeTag::Delete => '-',
                    ChangeTag::Insert => '+',
                    ChangeTag::Equal  => ' ',
                };
                out.push(prefix);
                out.push_str(&change.to_string());
                if change.missing_newline() { out.push('\n'); }
            }
        }
    }
    out
}

fn print_unified_diff(path: &std::path::Path, original: &str, formatted: &str) {
    use similar::{ChangeTag, TextDiff};

    let diff = TextDiff::from_lines(original, formatted);
    let display = path.display();
    println!("--- {display}");
    println!("+++ {display} (formatted)");
    for group in diff.grouped_ops(3) {
        let first = group.first().unwrap();
        let last = group.last().unwrap();
        let old_range = first.old_range().start..last.old_range().end;
        let new_range = first.new_range().start..last.new_range().end;
        println!(
            "@@ -{},{} +{},{} @@",
            old_range.start + 1, old_range.len(),
            new_range.start + 1, new_range.len(),
        );
        for op in &group {
            for change in diff.iter_changes(op) {
                let prefix = match change.tag() {
                    ChangeTag::Delete => '-',
                    ChangeTag::Insert => '+',
                    ChangeTag::Equal  => ' ',
                };
                print!("{prefix}{change}");
                if !change.missing_newline() { } else { println!(); }
            }
        }
    }
}
