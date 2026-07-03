// Config system tests: REQ-CFG-01 through REQ-CFG-11.

use std::fs;

use jinja_lsp::config::{ConfigOverlay, JinjaConfig};
use jinja_lsp::format::FormatterConfig;

fn tmpdir(suffix: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("jinja_lsp_cfg_{suffix}"));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

// ---------- REQ-CFG-01: discover jinja.toml before pyproject.toml ----------

#[test]
fn discovers_jinja_toml_before_pyproject() {
    let root = tmpdir("cfg01_both");
    fs::write(root.join("jinja.toml"), r#"extensions = ["html"]"#).unwrap();
    fs::write(
        root.join("pyproject.toml"),
        "[tool.jinja]\nextensions = [\"jinja\"]\n",
    )
    .unwrap();

    let cfg = JinjaConfig::discover(&root).unwrap();
    assert_eq!(cfg.extensions, vec!["html"], "jinja.toml must win over pyproject.toml");
}

#[test]
fn falls_back_to_pyproject_when_no_jinja_toml() {
    let root = tmpdir("cfg01_pyproject");
    fs::write(
        root.join("pyproject.toml"),
        "[tool.jinja]\nextensions = [\"jinja\"]\n",
    )
    .unwrap();

    let cfg = JinjaConfig::discover(&root).unwrap();
    assert_eq!(cfg.extensions, vec!["jinja"]);
}

#[test]
fn returns_defaults_when_no_config_file() {
    let root = tmpdir("cfg01_none");
    let cfg = JinjaConfig::discover(&root).unwrap();
    // zero-config defaults
    assert!(cfg.extensions.contains(&"html".to_owned()));
}

// ---------- REQ-CFG-02: zero-config template discovery ----------------------

#[test]
fn zero_config_discovers_templates_dir() {
    let root = tmpdir("cfg02_templates");
    fs::create_dir_all(root.join("templates")).unwrap();

    let cfg = JinjaConfig::discover(&root).unwrap();
    let dirs = cfg.resolved_template_dirs(&root);
    assert!(
        dirs.iter().any(|d| d.ends_with("templates")),
        "must discover templates/ dir: {dirs:?}"
    );
}

#[test]
fn zero_config_skips_missing_dirs_silently() {
    let root = tmpdir("cfg02_no_templates");
    // no templates/ dir exists
    let cfg = JinjaConfig::discover(&root).unwrap();
    let dirs = cfg.resolved_template_dirs(&root);
    // must not panic; dirs may be empty
    let _ = dirs;
}

// ---------- REQ-CFG-03: "..." sentinel merges discovered dirs ---------------

#[test]
fn sentinel_merges_discovered_dirs() {
    let root = tmpdir("cfg03_sentinel");
    fs::create_dir_all(root.join("templates")).unwrap();
    fs::create_dir_all(root.join("custom_tpl")).unwrap();
    fs::write(
        root.join("jinja.toml"),
        r#"templates = ["custom_tpl", "..."]"#,
    )
    .unwrap();

    let cfg = JinjaConfig::discover(&root).unwrap();
    let dirs = cfg.resolved_template_dirs(&root);
    assert!(dirs.iter().any(|d| d.ends_with("custom_tpl")), "custom_tpl must appear");
    assert!(dirs.iter().any(|d| d.ends_with("templates")), "auto-discovered templates/ must appear");
}

#[test]
fn no_sentinel_replaces_defaults() {
    let root = tmpdir("cfg03_no_sentinel");
    fs::create_dir_all(root.join("templates")).unwrap();
    fs::create_dir_all(root.join("custom_tpl")).unwrap();
    fs::write(root.join("jinja.toml"), r#"templates = ["custom_tpl"]"#).unwrap();

    let cfg = JinjaConfig::discover(&root).unwrap();
    let dirs = cfg.resolved_template_dirs(&root);
    assert!(dirs.iter().any(|d| d.ends_with("custom_tpl")));
    assert!(
        !dirs.iter().any(|d| d.ends_with("templates")),
        "templates/ must NOT appear when sentinel is absent"
    );
}

// ---------- REQ-CFG-04: config key set and defaults -------------------------

#[test]
fn defaults_when_all_keys_absent() {
    let root = tmpdir("cfg04_defaults");
    fs::write(root.join("jinja.toml"), "").unwrap();

    let cfg = JinjaConfig::discover(&root).unwrap();
    assert!(cfg.extensions.contains(&"html".to_owned()));
    assert!(cfg.extensions.contains(&"jinja".to_owned()));
    assert!(cfg.extras.is_empty());
    assert!(cfg.custom_builtins.is_empty());
    assert!(cfg.hints.is_empty());
    assert!(cfg.inline_patterns.contains(&"render_template_string".to_owned()));
}

// ---------- REQ-CFG-05: lint filter validation ------------------------------

#[test]
fn lint_select_accepts_full_code() {
    let root = tmpdir("cfg05_code");
    fs::write(root.join("jinja.toml"), "[lint]\nselect = [\"JINJA-E101\"]\n").unwrap();
    let cfg = JinjaConfig::discover(&root).unwrap();
    assert!(cfg.validate().is_ok());
}

#[test]
fn lint_select_accepts_class_prefix() {
    let root = tmpdir("cfg05_prefix");
    fs::write(root.join("jinja.toml"), "[lint]\nselect = [\"JINJA-E\"]\n").unwrap();
    let cfg = JinjaConfig::discover(&root).unwrap();
    assert!(cfg.validate().is_ok());
}

#[test]
fn lint_select_rejects_slug() {
    let root = tmpdir("cfg05_slug");
    fs::write(root.join("jinja.toml"), "[lint]\nselect = [\"unused-variable\"]\n").unwrap();
    let cfg = JinjaConfig::discover(&root).unwrap();
    assert!(cfg.validate().is_err(), "slug in lint.select must be a config error");
}

// ---------- REQ-CFG-07: validation ------------------------------------------

#[test]
fn unknown_extras_name_is_error() {
    let root = tmpdir("cfg07_extras");
    fs::write(root.join("jinja.toml"), r#"extras = ["nonexistent-pack"]"#).unwrap();
    let cfg = JinjaConfig::discover(&root).unwrap();
    assert!(cfg.validate().is_err(), "unknown extras name must be a config error");
}

#[test]
fn overlapping_select_ignore_is_warning() {
    let root = tmpdir("cfg07_overlap");
    fs::write(
        root.join("jinja.toml"),
        "[lint]\nselect = [\"JINJA-E101\"]\nignore = [\"JINJA-E101\"]\n",
    )
    .unwrap();
    let cfg = JinjaConfig::discover(&root).unwrap();
    let result = cfg.validate();
    // must succeed but produce a warning (not an error)
    assert!(result.is_ok(), "overlap should be a warning, not an error: {result:?}");
}

// ---------- REQ-CFG-11: initializationOptions overlay ----------------------

#[test]
fn overlay_overrides_per_key() {
    let root = tmpdir("cfg11_overlay");
    fs::write(root.join("jinja.toml"), r#"extensions = ["html"]"#).unwrap();

    let mut cfg = JinjaConfig::discover(&root).unwrap();
    let overlay = ConfigOverlay {
        extensions: Some(vec!["jinja".to_owned()]),
        extras: None,
        ..Default::default()
    };
    cfg.apply_overlay(overlay);

    assert_eq!(cfg.extensions, vec!["jinja"], "overlay must override extensions");
}

#[test]
fn overlay_absent_key_keeps_file_value() {
    let root = tmpdir("cfg11_keep");
    fs::write(root.join("jinja.toml"), r#"extensions = ["html"]"#).unwrap();

    let mut cfg = JinjaConfig::discover(&root).unwrap();
    let overlay = ConfigOverlay {
        extensions: None, // not overriding extensions
        extras: Some(vec!["starlette".to_owned()]),
        ..Default::default()
    };
    cfg.apply_overlay(overlay);

    assert_eq!(cfg.extensions, vec!["html"], "absent overlay key must keep file value");
    assert_eq!(cfg.extras, vec!["starlette"]);
}

#[test]
fn jinja_lsp_isj4_unrelated_json_deserializes_to_empty_overlay() {
    // jinja-lsp-isj4: ConfigOverlay is all-Option with unknown fields ignored, so a
    // JSON payload with no jinja-lsp-relevant keys at all (e.g. a settings object
    // for a totally different extension, or `{}`) still deserializes successfully —
    // and must be detectable as "empty" so the caller doesn't treat it as a request
    // to clear every setting.
    let json = r#"{"someOtherExtension": {"unrelatedSetting": true}}"#;
    let overlay: ConfigOverlay = serde_json::from_str(json).unwrap();
    assert!(overlay.is_empty(), "an unrelated settings payload must deserialize to an empty overlay");

    let empty: ConfigOverlay = serde_json::from_str("{}").unwrap();
    assert!(empty.is_empty(), "{{}} must deserialize to an empty overlay");
}

#[test]
fn jinja_lsp_isj4_overlay_with_any_field_is_not_empty() {
    let overlay = ConfigOverlay { extensions: Some(vec!["html".to_owned()]), ..Default::default() };
    assert!(!overlay.is_empty(), "an overlay with a real field set must not be empty");
}

#[test]
fn malformed_overlay_does_not_panic() {
    // ConfigOverlay deserialized tolerantly from JSON — partial / unknown fields ok
    let json = r#"{"unknown_field": 42, "extensions": ["html"]}"#;
    let overlay: Result<ConfigOverlay, _> = serde_json::from_str(json);
    // must not panic; unknown fields are ignored
    let _ = overlay;
}

// ---------- REQ-CFG-10: discover_with_path returns config file path ----------

#[test]
fn discover_with_path_returns_jinja_toml_path() {
    let root = tmpdir("cfg10_path_jinja");
    fs::write(root.join("jinja.toml"), r#"extensions = ["html"]"#).unwrap();
    let (cfg, path) = JinjaConfig::discover_with_path(&root).unwrap();
    assert_eq!(cfg.extensions, vec!["html"]);
    assert!(path.is_some(), "must return jinja.toml path");
    assert!(path.unwrap().ends_with("jinja.toml"), "path must point to jinja.toml");
}

#[test]
fn discover_with_path_returns_none_when_no_config_file() {
    let root = tmpdir("cfg10_no_file");
    let (_, path) = JinjaConfig::discover_with_path(&root).unwrap();
    assert!(path.is_none(), "must return None when no config file found");
}

#[test]
fn discover_with_path_returns_pyproject_path() {
    let root = tmpdir("cfg10_path_pyproject");
    fs::write(root.join("pyproject.toml"), "[tool.jinja]\nextensions = [\"jinja\"]\n").unwrap();
    let (cfg, path) = JinjaConfig::discover_with_path(&root).unwrap();
    assert_eq!(cfg.extensions, vec!["jinja"]);
    assert!(path.is_some());
    assert!(path.unwrap().ends_with("pyproject.toml"));
}

// ─── th0l: is_valid_lint_filter must enforce documented grammar ───────────────

#[test]
fn th0l_jinja_bare_prefix_alone_is_invalid() {
    use jinja_lsp::config::ConfigError;
    let root = tmpdir("th0l_bare");
    fs::write(root.join("jinja.toml"), "[lint]\nselect = [\"JINJA-\"]\n").unwrap();
    let (cfg, _) = JinjaConfig::discover_with_path(&root).unwrap();
    let result = cfg.validate();
    assert!(matches!(result, Err(ConfigError::InvalidLintFilter(_))), "JINJA- alone must be invalid");
}

#[test]
fn th0l_jinja_lowercase_suffix_is_invalid() {
    use jinja_lsp::config::ConfigError;
    let root = tmpdir("th0l_lowercase");
    fs::write(root.join("jinja.toml"), "[lint]\nselect = [\"JINJA-zzz\"]\n").unwrap();
    let (cfg, _) = JinjaConfig::discover_with_path(&root).unwrap();
    let result = cfg.validate();
    assert!(matches!(result, Err(ConfigError::InvalidLintFilter(_))), "JINJA-zzz must be invalid");
}

#[test]
fn th0l_jinja_valid_class_prefix_accepted() {
    let root = tmpdir("th0l_class");
    fs::write(root.join("jinja.toml"), "[lint]\nselect = [\"JINJA-E\"]\n").unwrap();
    let (cfg, _) = JinjaConfig::discover_with_path(&root).unwrap();
    assert!(cfg.validate().is_ok(), "JINJA-E must be valid");
}

#[test]
fn th0l_jinja_full_code_accepted() {
    let root = tmpdir("th0l_full");
    fs::write(root.join("jinja.toml"), "[lint]\nselect = [\"JINJA-W203\"]\n").unwrap();
    let (cfg, _) = JinjaConfig::discover_with_path(&root).unwrap();
    assert!(cfg.validate().is_ok(), "JINJA-W203 must be valid");
}

// ─── jinja-lsp-o787: overlay survives config file reload ─────────────────────

#[test]
fn overlay_survives_reload_base_config() {
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(JinjaConfig::default());
    // Apply an overlay (simulating initializationOptions).
    let overlay = ConfigOverlay {
        extensions: Some(vec!["jinja2".to_owned()]),
        ..Default::default()
    };
    state.apply_init_options(overlay).unwrap();
    assert_eq!(state.config.extensions, vec!["jinja2"]);

    // Now reload with a fresh file-based config (extensions = ["html"]).
    let file_config = JinjaConfig { extensions: vec!["html".to_owned()], ..Default::default() };
    state.reload_base_config(file_config);

    // The overlay must win over the file config.
    assert_eq!(state.config.extensions, vec!["jinja2"],
        "overlay must survive reload_base_config");
}

#[test]
fn absent_overlay_key_stays_file_value_after_reload() {
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(JinjaConfig::default());
    // Overlay that only overrides extras (not extensions).
    let overlay = ConfigOverlay {
        extras: Some(vec!["starlette".to_owned()]),
        ..Default::default()
    };
    state.apply_init_options(overlay).unwrap();

    // Reload with a file that changes extensions but NOT extras.
    let file_config = JinjaConfig { extensions: vec!["html".to_owned()], ..Default::default() };
    state.reload_base_config(file_config);

    assert_eq!(state.config.extensions, vec!["html"],
        "file value must win for keys absent in overlay");
    assert_eq!(state.config.extras, vec!["starlette"],
        "overlay value must win for keys present in overlay");
}

// ─── ci5n: from_file loads config from an explicit path ──────────────────────

#[test]
fn ci5n_from_file_loads_explicit_config() {
    let root = tmpdir("ci5n_from_file");
    let cfg_path = root.join("jinja.toml");
    fs::write(&cfg_path, "extensions = [\"jinja\"]\n").unwrap();
    let result = jinja_lsp::config::JinjaConfig::from_file(&cfg_path);
    assert!(result.is_ok(), "from_file must succeed for a valid jinja.toml");
    assert_eq!(result.unwrap().extensions, vec!["jinja"]);
}

#[test]
fn ci5n_from_file_returns_error_for_missing_file() {
    let root = tmpdir("ci5n_missing");
    let result = jinja_lsp::config::JinjaConfig::from_file(&root.join("nonexistent.toml"));
    assert!(result.is_err(), "from_file must return error for a missing file");
}

// ─── ADR-005 / REQ-CFG-10: config_delta diff logic ───────────────────────────

#[test]
fn config_delta_registry_changed_when_extras_differ() {
    let old = JinjaConfig::default();
    let mut new = JinjaConfig::default();
    new.extras = vec!["starlette".to_owned()];
    let (registry_changed, workspace_changed) = jinja_lsp::server::state::config_delta(&old, &new);
    assert!(registry_changed, "extras change must set registry_changed");
    assert!(!workspace_changed, "extras change must not set workspace_changed");
}

#[test]
fn config_delta_registry_changed_when_custom_builtins_differ() {
    let old = JinjaConfig::default();
    let mut new = JinjaConfig::default();
    new.custom_builtins = vec!["/some/dir".to_owned()];
    let (registry_changed, _) = jinja_lsp::server::state::config_delta(&old, &new);
    assert!(registry_changed, "custom_builtins change must set registry_changed");
}

#[test]
fn config_delta_registry_changed_when_hints_differ() {
    let old = JinjaConfig::default();
    let mut new = JinjaConfig::default();
    new.hints = vec!["/hints/dir".to_owned()];
    let (registry_changed, _) = jinja_lsp::server::state::config_delta(&old, &new);
    assert!(registry_changed, "hints change must set registry_changed");
}

#[test]
fn config_delta_workspace_changed_when_templates_differ() {
    let old = JinjaConfig::default();
    let mut new = JinjaConfig::default();
    new.templates_raw = vec!["templates".to_owned()];
    let (registry_changed, workspace_changed) = jinja_lsp::server::state::config_delta(&old, &new);
    assert!(workspace_changed, "templates change must set workspace_changed");
    assert!(!registry_changed, "templates change must not set registry_changed");
}

#[test]
fn config_delta_workspace_changed_when_extensions_differ() {
    let old = JinjaConfig::default();
    let mut new = JinjaConfig::default();
    new.extensions = vec!["html".to_owned(), "jinja".to_owned()];
    let (_, workspace_changed) = jinja_lsp::server::state::config_delta(&old, &new);
    assert!(workspace_changed, "extensions change must set workspace_changed");
}

#[test]
fn config_delta_no_change_when_only_lint_differs() {
    use jinja_lsp::config::LintConfig;
    let old = JinjaConfig::default();
    let mut new = JinjaConfig::default();
    new.lint = LintConfig { select: vec!["JINJA-E101".to_owned()], ignore: vec![] };
    let (registry_changed, workspace_changed) = jinja_lsp::server::state::config_delta(&old, &new);
    assert!(!registry_changed, "lint-only change must not set registry_changed");
    assert!(!workspace_changed, "lint-only change must not set workspace_changed");
}

#[test]
fn reload_config_selective_skips_registry_rebuild_for_lint_only_change() {
    use jinja_lsp::config::LintConfig;
    use jinja_lsp::server::state::ServerState;
    use jinja_lsp::builtins::registry::{Category, Source, DocEntry};

    let mut state = ServerState::with_config(JinjaConfig::default());
    // Insert a sentinel entry that a real registry rebuild would NOT contain.
    let sentinel = DocEntry {
        name: "my_sentinel".to_owned(),
        category: Category::Filter,
        signature: None,
        since: None,
        params: vec![],
        body: "sentinel".to_owned(),
        source: Source::Custom,
        ty: None,
        template: None,
    };
    state.registry.insert(sentinel);
    assert!(state.registry.get(Category::Filter, "my_sentinel").is_some(), "sentinel must be inserted");

    // Reload with a config that only changes lint (not extras/custom_builtins/hints).
    let mut new_cfg = JinjaConfig::default();
    new_cfg.lint = LintConfig { select: vec!["JINJA-E101".to_owned()], ignore: vec![] };
    let (registry_rebuilt, _) = state.reload_config_selective(new_cfg);

    assert!(!registry_rebuilt, "registry must NOT be rebuilt for lint-only change");
    assert!(
        state.registry.get(Category::Filter, "my_sentinel").is_some(),
        "sentinel must survive a lint-only reload (registry not rebuilt)"
    );
    assert_eq!(state.config.lint.select, vec!["JINJA-E101"], "lint must be updated");
}

#[test]
fn reload_config_selective_rebuilds_registry_when_extras_change() {
    use jinja_lsp::server::state::ServerState;
    use jinja_lsp::builtins::registry::{Category, Source, DocEntry};

    let mut state = ServerState::with_config(JinjaConfig::default());
    let sentinel = DocEntry {
        name: "my_sentinel_extras".to_owned(),
        category: Category::Filter,
        signature: None,
        since: None,
        params: vec![],
        body: "sentinel".to_owned(),
        source: Source::Custom,
        ty: None,
        template: None,
    };
    state.registry.insert(sentinel);

    let mut new_cfg = JinjaConfig::default();
    new_cfg.extras = vec!["starlette".to_owned()];
    let (registry_rebuilt, _) = state.reload_config_selective(new_cfg);

    assert!(registry_rebuilt, "registry must be rebuilt when extras change");
    // Sentinel is gone since registry was rebuilt from scratch.
    assert!(
        state.registry.get(Category::Filter, "my_sentinel_extras").is_none(),
        "sentinel must be gone after registry rebuild"
    );
    assert_eq!(state.config.extras, vec!["starlette"], "extras must be updated");
}

// ─── [format] section ────────────────────────────────────────────────────────

#[test]
fn config_format_section_defaults_when_absent() {
    let cfg = JinjaConfig::default();
    assert_eq!(cfg.format, FormatterConfig::default());
    assert_eq!(cfg.format.indent_size, 4);
    assert!(!cfg.format.space_around_pipe);
    assert!(cfg.format.space_after_comma);
    assert!(cfg.format.newline_at_eof);
    assert!(cfg.format.trim_trailing_whitespace);
}

#[test]
fn config_format_section_parsed_from_toml() {
    let toml = r#"
[format]
indent_size = 2
space_around_pipe = true
space_after_comma = false
newline_at_eof = false
"#;
    let dir = tmpdir("format_section");
    let cfg_path = dir.join("jinja.toml");
    fs::write(&cfg_path, toml).unwrap();
    let cfg = JinjaConfig::from_file(&cfg_path).unwrap();
    assert_eq!(cfg.format.indent_size, 2);
    assert!(cfg.format.space_around_pipe);
    assert!(!cfg.format.space_after_comma);
    assert!(!cfg.format.newline_at_eof);
    assert!(cfg.format.trim_trailing_whitespace, "unset keys use default");
}
