// REQ-EDIT-03/04/05/06/11: VS Code extension manifest and structure verification.
// These are static doc-check tests — they parse package.json and verify the
// fields required by the spec without running the actual extension.

fn pkg() -> serde_json::Value {
    let raw = include_str!("../editors/vscode/package.json");
    serde_json::from_str(raw).expect("package.json must be valid JSON")
}

// ─── T-07: REQ-EDIT-04 — Activation events ───────────────────────────────────

#[test]
fn vscode_activation_events_include_jinja_languages_and_config() {
    let pkg = pkg();
    let events: Vec<&str> = pkg["activationEvents"]
        .as_array()
        .expect("activationEvents must be array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();

    assert!(events.iter().any(|&e| e == "onLanguage:jinja"), "must activate on jinja");
    assert!(events.iter().any(|&e| e == "onLanguage:jinja-html"), "must activate on jinja-html");
    assert!(
        events.iter().any(|&e| e == "workspaceContains:jinja.toml"),
        "must activate when workspace has jinja.toml"
    );
}

// ─── T-08: REQ-EDIT-05 — Settings map one-to-one to jinja.toml keys ─────────

#[test]
fn vscode_settings_map_required_keys() {
    let pkg = pkg();
    let props = &pkg["contributes"]["configuration"]["properties"];

    // All 7 config keys must be present (server.path is client-only but still required).
    for key in &[
        "jinja-lsp.server.path",
        "jinja-lsp.templates",
        "jinja-lsp.extensions",
        "jinja-lsp.extras",
        "jinja-lsp.customBuiltins",
        "jinja-lsp.hints",
        "jinja-lsp.lint.select",
        "jinja-lsp.lint.ignore",
    ] {
        assert!(
            props.get(key).is_some(),
            "package.json must define setting: {key}"
        );
    }
}

// ─── T-12: REQ-EDIT-06 — tmLanguage files registered for jinja/jinja-html ───

#[test]
fn vscode_grammars_registered_for_both_languages() {
    let pkg = pkg();
    let grammars = pkg["contributes"]["grammars"]
        .as_array()
        .expect("grammars must be array");

    let languages: Vec<&str> = grammars
        .iter()
        .filter_map(|g| g["language"].as_str())
        .collect();

    assert!(languages.contains(&"jinja"), "tmLanguage must cover jinja language");
    assert!(languages.contains(&"jinja-html"), "tmLanguage must cover jinja-html language");
}

// ─── T-12b: tmLanguage files exist on disk ───────────────────────────────────

#[test]
fn vscode_tmlanguage_files_exist() {
    let _ = include_str!("../editors/vscode/syntaxes/jinja.tmLanguage.json");
    let _ = include_str!("../editors/vscode/syntaxes/jinja-html.tmLanguage.json");
}

// ─── T-25: REQ-EDIT-11 — languageId contributions are canonical ──────────────

#[test]
fn vscode_language_ids_are_canonical() {
    let pkg = pkg();
    let languages = pkg["contributes"]["languages"]
        .as_array()
        .expect("languages must be array");

    let ids: Vec<&str> = languages
        .iter()
        .filter_map(|l| l["id"].as_str())
        .collect();

    // The two canonical languageIds per REQ-EDIT-11.
    assert!(ids.contains(&"jinja"), "VS Code must contribute 'jinja' language");
    assert!(ids.contains(&"jinja-html"), "VS Code must contribute 'jinja-html' language");
}

// ─── jinja-lsp-ltdq: jinja-html must not steal the default .html association ─

#[test]
fn jinja_html_does_not_claim_bare_html_extension() {
    // Registering plain ".html" makes jinja-html the default language for
    // every HTML file in every workspace, stealing VS Code's built-in HTML
    // IntelliSense/Emmet/formatting even in projects with no Jinja at all.
    let pkg = pkg();
    let languages = pkg["contributes"]["languages"].as_array().expect("languages must be array");
    let jinja_html = languages
        .iter()
        .find(|l| l["id"].as_str() == Some("jinja-html"))
        .expect("jinja-html language entry must exist");
    let extensions: Vec<&str> = jinja_html["extensions"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    assert!(
        !extensions.contains(&".html"),
        "jinja-html must not register bare '.html' as a default extension; got: {extensions:?}"
    );
}

// ─── T-01: REQ-EDIT-01 — extension.ts uses jinja-lsp lsp command ─────────────

#[test]
fn vscode_extension_ts_launches_jinja_lsp_lsp() {
    let src = include_str!("../editors/vscode/src/extension.ts");
    assert!(
        src.contains("'jinja-lsp'") || src.contains("\"jinja-lsp\""),
        "extension.ts must reference the jinja-lsp binary"
    );
    assert!(
        src.contains("'lsp'") || src.contains("\"lsp\""),
        "extension.ts must launch with the lsp subcommand"
    );
    assert!(src.contains("stdio"), "extension.ts must use stdio transport");
}
