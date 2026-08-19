//! REQ-FOLD-01: the `doctor` front-end — report what the server would discover.
//!
//! Answers "why is jinja-lsp not seeing my templates / my filter / my config"
//! without making the user reason about discovery rules. It reports only what is
//! actually resolved, and stays quiet about anything not configured: an empty
//! section is omitted rather than printed as "none".

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use owo_colors::OwoColorize;

use crate::builtins::registry::Registry;
use crate::config::JinjaConfig;
use crate::workspace::build_workspace;

/// Exit code: 0 = healthy, 1 = problems found, 2 = config could not be read.
pub fn run_doctor(config_path: Option<&str>) -> i32 {
    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let mut problems = 0usize;

    println!("jinja-lsp {}", env!("CARGO_PKG_VERSION"));

    let cwd = std::env::current_dir().unwrap_or_default();
    let (cfg, cfg_root, source) = match load_config(config_path, &cwd) {
        Ok(triple) => triple,
        Err(e) => {
            println!();
            println!("{} {e}", paint("config error:", Paint::Err, color));
            return 2;
        }
    };

    section("Config");
    field("source", &source);
    match cfg.validate() {
        Ok(warnings) if warnings.is_empty() => field("status", &paint("ok", Paint::Ok, color)),
        Ok(warnings) => {
            problems += warnings.len();
            for w in &warnings {
                field("warning", &paint(&format!("{w:?}"), Paint::Warn, color));
            }
        }
        Err(e) => {
            problems += 1;
            field("invalid", &paint(&format!("{e}"), Paint::Err, color));
        }
    }

    // ── Templates ────────────────────────────────────────────────────────────
    // Walk what the user *configured*, not `resolved_template_dirs`, which drops
    // entries that do not exist. That is right for the runtime path — a stale
    // entry must not break indexing — but it means a typo'd directory is exactly
    // the failure a user runs `doctor` to find, and it would be invisible here.
    let exts: Vec<&str> = cfg.extensions.iter().map(|s| s.as_str()).collect();
    let auto = JinjaConfig::zero_config_dirs(&cfg_root);
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    if cfg.templates_raw.is_empty() {
        candidates.extend(auto.iter().map(|d| (display_path(d, &cwd), d.clone())));
    }
    for entry in &cfg.templates_raw {
        if entry == "..." {
            candidates.extend(auto.iter().map(|d| (display_path(d, &cwd), d.clone())));
        } else {
            candidates.push((entry.clone(), cfg_root.join(entry)));
        }
    }

    section("Templates");
    if candidates.is_empty() {
        problems += 1;
        field(
            "none",
            &paint(
                "no template directories found — set `templates` in your config",
                Paint::Err,
                color,
            ),
        );
    }
    let mut existing: Vec<PathBuf> = Vec::new();
    for (label, dir) in &candidates {
        if !dir.is_dir() {
            problems += 1;
            field(label, &paint("directory not found", Paint::Err, color));
            continue;
        }
        let count = build_workspace(&[dir.as_path()], &exts).templates.len();
        if count == 0 {
            problems += 1;
            field(
                label,
                &paint(
                    &format!("no matching files (looking for {})", exts.join(", ")),
                    Paint::Warn,
                    color,
                ),
            );
        } else {
            field(label, &plural(count, "template"));
        }
        existing.push(dir.clone());
    }
    field("extensions", &exts.join(", "));

    // ── Builtins ─────────────────────────────────────────────────────────────
    // Counted by diffing the registry as each source loads, so the numbers say
    // what each source actually contributed rather than that it was configured.
    section("Builtins");
    let mut registry = Registry::load_core();
    field("core", &format!("{} entries", registry.entry_count()));

    for extra in &cfg.extras {
        let before = registry.entry_count();
        let loaded = registry.load_packs(&[extra.as_str()]);
        if loaded == 0 {
            problems += 1;
            field(extra, &paint("unknown pack", Paint::Err, color));
        } else {
            field(extra, &added(registry.entry_count() - before));
        }
    }
    report_dirs(
        &cfg.custom_builtins,
        &cfg_root,
        &cwd,
        &mut registry,
        &mut problems,
        color,
        Registry::load_custom_builtins,
    );
    report_dirs(
        &cfg.hints,
        &cfg_root,
        &cwd,
        &mut registry,
        &mut problems,
        color,
        Registry::load_hints_from_dir,
    );

    // Sidecars are discovered per template rather than configured, so they are
    // the least visible source and the most worth surfacing.
    let sidecars = count_sidecars(&existing);
    if sidecars > 0 {
        field("sidecars", &plural(sidecars, "*.hints.md file"));
    }

    // ── Lint ─────────────────────────────────────────────────────────────────
    if !cfg.lint.select.is_empty() || !cfg.lint.ignore.is_empty() {
        section("Lint");
        if !cfg.lint.select.is_empty() {
            field("select", &cfg.lint.select.join(", "));
        }
        if !cfg.lint.ignore.is_empty() {
            field("ignore", &cfg.lint.ignore.join(", "));
        }
    }

    println!();
    if problems == 0 {
        println!("{}", paint("No problems detected.", Paint::Ok, color));
        0
    } else {
        println!("{}", paint(&plural(problems, "problem"), Paint::Err, color));
        1
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

type Loader = fn(&mut Registry, &Path);

#[allow(clippy::too_many_arguments)]
fn report_dirs(
    configured: &[String],
    cfg_root: &Path,
    cwd: &Path,
    registry: &mut Registry,
    problems: &mut usize,
    color: bool,
    load: Loader,
) {
    for dir_str in configured {
        let dir = cfg_root.join(dir_str);
        let shown = display_path(&dir, cwd);
        if !dir.is_dir() {
            *problems += 1;
            field(&shown, &paint("directory not found", Paint::Err, color));
            continue;
        }
        let before = registry.entry_count();
        load(registry, &dir);
        let gained = registry.entry_count() - before;
        if gained == 0 {
            *problems += 1;
            field(&shown, &paint("no entries loaded", Paint::Warn, color));
        } else {
            field(&shown, &added(gained));
        }
    }
}

fn load_config(
    config_path: Option<&str>,
    cwd: &Path,
) -> Result<(JinjaConfig, PathBuf, String), crate::config::ConfigError> {
    match config_path {
        Some(p) => {
            let file = Path::new(p);
            let root = file.parent().map(Path::to_path_buf).unwrap_or_default();
            let cfg = JinjaConfig::from_file(file)?;
            Ok((cfg, root, format!("{} (--config)", file.display())))
        }
        None => {
            let (cfg, found) = JinjaConfig::discover_with_path(cwd)?;
            match found {
                Some(path) => {
                    let root = path.parent().map(Path::to_path_buf).unwrap_or_default();
                    let shown = display_path(&path, cwd);
                    Ok((cfg, root, shown))
                }
                None => Ok((
                    cfg,
                    cwd.to_path_buf(),
                    "none — using zero-config defaults".to_owned(),
                )),
            }
        }
    }
}

/// Count `*.hints.md` files under the template roots.
///
/// Walks the directories rather than asking `find_sidecar` about each indexed
/// template: workspace keys are relative to their template root, so resolving
/// one against the process working directory finds nothing.
fn count_sidecars(dirs: &[PathBuf]) -> usize {
    fn walk(dir: &Path, found: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".hints.md"))
            {
                *found += 1;
            }
        }
    }
    let mut found = 0;
    for dir in dirs {
        walk(dir, &mut found);
    }
    found
}

/// Shorten to a path relative to the working directory when it is below it —
/// an absolute path for every row buries the part that differs.
fn display_path(path: &Path, cwd: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}

fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn added(n: usize) -> String {
    if n == 1 {
        "+1 entry".to_owned()
    } else {
        format!("+{n} entries")
    }
}

fn section(title: &str) {
    println!();
    println!("{title}");
}

fn field(name: &str, value: &str) {
    println!("  {name:<14} {value}");
}

enum Paint {
    Ok,
    Warn,
    Err,
}

fn paint(s: &str, kind: Paint, color: bool) -> String {
    if !color {
        return s.to_owned();
    }
    match kind {
        Paint::Ok => s.green().to_string(),
        Paint::Warn => s.yellow().to_string(),
        Paint::Err => s.red().to_string(),
    }
}
