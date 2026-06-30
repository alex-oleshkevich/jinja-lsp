// Config system tests: REQ-CFG-01 through REQ-CFG-11.

use std::fs;

use jinja_lsp::config::{ConfigOverlay, JinjaConfig};

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
