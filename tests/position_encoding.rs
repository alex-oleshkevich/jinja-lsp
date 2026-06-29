// F07 / jinja-lsp-7b7s: position encoding negotiation and UTF-16 ↔ byte conversion.

// ─── Wiring contract ─────────────────────────────────────────────────────────

#[test]
fn server_negotiates_utf8_position_encoding() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("PositionEncodingKind"),
        "server mod must negotiate UTF-8 position encoding in initialize()"
    );
}

// ─── Conversion unit tests ────────────────────────────────────────────────────

use jinja_lsp::server::lsp_char_to_byte_col;
use jinja_lsp::server::byte_col_to_lsp_char;

#[test]
fn utf16_to_byte_ascii_no_op() {
    // ASCII: UTF-16 code units == bytes == UTF-8 code units.
    assert_eq!(lsp_char_to_byte_col("hello world", 5, false), 5);
}

#[test]
fn utf16_to_byte_multibyte_accent() {
    // "café" — é is U+00E9: 2 bytes in UTF-8, 1 UTF-16 code unit.
    // UTF-16 col 4 (past "café") should map to byte col 5.
    let line = "café";
    assert_eq!(lsp_char_to_byte_col(line, 4, false), 5);
}

#[test]
fn utf16_to_byte_emoji() {
    // "😀" is U+1F600: 4 bytes in UTF-8, 2 UTF-16 surrogate-pair code units.
    // UTF-16 col 2 (past the emoji) should map to byte col 4.
    let line = "😀x";
    assert_eq!(lsp_char_to_byte_col(line, 2, false), 4);
}

#[test]
fn byte_to_utf16_ascii_no_op() {
    assert_eq!(byte_col_to_lsp_char("hello world", 5, false), 5);
}

#[test]
fn byte_to_utf16_multibyte_accent() {
    // byte col 5 (past "café") → UTF-16 col 4.
    let line = "café";
    assert_eq!(byte_col_to_lsp_char(line, 5, false), 4);
}

#[test]
fn byte_to_utf16_emoji() {
    // byte col 4 (past "😀") → UTF-16 col 2 (surrogate pair).
    let line = "😀x";
    assert_eq!(byte_col_to_lsp_char(line, 4, false), 2);
}

#[test]
fn utf8_mode_is_identity() {
    // In UTF-8 mode, both functions return the input unchanged.
    let line = "café";
    assert_eq!(lsp_char_to_byte_col(line, 5, true), 5);
    assert_eq!(byte_col_to_lsp_char(line, 5, true), 5);
}

#[test]
fn roundtrip_byte_utf16_byte() {
    let line = "{{ äü | upper }}";
    // Byte 10 = start of "upper" ({{ = 2, space=1, ä=2, ü=2, space+|+space = 3)
    let utf16 = byte_col_to_lsp_char(line, 10, false);
    let byte = lsp_char_to_byte_col(line, utf16, false);
    assert_eq!(byte, 10, "roundtrip must be identity");
}
