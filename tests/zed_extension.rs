// REQ-EDIT-07/08/12: Zed extension manifest and source verification.
// Static doc-check tests that parse extension.toml and verify the required fields.

use jinja_lsp::server::is_jinja_language_id;

fn manifest() -> toml::Value {
    let raw = include_str!("../editors/zed/extension.toml");
    toml::from_str(raw).expect("extension.toml must be valid TOML")
}

// ─── T-13: REQ-EDIT-07 — extension.toml declares grammar + server ────────────

#[test]
fn zed_manifest_declares_jinja_grammar() {
    let m = manifest();
    let grammars = m
        .get("grammars")
        .expect("extension.toml must have [grammars]");
    assert!(
        grammars.get("jinja").is_some(),
        "extension.toml must declare the jinja grammar"
    );
    // REQ-EDIT-07: points at upstream alex-oleshkevich/tree-sitter-jinja (ADR-002).
    let repo = grammars["jinja"]["repository"].as_str().unwrap_or("");
    assert!(
        repo.contains("tree-sitter-jinja"),
        "grammar must point at tree-sitter-jinja repo; got: {repo}"
    );
}

#[test]
fn zed_manifest_declares_language_server() {
    let m = manifest();
    let servers = m
        .get("language_servers")
        .expect("extension.toml must have [language_servers]");
    // REQ-EDIT-07/08: language-server id is jinja2-lsp, language is Jinja2 (HTML).
    let server = servers
        .get("jinja2-lsp")
        .expect("must declare jinja2-lsp server");
    let langs = server["languages"]
        .as_array()
        .expect("languages must be array");
    let lang_names: Vec<&str> = langs.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        lang_names.contains(&"Jinja2 (HTML)"),
        "jinja2-lsp must serve Jinja2 (HTML); got: {lang_names:?}"
    );
}

#[test]
fn zed_manifest_is_cdylib_crate() {
    let cargo_raw = include_str!("../editors/zed/Cargo.toml");
    let cargo: toml::Value = toml::from_str(cargo_raw).expect("Cargo.toml must be valid TOML");
    let crate_types = cargo["lib"]["crate-type"]
        .as_array()
        .expect("lib.crate-type must be array");
    let types: Vec<&str> = crate_types.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        types.contains(&"cdylib"),
        "Zed extension crate must be cdylib; got: {types:?}"
    );
}

// ─── T-13b: grammar must be pinned to an immutable SHA, not HEAD ─────────────

#[test]
fn zed_grammar_pinned_to_sha_not_head() {
    let m = manifest();
    let commit = m["grammars"]["jinja"]["commit"].as_str().unwrap_or("HEAD");
    assert_ne!(
        commit, "HEAD",
        "grammar commit must be a full SHA, not 'HEAD' — non-reproducible builds break users"
    );
    assert!(
        commit.len() >= 40,
        "grammar commit must be a full 40-char SHA; got: {commit}"
    );
}

// ─── T-13c: languages/jinja2-html/config.toml must exist with the correct name ─

#[test]
fn zed_language_config_exists_with_correct_name() {
    let cfg_raw = include_str!("../editors/zed/languages/jinja2-html/config.toml");
    let cfg: toml::Value =
        toml::from_str(cfg_raw).expect("languages/jinja2-html/config.toml must be valid TOML");
    let name = cfg["name"].as_str().unwrap_or("");
    assert_eq!(
        name, "Jinja2 (HTML)",
        "language config name must match the language_servers entry; got: {name}"
    );
    let grammar = cfg["grammar"].as_str().unwrap_or("");
    assert_eq!(
        grammar, "jinja",
        "language config grammar must be 'jinja'; got: {grammar}"
    );
}

// ─── jinja-lsp-5ydn: base Jinja2 language for bare .j2/.jinja/.jinja2 files ──

#[test]
fn zed_base_language_config_exists_with_correct_name() {
    let cfg_raw = include_str!("../editors/zed/languages/jinja2/config.toml");
    let cfg: toml::Value =
        toml::from_str(cfg_raw).expect("languages/jinja2/config.toml must be valid TOML");
    let name = cfg["name"].as_str().unwrap_or("");
    assert_eq!(name, "Jinja2", "base language config name; got: {name}");
    let grammar = cfg["grammar"].as_str().unwrap_or("");
    assert_eq!(
        grammar, "jinja",
        "base language config grammar must be 'jinja'; got: {grammar}"
    );
}

// jinja-lsp-5ydn: bare .j2/.jinja/.jinja2 files must keep the LSP after the
// jinja2 -> jinja2-html restructure (this was a flagged regression risk).
#[test]
fn zed_base_language_keeps_bare_suffixes_and_lsp() {
    let cfg_raw = include_str!("../editors/zed/languages/jinja2/config.toml");
    let cfg: toml::Value = toml::from_str(cfg_raw).expect("config.toml must be valid TOML");
    let suffixes: Vec<&str> = cfg["path_suffixes"]
        .as_array()
        .expect("path_suffixes must be array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for bare in ["j2", "jinja", "jinja2"] {
        assert!(
            suffixes.contains(&bare),
            "base Jinja2 must claim bare suffix {bare:?}; got: {suffixes:?}"
        );
    }

    let m = manifest();
    let langs: Vec<&str> = m["language_servers"]["jinja2-lsp"]["languages"]
        .as_array()
        .expect("languages must be array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        langs.contains(&"Jinja2"),
        "jinja2-lsp must also serve the base Jinja2 language, or bare .j2 files lose the LSP; got: {langs:?}"
    );
}

// jinja-lsp-ioce: every language_ids value declared in extension.toml must be
// accepted by is_jinja_language_id(), or Zed will send a languageId the server
// silently rejects on every didOpen for that language.
#[test]
fn zed_every_declared_language_id_is_accepted_by_server() {
    let m = manifest();
    let language_ids = m["language_servers"]["jinja2-lsp"]["language_ids"]
        .as_table()
        .expect("language_ids must be a table");
    assert!(!language_ids.is_empty(), "language_ids must not be empty");
    for (name, id) in language_ids {
        let id = id.as_str().expect("language_ids values must be strings");
        assert!(
            is_jinja_language_id(id),
            "language_ids[{name:?}] = {id:?} is not accepted by is_jinja_language_id()"
        );
    }

    // Every language declared for jinja2-lsp must also have a language_ids
    // entry -- otherwise it falls back to Zed's name.to_lowercase() default,
    // which the server rejects (jinja-lsp-fxse).
    let langs = m["language_servers"]["jinja2-lsp"]["languages"]
        .as_array()
        .expect("languages must be array");
    for lang in langs.iter().filter_map(|v| v.as_str()) {
        assert!(
            language_ids.contains_key(lang),
            "language {lang:?} is declared for jinja2-lsp but has no language_ids mapping"
        );
    }
}

// jinja-lsp-ioce: scan every languages/*/config.toml at test time (not
// include_str!, so this keeps working as more language dirs are batch-added
// in jinja-lsp-ao83 without editing this test) and assert no two directories
// claim the same path_suffix.
#[test]
fn zed_no_language_path_suffix_overlap() {
    let languages_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("editors/zed/languages");
    let mut claims: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&languages_dir).expect("languages dir must exist") {
        let dir = entry.expect("readable dir entry").path();
        if !dir.is_dir() {
            continue;
        }
        let cfg_path = dir.join("config.toml");
        let raw = std::fs::read_to_string(&cfg_path)
            .unwrap_or_else(|e| panic!("{}: {e}", cfg_path.display()));
        let cfg: toml::Value = toml::from_str(&raw)
            .unwrap_or_else(|e| panic!("{}: invalid TOML: {e}", cfg_path.display()));
        let dir_name = dir.file_name().unwrap().to_string_lossy().into_owned();
        for suffix in cfg["path_suffixes"]
            .as_array()
            .unwrap_or_else(|| panic!("{}: path_suffixes must be array", cfg_path.display()))
            .iter()
            .filter_map(|v| v.as_str())
        {
            claims.push((suffix.to_owned(), dir_name.clone()));
        }
    }
    for (suffix, dir) in &claims {
        let owners: Vec<&str> = claims
            .iter()
            .filter(|(s, _)| s == suffix)
            .map(|(_, d)| d.as_str())
            .collect();
        assert!(
            owners.len() == 1,
            "path_suffix {suffix:?} claimed by more than one language dir: {owners:?} (via {dir})"
        );
    }
}

// ─── jinja-lsp-pxs5: {# #} is a block comment, not a line-comment prefix ─────

fn assert_block_comment_not_line_comments(cfg_raw: &str, label: &str) {
    let cfg: toml::Value = toml::from_str(cfg_raw).expect("config.toml must be valid TOML");

    // line_comments treats each string as an independent line-comment prefix,
    // so toggling a comment would prepend "{# " without ever closing it.
    assert!(
        cfg.get("line_comments").is_none(),
        "line_comments must not be set for Jinja ({label}) — {{# #}} is a block comment, not a line prefix"
    );

    let block_comment = cfg["block_comment"]
        .as_array()
        .unwrap_or_else(|| panic!("block_comment must be set as a [start, end] pair ({label})"));
    let pair: Vec<&str> = block_comment
        .iter()
        .map(|v| v.as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        pair,
        vec!["{# ", " #}"],
        "block_comment must be [\"{{# \", \" #}}\"] ({label})"
    );
}

#[test]
fn zed_language_config_uses_block_comment_not_line_comments() {
    assert_block_comment_not_line_comments(
        include_str!("../editors/zed/languages/jinja2/config.toml"),
        "jinja2",
    );
    assert_block_comment_not_line_comments(
        include_str!("../editors/zed/languages/jinja2-html/config.toml"),
        "jinja2-html",
    );
}

// ─── T-14: REQ-EDIT-07 — language_server_command returns jinja-lsp lsp ───────

#[test]
fn zed_extension_source_returns_lsp_command() {
    let src = include_str!("../editors/zed/src/lib.rs");
    assert!(
        src.contains("\"jinja-lsp\"") || src.contains("'jinja-lsp'"),
        "src/lib.rs must reference jinja-lsp binary"
    );
    assert!(
        src.contains("\"lsp\""),
        "language_server_command must use 'lsp' subcommand"
    );
    assert!(
        src.contains("language_server_command"),
        "src/lib.rs must implement language_server_command"
    );
}

// ─── T-17: REQ-EDIT-08 — jinja2-lsp id and Jinja2 (HTML) language ───────────

#[test]
fn zed_manifest_legacy_ids_preserved() {
    let m = manifest();
    // The language-server id jinja2-lsp and language Jinja2 (HTML) are the legacy
    // identifiers from the hand-created .zed/settings.json. They must remain stable
    // so existing Zed users' configuration keeps working (REQ-EDIT-07/08).
    let servers = m["language_servers"]
        .as_table()
        .expect("language_servers table");
    assert!(
        servers.contains_key("jinja2-lsp"),
        "legacy jinja2-lsp id must be preserved"
    );
    let lang = servers["jinja2-lsp"]["languages"][0].as_str().unwrap_or("");
    assert_eq!(
        lang, "Jinja2 (HTML)",
        "legacy language name Jinja2 (HTML) must be preserved"
    );
}

#[test]
fn zed_no_nonexistent_api_calls() {
    let src = include_str!("../editors/zed/src/lib.rs");
    // verify_file_against_checksum does not exist in zed_extension_api 0.2.
    assert!(
        !src.contains("verify_file_against_checksum"),
        "zed lib.rs must not call verify_file_against_checksum (not in zed_extension_api 0.2)"
    );
}

#[test]
fn zed_binary_sha256_published_by_release_workflow() {
    let src = include_str!("../.github/workflows/release.yml");
    // REQ-EDIT-12: release.yml must compute and upload the extracted-binary hash
    // so the Zed extension can verify it. Archive hash alone is insufficient because
    // download_file extracts the archive before the extension can access its bytes.
    assert!(
        src.contains("binary.sha256"),
        "release.yml must publish a .binary.sha256 asset containing the extracted-binary hash"
    );
}

#[test]
fn jinja_lsp_r52g_package_script_does_not_mutate_source_tree_and_resolves_repo_root() {
    let src = include_str!("../scripts/package-zed-extension.sh");
    // The script used to `cp LICENSE "$ZED_SRC/"`, leaving an untracked
    // editors/zed/LICENSE behind — redundant since LICENSE is already copied
    // straight into the packaging stage dir.
    assert!(
        !src.contains(r#"cp LICENSE "$ZED_SRC/""#),
        "package-zed-extension.sh must not copy LICENSE into the editors/zed source tree"
    );
    // It must resolve paths from the script's own location (like install-zed-extension.sh's
    // REPO_ROOT convention) instead of assuming the repo root as cwd.
    assert!(
        src.contains("REPO_ROOT="),
        "package-zed-extension.sh must resolve REPO_ROOT from the script location, not cwd"
    );
    assert!(
        src.contains(r#"ZED_SRC="$REPO_ROOT/editors/zed""#),
        "ZED_SRC must be resolved relative to REPO_ROOT"
    );
    assert!(
        src.contains("--manifest-path \"$REPO_ROOT/Cargo.toml\""),
        "cargo metadata must use an explicit --manifest-path so VERSION resolves correctly regardless of cwd"
    );
}

// ─── jinja-lsp-ioce: validation batch (YAML, JSON, Markdown) ─────────────────

fn assert_embed_language_config(dir: &str, expected_name: &str, expected_injection_lang: &str) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("editors/zed/languages");
    let cfg_raw = std::fs::read_to_string(base.join(dir).join("config.toml"))
        .unwrap_or_else(|e| panic!("{dir}/config.toml: {e}"));
    let cfg: toml::Value = toml::from_str(&cfg_raw).expect("valid TOML");
    assert_eq!(cfg["name"].as_str().unwrap_or(""), expected_name);
    assert_eq!(cfg["grammar"].as_str().unwrap_or(""), "jinja");

    let inj_raw = std::fs::read_to_string(base.join(dir).join("injections.scm"))
        .unwrap_or_else(|e| panic!("{dir}/injections.scm: {e}"));
    // The legacy extension's grammar called this node `text`; ours calls it
    // `content` (verified against the pinned tree-sitter-jinja commit in
    // jinja-lsp-fnj6) -- a stray `(text)` here would silently match nothing.
    assert!(
        inj_raw.contains("(content)"),
        "{dir}/injections.scm must inject via (content), not the legacy grammar's (text)"
    );
    assert!(
        !inj_raw.contains("(text)"),
        "{dir}/injections.scm must not use the legacy grammar's (text) node"
    );
    assert!(
        inj_raw.contains(&format!("\"{expected_injection_lang}\"")),
        "{dir}/injections.scm must inject language {expected_injection_lang:?}"
    );

    // scope_opt_in_language_servers is a syntax-scope gate (only takes effect
    // inside an overrides.scm-defined scope that opts in) -- it is NOT a user
    // opt-in switch, and this extension ships no overrides.scm, so it would be
    // permanently-dead config if copied verbatim from the legacy extension.
    assert!(
        !cfg_raw.contains("scope_opt_in_language_servers"),
        "{dir}/config.toml must not carry dead scope_opt_in_language_servers config"
    );
    assert!(
        !cfg_raw.contains("vscode-jinja"),
        "{dir}/config.toml must not reference the nonexistent vscode-jinja server"
    );
}

#[test]
fn zed_jinja2_yaml_config_is_correct() {
    assert_embed_language_config("jinja2-yaml", "Jinja2 (YAML)", "yaml");
}

#[test]
fn zed_jinja2_json_config_is_correct() {
    assert_embed_language_config("jinja2-json", "Jinja2 (JSON)", "json");
}

#[test]
fn zed_jinja2_markdown_config_is_correct() {
    assert_embed_language_config("jinja2-markdown", "Jinja2 (Markdown)", "markdown");
}
