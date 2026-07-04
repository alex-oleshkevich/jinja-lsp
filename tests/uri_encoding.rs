// jinja-lsp-0kuj: URIs must be decoded/encoded via Url::to_file_path /
// Url::from_file_path, not raw uri.path() / hand-rolled "file://{path}"
// string formatting — otherwise paths with spaces, '#', '?', or non-ASCII
// characters silently desync from the real filesystem path.
//
// Url::from_file_path/to_file_path require a host-OS-absolute path (on
// Windows that means a drive letter); a bare "/my dir/t.html" isn't a valid
// Windows path, so every case below builds its path/URI via a helper that's
// absolute on whichever OS the test runs on.

use jinja_lsp::server::{Backend, path_to_uri};
use tower_lsp::lsp_types::Url;

#[cfg(windows)]
fn dir(rest: &str) -> String {
    format!(r"C:\{rest}")
}

#[cfg(not(windows))]
fn dir(rest: &str) -> String {
    format!("/{rest}")
}

#[cfg(windows)]
fn uri_path(rest: &str) -> String {
    format!("file:///C:/{rest}")
}

#[cfg(not(windows))]
fn uri_path(rest: &str) -> String {
    format!("file:///{rest}")
}

#[test]
fn uri_to_key_decodes_percent_encoded_spaces() {
    let uri = Url::parse(&uri_path("my%20dir/t.html")).unwrap();
    assert_eq!(Backend::uri_to_key(&uri), dir("my dir/t.html"));
}

#[test]
fn uri_to_key_decodes_non_ascii() {
    let uri = Url::parse(&uri_path("caf%C3%A9/t.html")).unwrap();
    assert_eq!(Backend::uri_to_key(&uri), dir("café/t.html"));
}

#[test]
fn path_to_uri_encodes_spaces() {
    let uri = path_to_uri(&dir("my dir/t.html"));
    assert_eq!(uri.as_str(), uri_path("my%20dir/t.html"));
}

#[test]
fn path_to_uri_encodes_hash_and_question_mark() {
    // '#' and '?' have special meaning in a URI; a naive "file://{path}"
    // format would truncate the path or misparse it as a query/fragment.
    let uri = path_to_uri(&dir("weird#name?.html"));
    let decoded = uri.to_file_path().expect("must round-trip to a file path");
    assert_eq!(decoded.to_string_lossy(), dir("weird#name?.html"));
}

#[test]
fn roundtrip_path_with_spaces_through_uri_and_back() {
    let original = dir("my dir/t.html");
    let uri = path_to_uri(&original);
    let key = Backend::uri_to_key(&uri);
    assert_eq!(key, original, "path -> uri -> key must round-trip exactly");
}
