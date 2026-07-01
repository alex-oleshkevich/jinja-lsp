// REQ-INLN-03: inline range record — translates inline-relative positions to host-file coords.

/// Maps an inline template's origin in its host file so positions can be expressed
/// in host-file coordinates (REQ-INLN-03).  Created in `state::index_file_into` and
/// stored in `WorkspaceIndex::inline_ranges` keyed by the inline template key.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineRange {
    /// Absolute path of the host file this region was extracted from.
    pub host_path: String,
    /// Byte offset of the first byte of the inline content in the host file.
    pub host_offset: usize,
    /// 0-indexed line in the host file where the inline content begins.
    pub host_line: u32,
    /// 0-indexed column in the host file where the inline content begins.
    pub host_col: u32,
    /// Byte length of the inline content.
    pub content_len: usize,
}

impl InlineRange {
    /// Translate an inline-relative `(line, col)` to host-file `(line, col)`.
    ///
    /// Line 0 of the inline content sits at `(host_line, host_col + inline_col)`.
    /// Any subsequent inline line maps to `(host_line + inline_line, inline_col)`.
    pub fn to_host_position(&self, inline_line: u32, inline_col: u32) -> (u32, u32) {
        let out_line = self.host_line + inline_line;
        let out_col = if inline_line == 0 {
            self.host_col + inline_col
        } else {
            inline_col
        };
        (out_line, out_col)
    }

    /// Translate a host-file `(line, col)` to inline-relative `(line, col)`.
    ///
    /// Returns `None` if the host position is before the inline content starts.
    pub fn to_inline_position(&self, host_line: u32, host_col: u32) -> Option<(u32, u32)> {
        if host_line < self.host_line {
            return None;
        }
        let inline_line = host_line - self.host_line;
        let inline_col = if inline_line == 0 {
            host_col.checked_sub(self.host_col)?
        } else {
            host_col
        };
        Some((inline_line, inline_col))
    }

    /// Return `true` if `byte` falls within this inline region in the host file.
    pub fn contains_host_byte(&self, byte: usize) -> bool {
        byte >= self.host_offset && byte < self.host_offset + self.content_len
    }
}
