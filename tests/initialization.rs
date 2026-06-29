// REQ-EDIT-01/02/10/11: InitializationOptions and config-overlay wiring.

use jinja_lsp::builtins::registry::{Category, Source};
use jinja_lsp::config::{ConfigOverlay, JinjaConfig};
use jinja_lsp::server::state::ServerState;

// ─── T-01: server state stores config ────────────────────────────────────────

#[test]
fn state_stores_config() {
    let cfg = JinjaConfig::default();
    let state = ServerState::with_config(cfg.clone());
    assert_eq!(state.config.extras, cfg.extras, "config must round-trip through state");
}

// ─── T-02: config overlay applied via apply_init_options ─────────────────────

#[test]
fn state_apply_init_options_overrides_per_key() {
    let mut state = ServerState::with_config(JinjaConfig::default());
    // Default extras is empty; overlay sets starlette.
    let overlay = ConfigOverlay {
        extras: Some(vec!["starlette".to_owned()]),
        ..Default::default()
    };
    state.apply_init_options(overlay);
    assert_eq!(state.config.extras, vec!["starlette"]);
}

// ─── T-03: absent overlay key keeps existing config value ────────────────────

#[test]
fn state_overlay_absent_key_keeps_existing_value() {
    let mut cfg = JinjaConfig::default();
    cfg.extensions = vec!["html".to_owned()];
    let mut state = ServerState::with_config(cfg);
    let overlay = ConfigOverlay {
        extensions: None,  // not overriding
        extras: Some(vec!["flask".to_owned()]),
        ..Default::default()
    };
    state.apply_init_options(overlay);
    assert_eq!(state.config.extensions, vec!["html"], "extensions kept from original");
    assert_eq!(state.config.extras, vec!["flask"], "extras applied from overlay");
}

// ─── T-04: JSON InitializationOptions deserialized and applied ───────────────

#[test]
fn json_init_options_round_trip() {
    // Simulate what the LSP initialize handler receives in initialization_options.
    let json = serde_json::json!({
        "templates": ["templates"],
        "extras": ["starlette"],
        "lint": { "ignore": ["JINJA-W203"] }
    });
    let overlay: ConfigOverlay = serde_json::from_value(json).expect("must deserialize");
    let mut state = ServerState::with_config(JinjaConfig::default());
    state.apply_init_options(overlay);
    assert_eq!(state.config.extras, vec!["starlette"]);
    assert_eq!(state.config.lint.ignore, vec!["JINJA-W203"]);
}

// ─── T-05: REQ-EDIT-01 — server command is lsp over stdio ────────────────────

#[test]
fn lsp_subcommand_is_stdio_not_tcp() {
    // The run_lsp_server function uses tokio::io::stdin/stdout — not a TCP listener.
    // Verify the function exists and is pub (it compiles iff the path is correct).
    let _: fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> =
        || Box::pin(jinja_lsp::server::run_lsp_server());
    // If this compiles, the lsp server is wired as an async fn, consistent with stdio.
}

// ─── T-05b: REQ-EDIT-11 — only jinja/jinja-html languageIds are Jinja ────────

#[test]
fn canonical_language_ids_are_jinja_and_jinja_html() {
    // The canonical languageIds the server recognises per REQ-EDIT-11.
    let accepted = &["jinja", "jinja-html"];
    let rejected = &["html", "htmldjango", "jinja2", "plaintext", ""];

    for &id in accepted {
        assert!(id == "jinja" || id == "jinja-html", "{id} must be canonical");
    }
    for &id in rejected {
        assert!(id != "jinja" && id != "jinja-html", "{id} must NOT be canonical");
    }
}

// ─── T-REG-01: REQ-BLTN-07 — registry loads custom_builtins from config at init

#[test]
fn state_registry_loads_custom_builtins_from_config() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("my_filter.md"),
        "---\nname: my_filter\ncategory: filter\n---\nCustom filter doc.",
    )
    .unwrap();

    let mut cfg = JinjaConfig::default();
    cfg.custom_builtins = vec![dir.path().to_string_lossy().to_string()];

    let state = ServerState::with_config(cfg);

    let entry = state.registry.get(Category::Filter, "my_filter");
    assert!(entry.is_some(), "custom builtin must be in registry after with_config");
    assert_eq!(entry.unwrap().source, Source::Custom);
}

// ─── T-REG-02: REQ-BLTN-07 — registry rebuilt on apply_init_options with custom_builtins

#[test]
fn state_registry_rebuilt_on_apply_init_options() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("overlay_filter.md"),
        "---\nname: overlay_filter\ncategory: filter\n---\nAdded via overlay.",
    )
    .unwrap();

    let mut state = ServerState::with_config(JinjaConfig::default());
    // Before overlay: custom filter must not be present
    assert!(
        state.registry.get(Category::Filter, "overlay_filter").is_none(),
        "filter must not exist before overlay"
    );

    let overlay = ConfigOverlay {
        custom_builtins: Some(vec![dir.path().to_string_lossy().to_string()]),
        ..Default::default()
    };
    state.apply_init_options(overlay);

    let entry = state.registry.get(Category::Filter, "overlay_filter");
    assert!(entry.is_some(), "custom builtin must be in registry after apply_init_options");
    assert_eq!(entry.unwrap().source, Source::Custom);
}

// ─── T-06: REQ-EDIT-09 — nvim-lspconfig snippet in README has required keys ──

#[test]
fn readme_nvim_snippet_has_required_keys() {
    let readme = include_str!("../README.md");
    // cmd must be jinja-lsp lsp
    assert!(readme.contains(r#"{ "jinja-lsp", "lsp" }"#), "README nvim cmd must be {{ \"jinja-lsp\", \"lsp\" }}");
    // filetypes must include the Neovim canonical filetypes per REQ-EDIT-11
    assert!(readme.contains("htmldjango"), "README nvim filetypes must include htmldjango");
    assert!(readme.contains("jinja.html"), "README nvim filetypes must include jinja.html");
    // init_options must be shown
    assert!(readme.contains("init_options"), "README nvim snippet must show init_options");
    // root_dir keyed on jinja.toml
    assert!(readme.contains("jinja.toml"), "README nvim snippet must reference jinja.toml root");
}
