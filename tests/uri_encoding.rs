// jinja-lsp-0kuj: URIs must be decoded/encoded via Url::to_file_path /
// Url::from_file_path, not raw uri.path() / hand-rolled "file://{path}"
// string formatting — otherwise paths with spaces, '#', '?', or non-ASCII
// characters silently desync from the real filesystem path.

use jinja_lsp::server::{path_to_uri, Backend};
use tower_lsp::lsp_types::Url;

#[test]
fn uri_to_key_decodes_percent_encoded_spaces() {
    let uri = Url::parse("file:///my%20dir/t.html").unwrap();
    assert_eq!(Backend::uri_to_key(&uri), "/my dir/t.html");
}

#[test]
fn uri_to_key_decodes_non_ascii() {
    let uri = Url::parse("file:///caf%C3%A9/t.html").unwrap();
    assert_eq!(Backend::uri_to_key(&uri), "/café/t.html");
}

#[test]
fn path_to_uri_encodes_spaces() {
    let uri = path_to_uri("/my dir/t.html");
    assert_eq!(uri.as_str(), "file:///my%20dir/t.html");
}

#[test]
fn path_to_uri_encodes_hash_and_question_mark() {
    // '#' and '?' have special meaning in a URI; a naive "file://{path}"
    // format would truncate the path or misparse it as a query/fragment.
    let uri = path_to_uri("/weird#name?.html");
    let decoded = uri.to_file_path().expect("must round-trip to a file path");
    assert_eq!(decoded.to_string_lossy(), "/weird#name?.html");
}

#[test]
fn roundtrip_path_with_spaces_through_uri_and_back() {
    let original = "/my dir/t.html";
    let uri = path_to_uri(original);
    let key = Backend::uri_to_key(&uri);
    assert_eq!(key, original, "path -> uri -> key must round-trip exactly");
}
