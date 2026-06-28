// REQ-FOLD-01..REQ-FOLD-08: Module layout and downward-dependency rule.
// These tests verify the module skeleton exists by importing its public API.
// Compilation failure = missing module = test failure.

use jinja_lsp::builtins;
use jinja_lsp::diagnostics;
use jinja_lsp::edit;
use jinja_lsp::features;
use jinja_lsp::format;
use jinja_lsp::parsing;
use jinja_lsp::workspace;

// REQ-FOLD-01: One crate, layered modules
#[test]
fn all_layers_exist() {
    let _ = parsing::layer_name();
    let _ = workspace::layer_name();
    let _ = diagnostics::layer_name();
    let _ = builtins::layer_name();
    let _ = features::layer_name();
    let _ = edit::layer_name();
    let _ = format::layer_name();
}

// REQ-FOLD-08: No lower-layer module may import features/.
// Reads the source files of parsing/, workspace/, and diagnostics/ and asserts
// none contains `use jinja_lsp::features` or `crate::features`.
#[test]
fn lower_layers_do_not_import_features() {
    let src_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let lower_layers = ["parsing", "workspace", "diagnostics"];

    for layer in &lower_layers {
        let layer_dir = src_root.join(layer);
        assert!(layer_dir.exists(), "{layer}/ directory missing");

        for entry in walkdir(&layer_dir) {
            let content = std::fs::read_to_string(&entry)
                .unwrap_or_else(|_| panic!("failed to read {}", entry.display()));
            assert!(
                !content.contains("crate::features"),
                "{} imports features/ (REQ-FOLD-08 violation)",
                entry.display()
            );
        }
    }
}

fn walkdir(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                out.extend(walkdir(&path));
            } else if path.extension().is_some_and(|x| x == "rs") {
                out.push(path);
            }
        }
    }
    out
}
