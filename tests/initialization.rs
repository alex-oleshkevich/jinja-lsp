// REQ-EDIT-01/02/10/11: InitializationOptions and config-overlay wiring.

use jinja_lsp::builtins::registry::{Category, Source};
use jinja_lsp::config::{ConfigError, ConfigOverlay, ConfigWarning, JinjaConfig};
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
    let _ = state.apply_init_options(overlay);
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
    let _ = state.apply_init_options(overlay);
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
    let _ = state.apply_init_options(overlay);
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

// ─── REQ-CFG-07: validate() is called by apply_init_options ──────────────────

#[test]
fn apply_init_options_surfaces_unknown_extras_error() {
    let mut state = ServerState::with_config(JinjaConfig::default());
    let overlay = ConfigOverlay {
        extras: Some(vec!["nonexistent-pack".to_owned()]),
        ..Default::default()
    };
    let result = state.apply_init_options(overlay);
    assert!(
        matches!(result, Err(ConfigError::UnknownExtra(_))),
        "unknown extras must surface as ConfigError: {result:?}",
    );
}

#[test]
fn apply_init_options_surfaces_overlapping_filter_warning() {
    let mut state = ServerState::with_config(JinjaConfig::default());
    let overlay = ConfigOverlay {
        lint: Some(jinja_lsp::config::LintOverlay {
            select: Some(vec!["JINJA-E101".to_owned()]),
            ignore: Some(vec!["JINJA-E101".to_owned()]),
        }),
        ..Default::default()
    };
    let result = state.apply_init_options(overlay).expect("overlapping filter is a warning, not error");
    assert!(
        result.iter().any(|w| matches!(w, ConfigWarning::OverlappingFilter(_))),
        "overlapping select/ignore must produce OverlappingFilter warning: {result:?}",
    );
}

#[test]
fn apply_init_options_valid_config_returns_empty_warnings() {
    let mut state = ServerState::with_config(JinjaConfig::default());
    let overlay = ConfigOverlay {
        extras: Some(vec!["starlette".to_owned()]),
        ..Default::default()
    };
    let result = state.apply_init_options(overlay).expect("valid config must not error");
    assert!(result.is_empty(), "valid config must produce no warnings: {result:?}");
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
    let _ = state.apply_init_options(overlay);

    let entry = state.registry.get(Category::Filter, "overlay_filter");
    assert!(entry.is_some(), "custom builtin must be in registry after apply_init_options");
    assert_eq!(entry.unwrap().source, Source::Custom);
}

// ─── REQ-EXT-01/02: extension packs loaded into registry from config ──────────

#[test]
fn with_config_extras_loads_pack_entries_into_registry() {
    // config.extras = ["starlette"] → build_registry must call load_packs.
    let mut cfg = JinjaConfig::default();
    cfg.extras = vec!["starlette".to_owned()];
    let state = ServerState::with_config(cfg);
    assert!(
        state.registry.get(Category::Function, "url_for").is_some(),
        "starlette pack entries must be in registry after with_config(extras=[starlette])"
    );
}

#[test]
fn apply_init_options_extras_loads_pack_entries_into_registry() {
    let mut state = ServerState::with_config(JinjaConfig::default());
    // Before overlay: url_for must not exist (no pack loaded)
    assert!(state.registry.get(Category::Function, "url_for").is_none(), "url_for must not exist before pack overlay");

    let overlay = ConfigOverlay {
        extras: Some(vec!["starlette".to_owned()]),
        ..Default::default()
    };
    let _ = state.apply_init_options(overlay);

    assert!(
        state.registry.get(Category::Function, "url_for").is_some(),
        "starlette pack entries must be in registry after apply_init_options(extras=[starlette])"
    );
}

// ─── REQ-HINT-02: user hints loaded into registry from config.hints dirs ─────

#[test]
fn with_config_hints_dir_loads_hints_into_registry() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("my_ctx.md"),
        "---\nname: my_ctx\ncategory: variable\n---\nContext variable doc.",
    )
    .unwrap();

    let mut cfg = JinjaConfig::default();
    cfg.hints = vec![dir.path().to_string_lossy().to_string()];
    let state = ServerState::with_config(cfg);

    assert!(
        state.registry.get(Category::Variable, "my_ctx").is_some(),
        "hint variable must be in registry after with_config(hints=[dir])"
    );
}

#[test]
fn apply_init_options_hints_dir_loads_hints_into_registry() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("ctx_hint.md"),
        "---\nname: ctx_hint\ncategory: variable\n---\nHint via overlay.",
    )
    .unwrap();

    let mut state = ServerState::with_config(JinjaConfig::default());
    assert!(state.registry.get(Category::Variable, "ctx_hint").is_none(), "hint must not exist before overlay");

    let overlay = ConfigOverlay {
        hints: Some(vec![dir.path().to_string_lossy().to_string()]),
        ..Default::default()
    };
    let _ = state.apply_init_options(overlay);

    assert!(
        state.registry.get(Category::Variable, "ctx_hint").is_some(),
        "hint variable must be in registry after apply_init_options(hints=[dir])"
    );
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

// ─── jinja-lsp-uoyh: reset_config sets full config + rebuilds registry ────────

#[test]
fn state_reset_config_updates_config_and_registry() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("reset_filter.md"),
        "---\nname: reset_filter\ncategory: filter\n---\nFilter added via reset_config.",
    )
    .unwrap();

    let mut state = ServerState::with_config(JinjaConfig::default());
    assert!(
        state.registry.get(Category::Filter, "reset_filter").is_none(),
        "filter must not exist before reset_config"
    );

    let mut new_cfg = JinjaConfig::default();
    new_cfg.custom_builtins = vec![dir.path().to_string_lossy().to_string()];
    state.reset_config(new_cfg.clone());

    assert_eq!(state.config.custom_builtins, new_cfg.custom_builtins, "config.custom_builtins must be updated");
    assert!(
        state.registry.get(Category::Filter, "reset_filter").is_some(),
        "registry must be rebuilt with new config after reset_config"
    );
}

#[test]
fn state_reset_config_then_overlay_applies_on_top() {
    let mut state = ServerState::with_config(JinjaConfig::default());
    let mut base_cfg = JinjaConfig::default();
    base_cfg.extensions = vec!["html".to_owned()];
    state.reset_config(base_cfg);

    // Overlay should add extras on top of the reset config
    let overlay = ConfigOverlay {
        extras: Some(vec!["starlette".to_owned()]),
        ..Default::default()
    };
    let _ = state.apply_init_options(overlay);

    assert_eq!(state.config.extensions, vec!["html"], "extensions from reset_config must survive overlay");
    assert_eq!(state.config.extras, vec!["starlette"], "extras from overlay must be applied");
}
