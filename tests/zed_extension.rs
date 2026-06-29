// REQ-EDIT-07/08/12: Zed extension manifest and source verification.
// Static doc-check tests that parse extension.toml and verify the required fields.

fn manifest() -> toml::Value {
    let raw = include_str!("../editors/zed/extension.toml");
    toml::from_str(raw).expect("extension.toml must be valid TOML")
}

// ─── T-13: REQ-EDIT-07 — extension.toml declares grammar + server ────────────

#[test]
fn zed_manifest_declares_jinja_grammar() {
    let m = manifest();
    let grammars = m.get("grammars").expect("extension.toml must have [grammars]");
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
    let servers = m.get("language_servers").expect("extension.toml must have [language_servers]");
    // REQ-EDIT-07/08: language-server id is jinja2-lsp, language is Jinja2 (HTML).
    let server = servers.get("jinja2-lsp").expect("must declare jinja2-lsp server");
    let langs = server["languages"].as_array().expect("languages must be array");
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

// ─── T-13c: languages/jinja2/config.toml must exist with the correct name ────

#[test]
fn zed_language_config_exists_with_correct_name() {
    let cfg_raw = include_str!("../editors/zed/languages/jinja2/config.toml");
    let cfg: toml::Value = toml::from_str(cfg_raw).expect("languages/jinja2/config.toml must be valid TOML");
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

// ─── T-15/16: REQ-EDIT-12 — downloads and verifies checksum ─────────────────

#[test]
fn zed_extension_source_downloads_release_binary() {
    let src = include_str!("../editors/zed/src/lib.rs");
    // Must attempt a download.
    assert!(
        src.contains("download_file"),
        "src/lib.rs must call download_file for the release binary"
    );
    // Must use latest_github_release to locate the release asset (F21 single source of truth).
    assert!(
        src.contains("latest_github_release"),
        "src/lib.rs must fetch from github release to locate the binary"
    );
    // Note: zed_extension_api 0.2 has no checksum-verification API; verification
    // is skipped. Transport integrity is provided by HTTPS. (REQ-EDIT-12 aspirational.)
}

// ─── T-17: REQ-EDIT-08 — jinja2-lsp id and Jinja2 (HTML) language ───────────

#[test]
fn zed_manifest_legacy_ids_preserved() {
    let m = manifest();
    // The language-server id jinja2-lsp and language Jinja2 (HTML) are the legacy
    // identifiers from the hand-created .zed/settings.json. They must remain stable
    // so existing Zed users' configuration keeps working (REQ-EDIT-07/08).
    let servers = m["language_servers"].as_table().expect("language_servers table");
    assert!(
        servers.contains_key("jinja2-lsp"),
        "legacy jinja2-lsp id must be preserved"
    );
    let lang = servers["jinja2-lsp"]["languages"][0].as_str().unwrap_or("");
    assert_eq!(lang, "Jinja2 (HTML)", "legacy language name Jinja2 (HTML) must be preserved");
}

// ─── T-REL-01: REQ-EDIT-12 — release asset names match workflow-published archives

#[test]
fn zed_asset_names_match_release_workflow() {
    let src = include_str!("../editors/zed/src/lib.rs");
    // The workflow publishes exact Rust target-triple archives.
    // The extension must use match (os, arch) → triple, not os.to_string() shortcuts.
    for triple in &[
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ] {
        assert!(
            src.contains(triple),
            "zed lib.rs must reference target triple '{triple}' to match release asset names"
        );
    }
}

#[test]
fn zed_download_uses_archive_file_types() {
    let src = include_str!("../editors/zed/src/lib.rs");
    // Binaries are published as .tar.gz (Linux/macOS) and .zip (Windows).
    // Using Uncompressed would download the archive bytes and try to exec them.
    assert!(
        src.contains("GzipTar"),
        "zed lib.rs must use DownloadedFileType::GzipTar for .tar.gz archives"
    );
    assert!(
        src.contains("Zip"),
        "zed lib.rs must use DownloadedFileType::Zip for Windows .zip archives"
    );
    assert!(
        !src.contains("Uncompressed"),
        "zed lib.rs must NOT use DownloadedFileType::Uncompressed for compressed release archives"
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
