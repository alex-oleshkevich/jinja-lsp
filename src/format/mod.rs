// Jinja-only formatter engine — called by both the LSP formatting handler
// and the `jinja-lsp format` CLI front-end (F18).
//
// REQ-FMT-01: normalize delimiter inner spacing to exactly one space.
// REQ-FMT-03: normalize whitespace-control marker spacing (handled by FMT-01 path).
// REQ-FMT-04: normalize filter-pipe spacing, is-test spacing, filter-call arg commas.
// REQ-FMT-07: honor FormattingOptions (tabSize / insertSpaces).

use serde::Deserialize;
use tree_sitter::{Node, Parser};

pub fn layer_name() -> &'static str {
    "format"
}

// ── User-facing formatter config (jinja.toml [format]) ───────────────────────

fn default_indent_size() -> u32 {
    4
}
fn default_true() -> bool {
    true
}

/// User-controlled formatting preferences, readable from `jinja.toml [format]`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormatterConfig {
    /// Indent level in spaces (default 4; ignored when `use_tabs` is true).
    #[serde(default = "default_indent_size")]
    pub indent_size: u32,
    /// Use hard tabs instead of spaces for indentation (default false).
    #[serde(default)]
    pub use_tabs: bool,
    /// Add spaces around the `|` filter pipe operator: `x | filter` vs `x|filter` (default false).
    /// The `is` test operator always has spaces regardless of this setting.
    #[serde(default)]
    pub space_around_pipe: bool,
    /// Add spaces around symbolic binary operators (`+ - * / // % ** ~ == != < > <= >=`):
    /// `a + b` vs `a+b` (default false). Keyword operators (`and`, `or`, `is`, `in`, `not`)
    /// always keep their spaces regardless of this setting — removing them would change
    /// tokenization entirely (e.g. `andb` is one identifier, not `and` + `b`).
    #[serde(default)]
    pub space_around_operators: bool,
    /// Insert a space after each comma in argument lists: `f(a, b)` vs `f(a,b)` (default true).
    #[serde(default = "default_true")]
    pub space_after_comma: bool,
    /// Add a space just inside call parentheses: `f( a, b )` vs `f(a, b)` (default false).
    #[serde(default)]
    pub space_inside_parens: bool,
    /// Add a space just inside `{{ }}`: `{{ x }}` vs `{{x}}` (default true).
    #[serde(default = "default_true")]
    pub space_inside_variable_delimiters: bool,
    /// Add a space just inside `{% %}`: `{% if x %}` vs `{%if x%}` (default true).
    #[serde(default = "default_true")]
    pub space_inside_block_delimiters: bool,
    /// Blank lines to insert after a top-level block-closing tag
    /// (`{% endblock %}`/`{% endfor %}`/`{% endif %}`, …) (default 0).
    #[serde(default)]
    pub blank_lines_after_block: u8,
    /// Strip the first newline immediately after a `{% %}` block tag, mirroring
    /// Jinja2's runtime `trim_blocks` option (default false).
    #[serde(default)]
    pub trim_blocks: bool,
    /// Strip leading whitespace on a line before a `{% %}` block tag, mirroring
    /// Jinja2's runtime `lstrip_blocks` option (default false).
    #[serde(default)]
    pub lstrip_blocks: bool,
    /// Normalize string literal quote style (default `Preserve` — leave as written).
    #[serde(default)]
    pub preferred_quote: QuoteStyle,
    /// Ensure the file ends with a single newline (default true).
    #[serde(default = "default_true")]
    pub newline_at_eof: bool,
    /// Strip trailing whitespace from every line (default true).
    #[serde(default = "default_true")]
    pub trim_trailing_whitespace: bool,
}

/// Preferred string-literal quote style for `preferred_quote` (default `Preserve`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuoteStyle {
    Single,
    Double,
    #[default]
    Preserve,
}

impl QuoteStyle {
    /// The target quote character for this style, or `None` for `Preserve`
    /// (leave string literals exactly as written).
    fn as_char(self) -> Option<char> {
        match self {
            QuoteStyle::Single => Some('\''),
            QuoteStyle::Double => Some('"'),
            QuoteStyle::Preserve => None,
        }
    }
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            indent_size: 4,
            use_tabs: false,
            space_around_pipe: false,
            space_around_operators: false,
            space_after_comma: true,
            space_inside_parens: false,
            space_inside_variable_delimiters: true,
            space_inside_block_delimiters: true,
            blank_lines_after_block: 0,
            trim_blocks: false,
            lstrip_blocks: false,
            preferred_quote: QuoteStyle::Preserve,
            newline_at_eof: true,
            trim_trailing_whitespace: true,
        }
    }
}

// ── LSP compatibility shim ────────────────────────────────────────────────────

/// REQ-FMT-07: Formatting options from the LSP client.
#[derive(Debug, Clone, Copy)]
pub struct FormatOptions {
    /// Number of spaces per indent level (ignored when `insert_spaces` is false).
    pub tab_size: u32,
    /// Use spaces for indentation (true) or hard tabs (false).
    pub insert_spaces: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            tab_size: 4,
            insert_spaces: true,
        }
    }
}

impl FormatOptions {
    /// Merge LSP wire options into a `FormatterConfig`.
    /// File-level concerns (newline_at_eof, trim_trailing_whitespace) are disabled for LSP —
    /// editors handle those independently.
    pub fn into_config(self) -> FormatterConfig {
        FormatterConfig {
            indent_size: self.tab_size,
            use_tabs: !self.insert_spaces,
            newline_at_eof: false,
            trim_trailing_whitespace: false,
            ..FormatterConfig::default()
        }
    }

    /// Merge these LSP wire options onto `base` (the workspace's `FormatterConfig`,
    /// loaded from jinja.toml `[format]`), overriding only `indent_size`/`use_tabs` —
    /// every jinja.toml-only option (space_around_pipe, preferred_quote, …) is taken
    /// from `base` unchanged, and file-level concerns are disabled for LSP the same
    /// way `into_config` disables them (editors handle those independently).
    pub fn merge_into(self, base: &FormatterConfig) -> FormatterConfig {
        FormatterConfig {
            indent_size: self.tab_size,
            use_tabs: !self.insert_spaces,
            newline_at_eof: false,
            trim_trailing_whitespace: false,
            ..base.clone()
        }
    }
}

/// Pure formatting config: 4-space indent, compact pipes, no file-meta concerns.
/// Used by `format()` and integration tests that care about normalization, not file-writing.
const FORMAT_PURE: FormatterConfig = FormatterConfig {
    indent_size: 4,
    use_tabs: false,
    space_around_pipe: false,
    space_around_operators: false,
    space_after_comma: true,
    space_inside_parens: false,
    space_inside_variable_delimiters: true,
    space_inside_block_delimiters: true,
    blank_lines_after_block: 0,
    trim_blocks: false,
    lstrip_blocks: false,
    preferred_quote: QuoteStyle::Preserve,
    newline_at_eof: false,
    trim_trailing_whitespace: false,
};

/// Format `source` with pure normalization — no file-level concerns (newline-at-eof, trailing-whitespace).
/// The CLI uses `format_with_config` with the full loaded `FormatterConfig`.
pub fn format(source: &str) -> String {
    format_with_config(source, &FORMAT_PURE)
}

/// Format `source` respecting the given LSP FormattingOptions (jinja-specific fields use defaults).
pub fn format_with_options(source: &str, opts: FormatOptions) -> String {
    format_with_config(source, &opts.into_config())
}

/// Format `source` with a full `FormatterConfig`.
pub fn format_with_config(source: &str, config: &FormatterConfig) -> String {
    let lang = tree_sitter_jinja::language();
    let mut parser = Parser::new();
    if parser.set_language(&lang).is_err() {
        return source.to_owned();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return source.to_owned(),
    };

    // Skip files with syntax errors — do not risk producing a corrupt result.
    if tree.root_node().has_error() {
        return source.to_owned();
    }

    let bytes = source.as_bytes();
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();

    collect_delimiter_normalizations(tree.root_node(), bytes, config, &mut replacements);

    // Apply delimiter normalizations right-to-left so earlier byte offsets stay valid.
    let after_delimiters = if replacements.is_empty() {
        source.to_owned()
    } else {
        replacements.sort_by_key(|r| std::cmp::Reverse(r.0));
        let mut result = source.to_owned();
        for (start, end, new_text) in replacements {
            result.replace_range(start..end, &new_text);
        }
        result
    };

    // REQ-FMT-02 / REQ-FMT-07: re-indent Jinja-tag lines with the configured indent unit.
    let indent_unit: String = if config.use_tabs {
        "\t".to_owned()
    } else {
        " ".repeat(config.indent_size as usize)
    };
    let after_reindent = reindent(&after_delimiters, &indent_unit);

    // Insert blank lines after a top-level block-closing tag, if configured.
    let after_blank_lines = if config.blank_lines_after_block > 0 {
        insert_blank_lines_after_top_level_blocks(&after_reindent, config.blank_lines_after_block)
    } else {
        after_reindent
    };

    // Jinja2-runtime-style whitespace control, mirrored into the formatted source.
    let after_lstrip = if config.lstrip_blocks {
        apply_lstrip_blocks(&after_blank_lines)
    } else {
        after_blank_lines
    };
    let after_trim_blocks = if config.trim_blocks {
        apply_trim_blocks(&after_lstrip)
    } else {
        after_lstrip
    };

    // Post-processing: trailing-whitespace trim and newline-at-eof.
    let after_trim = if config.trim_trailing_whitespace {
        trim_trailing_whitespace(&after_trim_blocks)
    } else {
        after_trim_blocks
    };

    let result = if config.newline_at_eof {
        ensure_newline_at_eof(after_trim)
    } else {
        after_trim
    };

    if result == source {
        source.to_owned()
    } else {
        result
    }
}

// ── REQ-FMT-01/03/04 — Per-delimiter normalization ───────────────────────────

/// Walk the tree and collect (start_byte, end_byte, normalized_text) for every
/// `render_expression`, `control`, and `comment` node whose text changes.
fn collect_delimiter_normalizations(
    node: Node,
    bytes: &[u8],
    config: &FormatterConfig,
    out: &mut Vec<(usize, usize, String)>,
) {
    let kind = node.kind();
    if matches!(kind, "render_expression" | "control" | "comment") {
        let text = node.utf8_text(bytes).unwrap_or("");
        let normalized = normalize_node(node, text, bytes, config, kind);
        if normalized != text {
            out.push((node.start_byte(), node.end_byte(), normalized));
        }
        // Don't descend — the whole span is replaced.
        return;
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_delimiter_normalizations(cursor.node(), bytes, config, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

// ── REQ-FMT-02 — Block-body re-indentation ───────────────────────────────────

/// Paired Jinja tags that open a new indentation level.
const OPENERS: &[&str] = &[
    "block",
    "for",
    "if",
    "elif",
    "else",
    "macro",
    "call",
    "with",
    "filter",
    "autoescape",
    "trans",
    "raw",
];
/// Tags that close (or re-align at) an indentation level.
const CLOSERS: &[&str] = &[
    "endblock",
    "endfor",
    "endif",
    "endmacro",
    "endcall",
    "endwith",
    "endfilter",
    "elif",
    "else",
    "endset",
    "endautoescape",
    "endtrans",
    "endraw",
];

/// Return true if `line` is a Jinja-tag line: first non-whitespace content is `{%`.
fn is_jinja_tag_line(line: &str) -> bool {
    let t = line.trim_start_matches([' ', '\t']);
    t.starts_with("{%")
}

/// Extract ALL `(keyword, inner_content)` pairs from ALL `{%...%}` tags on a single line.
fn jinja_tag_keywords_on_line(line: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut s = line;
    while let Some(start) = s.find("{%") {
        let after_open = &s[start + 2..];
        if let Some(end) = after_open.find("%}") {
            let inner = &after_open[..end];
            let kw_str = inner.trim_matches('-').trim();
            if let Some(first) = kw_str.split_whitespace().next() {
                result.push((first.to_owned(), kw_str.to_owned()));
            }
            s = &after_open[end + 2..];
        } else {
            break;
        }
    }
    result
}

/// Return true when `(keyword, inner)` acts as a block opener that increases indentation.
fn is_opener(kw: &str, inner: &str) -> bool {
    if kw == "set" {
        // Block set: `{% set name %}…{% endset %}` has no `=`; inline `{% set x = … %}` does.
        return !inner.contains('=');
    }
    OPENERS.contains(&kw)
}

/// HTML void elements: never paired with a closing tag, so they never open a
/// nesting level even without a trailing `/`.
const VOID_HTML_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// HTML tags whose interior content is opaque (JS/CSS/preformatted text) and must
/// never be re-indented, mirroring `{% raw %}`'s Jinja-level equivalent.
const LITERAL_HTML_TAGS: &[&str] = &["script", "style", "pre"];

/// One depth-affecting token found while scanning a line (or multi-line chunk)
/// for combined HTML+Jinja nesting, per djhtml's model of treating both tag
/// systems as one unified stream of indent/dedent events.
struct DepthToken {
    is_opener: bool,
    is_closer: bool,
    /// Set when this token is an HTML opening tag for `<script>`/`<style>`/`<pre>` —
    /// signals the caller to skip re-indenting lines until the matching close tag.
    enters_html_literal: Option<&'static str>,
}

/// Find the first occurrence of ASCII `marker` in `bytes` at or after `start`,
/// skipping over `'...'`/`"..."` quoted regions (which may contain the marker
/// harmlessly, e.g. a `%}` inside a Jinja string literal or a `>` inside an
/// HTML attribute value). Returns `None` if `marker` never appears unquoted.
fn find_unquoted_marker(bytes: &[u8], start: usize, marker: &[u8]) -> Option<usize> {
    let mut i = start;
    let mut in_str: Option<u8> = None;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        if b == b'"' || b == b'\'' {
            in_str = Some(b);
            i += 1;
            continue;
        }
        if bytes[i..].starts_with(marker) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find the unquoted `>` terminating an HTML tag starting at `start` (the byte
/// right after the tag name). Returns `(index_of_gt, is_self_closing)`.
fn find_html_tag_end(bytes: &[u8], start: usize) -> Option<(usize, bool)> {
    let mut i = start;
    let mut in_str: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' | b'\'' => {
                in_str = Some(b);
                i += 1;
            }
            b'>' => {
                let self_close = i > start && bytes[i - 1] == b'/';
                return Some((i, self_close));
            }
            _ => i += 1,
        }
    }
    None
}

/// Scan `text` (which may span multiple physical lines) for every Jinja `{% %}`
/// tag and HTML tag it contains, in left-to-right order, classifying each as an
/// opener/closer/neutral `DepthToken` for combined indentation-depth tracking.
/// Skips the interior of `{{ }}` expressions, `{# #}`/`<!-- -->` comments, and
/// quoted HTML attribute values — none of these affect nesting depth.
///
/// Returns `(tokens, complete)`: `complete` is `false` when `text` ends mid-construct
/// (an unclosed `{%`, `{{`, `{#`, `<!--`, or HTML tag needing more lines to close).
fn scan_depth_tokens(text: &str) -> (Vec<DepthToken>, bool) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    let mut tokens = Vec::new();

    while i < n {
        let rest = &bytes[i..];
        if rest.starts_with(b"<!--") {
            match find_unquoted_marker(bytes, i + 4, b"-->") {
                Some(pos) => i = pos + 3,
                None => return (tokens, false),
            }
            continue;
        }
        if rest.starts_with(b"{#") {
            match find_unquoted_marker(bytes, i + 2, b"#}") {
                Some(pos) => i = pos + 2,
                None => return (tokens, false),
            }
            continue;
        }
        if rest.starts_with(b"{{") {
            match find_unquoted_marker(bytes, i + 2, b"}}") {
                Some(pos) => i = pos + 2,
                None => return (tokens, false),
            }
            continue;
        }
        if rest.starts_with(b"{%") {
            match find_unquoted_marker(bytes, i + 2, b"%}") {
                Some(pos) => {
                    let inner = &text[i + 2..pos];
                    let kw_str = inner.trim_matches('-').trim();
                    if let Some(first) = kw_str.split_whitespace().next() {
                        let is_open = is_opener(first, kw_str);
                        let is_close = CLOSERS.contains(&first);
                        if is_open || is_close {
                            tokens.push(DepthToken {
                                is_opener: is_open,
                                is_closer: is_close,
                                enters_html_literal: None,
                            });
                        }
                    }
                    i = pos + 2;
                }
                None => return (tokens, false),
            }
            continue;
        }
        if rest.starts_with(b"</") {
            let name_end = text[i + 2..]
                .find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == ':'))
                .map(|p| i + 2 + p)
                .unwrap_or(n);
            match find_html_tag_end(bytes, name_end) {
                Some((pos, _)) => {
                    tokens.push(DepthToken {
                        is_opener: false,
                        is_closer: true,
                        enters_html_literal: None,
                    });
                    i = pos + 1;
                }
                None => return (tokens, false),
            }
            continue;
        }
        if rest.starts_with(b"<!") {
            match find_html_tag_end(bytes, i + 2) {
                Some((pos, _)) => i = pos + 1,
                None => return (tokens, false),
            }
            continue;
        }
        if rest.first() == Some(&b'<') && rest.get(1).is_some_and(|c| c.is_ascii_alphabetic()) {
            let name_end = text[i + 1..]
                .find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == ':'))
                .map(|p| i + 1 + p)
                .unwrap_or(n);
            let name = text[i + 1..name_end].to_ascii_lowercase();
            match find_html_tag_end(bytes, name_end) {
                Some((pos, self_close)) => {
                    let is_void = VOID_HTML_ELEMENTS.contains(&name.as_str());
                    if !self_close && !is_void {
                        let literal = LITERAL_HTML_TAGS.iter().copied().find(|&t| t == name);
                        tokens.push(DepthToken {
                            is_opener: true,
                            is_closer: false,
                            enters_html_literal: literal,
                        });
                    }
                    i = pos + 1;
                }
                None => return (tokens, false),
            }
            continue;
        }
        i += text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }

    (tokens, true)
}

/// Return true when `line` (case-insensitively) contains the closing tag for
/// `tag` — used to detect the end of a `<script>`/`<style>`/`<pre>` literal
/// region so its interior can be skipped without being parsed as HTML (its
/// content may contain `<`/`>` that isn't a real tag).
///
/// Requires a word boundary right after the tag name so literal content like
/// `</pretend>` (inside a `<pre>` block) isn't mistaken for `</pre>`.
fn contains_html_closing_tag(line: &str, tag: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let needle = format!("</{tag}");
    let mut start = 0;
    while let Some(rel) = lower[start..].find(&needle) {
        let end = start + rel + needle.len();
        let boundary = lower
            .as_bytes()
            .get(end)
            .is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_' || *b == b':'));
        if boundary {
            return true;
        }
        start += rel + 1;
    }
    false
}

/// Re-indent every line (HTML, `{{ }}` expressions, and `{% %}` tags alike) so its
/// leading whitespace equals `depth × indent_unit`, where depth is the count of
/// open paired constructs — Jinja block tags AND HTML tags — enclosing the line
/// (djhtml's combined-tokenizer model). Content itself (everything after leading
/// whitespace) is never rewritten, only reindented.
pub fn reindent(source: &str, indent_unit: &str) -> String {
    let mut depth: usize = 0;
    let mut out = String::with_capacity(source.len());
    // Inside {% raw %}…{% endraw %}, content is literal output, not Jinja — it
    // must never be re-indented or counted toward depth, even if it looks like
    // a Jinja tag. Only the matching `endraw` line is still processed normally.
    let mut in_raw = false;
    // Inside <script>/<style>/<pre>…</…>, content is opaque host content — never
    // re-indented or scanned for tags. Only the matching close tag line is
    // still processed normally.
    let mut html_literal: Option<&'static str> = None;
    // Inside <!-- … -->, content (which may span lines) is never re-indented or
    // scanned — HTML comments can't be nested and their content is inert.
    let mut in_html_comment = false;

    let lines: Vec<&str> = source.split('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if i > 0 {
            out.push('\n');
        }

        if in_raw {
            let is_endraw = is_jinja_tag_line(line)
                && jinja_tag_keywords_on_line(line)
                    .first()
                    .map(|(kw, _)| kw.as_str())
                    == Some("endraw");
            if !is_endraw {
                out.push_str(line);
                i += 1;
                continue;
            }
            in_raw = false;
        }

        if let Some(tag) = html_literal {
            if !contains_html_closing_tag(line, tag) {
                out.push_str(line);
                i += 1;
                continue;
            }
            html_literal = None;
        }

        if in_html_comment {
            if line.contains("-->") {
                in_html_comment = false;
            }
            out.push_str(line);
            i += 1;
            continue;
        }

        if line.trim().is_empty() {
            out.push_str(line);
            i += 1;
            continue;
        }

        let trimmed_start = line.trim_start_matches([' ', '\t']);
        if trimmed_start.starts_with("<!--") && !trimmed_start.contains("-->") {
            out.push_str(line);
            in_html_comment = true;
            i += 1;
            continue;
        }

        // A tag/construct can split across multiple physical lines (wrapped Jinja
        // tags, wrapped HTML attributes). Accumulate lines until the chunk is
        // "complete" (or EOF). Continuation lines are emitted verbatim — only the
        // chunk's first line is re-indented.
        let mut end = i;
        let mut full_text = line.to_owned();
        loop {
            let (_, complete) = scan_depth_tokens(&full_text);
            if complete || end + 1 >= lines.len() {
                break;
            }
            end += 1;
            full_text.push('\n');
            full_text.push_str(lines[end]);
        }

        let (tokens, _complete) = scan_depth_tokens(&full_text);

        // Closers (endblock, endif, </div>, …) and re-aligners (elif, else) print
        // at depth-1 when they are the first depth-affecting construct on the line.
        if tokens.first().is_some_and(|t| t.is_closer) && depth > 0 {
            depth -= 1;
        }

        // Write the chunk's first line with current depth indentation.
        let stripped = line.trim_start_matches([' ', '\t']);
        for _ in 0..depth {
            out.push_str(indent_unit);
        }
        out.push_str(stripped);

        // Continuation lines (if any) are emitted verbatim.
        for cont_line in &lines[i + 1..=end] {
            out.push('\n');
            out.push_str(cont_line);
        }
        i = end;

        // Compute net depth delta from ALL tokens in the (possibly multi-line) text.
        //
        // First token: if it is a closer, the decrement was already applied above;
        // count it only as an opener (+1) if applicable.
        // Subsequent tokens: pure openers +1, pure closers -1, realigners net 0.
        let mut delta: isize = 0;
        for (idx, tok) in tokens.iter().enumerate() {
            if idx == 0 {
                if tok.is_opener {
                    delta += 1;
                }
            } else if tok.is_opener && !tok.is_closer {
                delta += 1;
            } else if tok.is_closer && !tok.is_opener {
                delta -= 1;
            }
            // realigners (both opener+closer) at non-first position: net 0
        }

        if delta > 0 {
            depth = depth.saturating_add(delta as usize);
        } else if delta < 0 {
            depth = depth.saturating_sub((-delta) as usize);
        }

        // Enter raw mode unless this same line also closes it (e.g. `{% raw %}{% endraw %}`).
        let jinja_keywords = jinja_tag_keywords_on_line(&full_text);
        let first_kw = jinja_keywords
            .first()
            .map(|(kw, _)| kw.as_str())
            .unwrap_or("");
        if first_kw == "raw" && !jinja_keywords.iter().skip(1).any(|(kw, _)| kw == "endraw") {
            in_raw = true;
        }

        // Enter HTML-literal mode only if the last token in the chunk is still an
        // unclosed <script>/<style>/<pre> opener (a same-line `<script></script>`
        // closes itself and must not trigger literal mode).
        if let Some(tag) = tokens.last().and_then(|t| t.enters_html_literal) {
            html_literal = Some(tag);
        }

        i += 1;
    }

    out
}

/// Mirrors Jinja2's runtime `trim_blocks` option: strip the first newline
/// immediately after a `{% %}` block tag's closing delimiter.
fn apply_trim_blocks(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    loop {
        let Some(open_rel) = rest.find("{%") else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..open_rel]);
        let after_open = &rest[open_rel..];
        let Some(close_rel) = after_open.find("%}") else {
            out.push_str(after_open);
            break;
        };
        let close_end = close_rel + 2;
        out.push_str(&after_open[..close_end]);
        let remainder = &after_open[close_end..];
        rest = remainder.strip_prefix('\n').unwrap_or(remainder);
    }
    out
}

/// Mirrors Jinja2's runtime `lstrip_blocks` option: strip leading horizontal
/// whitespace on a line before a `{% %}` block tag (only when the tag is the
/// first non-whitespace content on the line).
fn apply_lstrip_blocks(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    loop {
        let Some(open_rel) = rest.find("{%") else {
            out.push_str(rest);
            break;
        };
        let prefix = &rest[..open_rel];
        let line_start = prefix.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let before_tag = &prefix[line_start..];
        if !before_tag.is_empty() && before_tag.chars().all(|c| c == ' ' || c == '\t') {
            out.push_str(&prefix[..line_start]);
        } else {
            out.push_str(prefix);
        }
        out.push_str("{%");
        rest = &rest[open_rel + 2..];
    }
    out
}

/// Insert `n` blank lines after every top-level block-closing tag (a genuine
/// closer — not `elif`/`else`, which don't return to depth 0 — whose depth
/// transition lands back at 0). Mirrors `reindent`'s depth-tracking exactly so
/// depth here matches the depth `reindent` already computed for the same lines.
fn insert_blank_lines_after_top_level_blocks(source: &str, n: u8) -> String {
    let mut depth: usize = 0;
    let mut in_raw = false;
    let mut out = String::with_capacity(source.len());
    let lines: Vec<&str> = source.split('\n').collect();

    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line);

        if in_raw {
            let is_endraw = is_jinja_tag_line(line)
                && jinja_tag_keywords_on_line(line)
                    .first()
                    .map(|(kw, _)| kw.as_str())
                    == Some("endraw");
            if !is_endraw {
                continue;
            }
            in_raw = false;
        }

        if !is_jinja_tag_line(line) {
            continue;
        }

        let keywords = jinja_tag_keywords_on_line(line);
        let first_kw = keywords.first().map(|(kw, _)| kw.as_str()).unwrap_or("");
        // A "real" closer (excludes elif/else, which re-align within the same
        // block rather than closing it) appearing anywhere on the line.
        let has_real_closer = keywords.iter().any(|(kw, _)| {
            CLOSERS.contains(&kw.as_str()) && !matches!(kw.as_str(), "elif" | "else")
        });
        if CLOSERS.contains(&first_kw) && depth > 0 {
            depth -= 1;
        }

        let mut delta: isize = 0;
        for (idx, (kw, inner)) in keywords.iter().enumerate() {
            let in_openers = is_opener(kw, inner);
            let in_closers = CLOSERS.contains(&kw.as_str());
            if idx == 0 {
                if in_openers {
                    delta += 1;
                }
            } else if in_openers && !in_closers {
                delta += 1;
            } else if in_closers && !in_openers {
                delta -= 1;
            }
        }
        if delta > 0 {
            depth = depth.saturating_add(delta as usize);
        } else if delta < 0 {
            depth = depth.saturating_sub((-delta) as usize);
        }

        if first_kw == "raw" && !keywords.iter().skip(1).any(|(kw, _)| kw == "endraw") {
            in_raw = true;
        }

        // A genuine top-level closer: depth is 0 after this line and the line
        // contains a real closer (covers both a normal multi-line close and a
        // fully self-contained same-line block like `{% block a %}x{% endblock %}`).
        let closed_to_top_level = depth == 0 && has_real_closer;
        if closed_to_top_level {
            // Don't duplicate blank lines the source (or a prior pass) already has,
            // and don't add trailing blank lines past the end of the file.
            let mut already_blank = 0usize;
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                already_blank += 1;
                j += 1;
            }
            let is_last_content_line = j >= lines.len();
            if !is_last_content_line {
                for _ in already_blank..n as usize {
                    out.push('\n');
                }
            }
        }
    }

    out
}

/// Apply all active passes to a single delimiter node's text.
///
/// Pipeline: FMT-04 sub-edits first (relative positions), then FMT-01 outer padding.
fn normalize_node(
    node: Node,
    text: &str,
    bytes: &[u8],
    config: &FormatterConfig,
    kind: &str,
) -> String {
    let node_start = node.start_byte();

    // Collect FMT-04 edits: relative byte positions within `text`.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    collect_fmt04_edits(node, bytes, node_start, config, &mut edits);

    // Apply FMT-04 edits right-to-left.
    let content = if edits.is_empty() {
        text.to_owned()
    } else {
        edits.sort_by_key(|e| std::cmp::Reverse(e.0));
        let mut buf = text.to_owned();
        for (start, end, new_text) in edits {
            buf.replace_range(start..end, &new_text);
        }
        buf
    };

    // Apply FMT-01: normalize outer delimiter padding. Comments always keep a
    // space; `{{ }}`/`{% %}` follow the configured variable/block preference.
    let add_space = match kind {
        "render_expression" => config.space_inside_variable_delimiters,
        "control" => config.space_inside_block_delimiters,
        _ => true,
    };
    normalize_delimiter_padding(&content, add_space)
}

// ── REQ-FMT-04 — Filter-pipe / is-test / filter-call-arg normalization ────────

/// Walk the delimiter subtree and collect FMT-04 edits as (rel_start, rel_end, new_text).
/// `node_start` is the absolute byte of the enclosing delimiter — used to convert to relative.
fn collect_fmt04_edits(
    node: Node,
    bytes: &[u8],
    node_start: usize,
    config: &FormatterConfig,
    out: &mut Vec<(usize, usize, String)>,
) {
    // Symbolic operators `space_around_operators` toggles. Keyword operators (`is`,
    // `and`, `or`) are excluded — removing their surrounding whitespace would change
    // tokenization entirely (e.g. "andb" is one identifier, not "and" + "b"), so they
    // always keep spaces regardless of any setting, the same as before this option existed.
    const SYMBOLIC_OPERATORS: &[&str] = &[
        "+", "-", "*", "/", "//", "%", "**", "~", "==", "!=", "<", ">", "<=", ">=",
    ];
    if node.kind() == "binary_operator" {
        let op = node.utf8_text(bytes).unwrap_or("");
        if op == "|" {
            // `space_around_pipe` controls `|`: compact (`x|filter`) vs spaced (`x | filter`).
            let (ws_start, ws_end) =
                surrounding_whitespace(bytes, node.start_byte(), node.end_byte());
            let rel_start = ws_start.saturating_sub(node_start);
            let rel_end = ws_end.saturating_sub(node_start);
            let normalized = if config.space_around_pipe {
                format!(" {op} ")
            } else {
                op.to_owned()
            };
            out.push((rel_start, rel_end, normalized));
        } else if op == "is" {
            // `is` is a keyword operator — spaces are always required to remain valid syntax.
            let (ws_start, ws_end) =
                surrounding_whitespace(bytes, node.start_byte(), node.end_byte());
            let rel_start = ws_start.saturating_sub(node_start);
            let rel_end = ws_end.saturating_sub(node_start);
            out.push((rel_start, rel_end, format!(" {op} ")));
        } else if config.space_around_operators && SYMBOLIC_OPERATORS.contains(&op) {
            // Opt-in only: default (false) leaves these operators exactly as written,
            // matching the pre-existing "formatter, not beautifier" behavior so
            // FormatOptions::default() never changes output for these operators.
            let (ws_start, ws_end) =
                surrounding_whitespace(bytes, node.start_byte(), node.end_byte());
            let rel_start = ws_start.saturating_sub(node_start);
            let rel_end = ws_end.saturating_sub(node_start);
            out.push((rel_start, rel_end, format!(" {op} ")));
        }
        return; // binary_operator has no relevant children
    }

    if node.kind() == "function_call" && is_filter_call(node, bytes) {
        if let Some(normalized) = normalize_filter_call(node, bytes, config) {
            let rel_start = node.start_byte().saturating_sub(node_start);
            let rel_end = node.end_byte().saturating_sub(node_start);
            out.push((rel_start, rel_end, normalized));
        }
        // Don't descend into filter calls — we've replaced the whole span.
        return;
    }

    if node.kind() == "string_literal" {
        if let Some(target) = config.preferred_quote.as_char() {
            let text = node.utf8_text(bytes).unwrap_or("");
            if let Some(normalized) = normalize_string_quote(text, target) {
                let rel_start = node.start_byte().saturating_sub(node_start);
                let rel_end = node.end_byte().saturating_sub(node_start);
                out.push((rel_start, rel_end, normalized));
            }
        }
        return; // string_literal has no relevant children
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_fmt04_edits(cursor.node(), bytes, node_start, config, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Return [ws_start, ws_end) spanning the operator AND any surrounding horizontal whitespace.
fn surrounding_whitespace(bytes: &[u8], op_start: usize, op_end: usize) -> (usize, usize) {
    let mut ws_start = op_start;
    while ws_start > 0 && (bytes[ws_start - 1] == b' ' || bytes[ws_start - 1] == b'\t') {
        ws_start -= 1;
    }
    let mut ws_end = op_end;
    while ws_end < bytes.len() && (bytes[ws_end] == b' ' || bytes[ws_end] == b'\t') {
        ws_end += 1;
    }
    (ws_start, ws_end)
}

/// Return `true` when `func_call` is the right operand of a `|` binary_expression.
///
/// Grammar path: function_call → primary_expression → unary_expression → [right side of `|`]
fn is_filter_call(func_call: Node, bytes: &[u8]) -> bool {
    let Some(primary) = func_call.parent() else {
        return false;
    };
    if primary.kind() != "primary_expression" {
        return false;
    }
    let Some(unary) = primary.parent() else {
        return false;
    };
    if unary.kind() != "unary_expression" {
        return false;
    }
    let Some(binary) = unary.parent() else {
        return false;
    };
    if binary.kind() != "binary_expression" {
        return false;
    }
    // The binary_expression's operator must be `|`.
    let mut cursor = binary.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "binary_operator" {
                return child.utf8_text(bytes).unwrap_or("") == "|";
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    false
}

/// Convert `text` (a full `'...'` or `"..."` string literal, quotes included) to
/// use `target` as its quote character. Returns `None` when nothing should change:
/// already using `target`, malformed, or the content contains an unescaped
/// occurrence of `target` (re-escaping is skipped conservatively to avoid ever
/// producing a subtly wrong string).
fn normalize_string_quote(text: &str, target: char) -> Option<String> {
    let orig = text.chars().next()?;
    if orig != '\'' && orig != '"' || orig == target {
        return None;
    }
    if text.len() < 2 || !text.ends_with(orig) {
        return None;
    }
    let inner = &text[orig.len_utf8()..text.len() - orig.len_utf8()];

    // Unescape `\<orig>` — no longer needed once the delimiter changes.
    let mut unescaped = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if chars.peek() == Some(&orig) {
                unescaped.push(orig);
                chars.next();
                continue;
            }
            unescaped.push(c);
            if let Some(next) = chars.next() {
                unescaped.push(next);
            }
            continue;
        }
        unescaped.push(c);
    }

    // Bail if the result would contain an unescaped `target` char — that would
    // require re-escaping, which this conservative pass does not attempt.
    let mut it = unescaped.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' {
            it.next();
            continue;
        }
        if c == target {
            return None;
        }
    }

    Some(format!("{target}{unescaped}{target}"))
}

/// Reconstruct a filter-call with normalized arg spacing: `name(arg1, arg2, ...)`.
///
/// Returns `None` if the call has no arguments (nothing to normalize).
fn normalize_filter_call(
    func_call: Node,
    bytes: &[u8],
    config: &FormatterConfig,
) -> Option<String> {
    // First named child is the identifier (function name).
    let name_node = func_call.named_child(0)?;
    let name = name_node.utf8_text(bytes).ok()?;

    // Collect all `arg` children.
    let mut args: Vec<String> = Vec::new();
    let mut cursor = func_call.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "arg" {
                let arg_text = child.utf8_text(bytes).ok()?;
                args.push(arg_text.trim().to_owned());
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    if args.is_empty() {
        return None;
    }

    let sep = if config.space_after_comma { ", " } else { "," };
    let joined = args.join(sep);
    let reconstructed = if config.space_inside_parens {
        format!("{name}( {joined} )")
    } else {
        format!("{name}({joined})")
    };
    let original = func_call.utf8_text(bytes).ok()?;
    if reconstructed == original {
        None
    } else {
        Some(reconstructed)
    }
}

// ── REQ-FMT-01 — Delimiter outer padding ─────────────────────────────────────

/// Normalize the padding just inside a single Jinja delimiter, always adding
/// exactly one space (the historical/default behavior for `{{ }}`/`{% %}`/`{# #}`).
///
/// Handles optional whitespace-control markers (`{%-`, `-%}` etc.) and
/// preserves multi-line comment interior by trimming only boundary whitespace.
pub fn normalize_delimiter(text: &str) -> String {
    normalize_delimiter_padding(text, true)
}

/// Like `normalize_delimiter`, but `add_space` controls whether exactly one space
/// is inserted inside the delimiter (`true`, the historical behavior) or the
/// content is packed tight against the delimiter markers (`false`).
///
/// `space_inside_variable_delimiters`/`space_inside_block_delimiters` drive this
/// for `{{ }}`/`{% %}`; comment delimiters (`{# #}`) always use `add_space = true`.
fn normalize_delimiter_padding(text: &str, add_space: bool) -> String {
    // Detect opening: {{-, {{, {%-, {%, {#-, {#
    let (open, rest) = if let Some(r) = text.strip_prefix("{{-") {
        ("{{-", r)
    } else if let Some(r) = text.strip_prefix("{%-") {
        ("{%-", r)
    } else if let Some(r) = text.strip_prefix("{#-") {
        ("{#-", r)
    } else if let Some(r) = text.strip_prefix("{{") {
        ("{{", r)
    } else if let Some(r) = text.strip_prefix("{%") {
        ("{%", r)
    } else if let Some(r) = text.strip_prefix("{#") {
        ("{#", r)
    } else {
        return text.to_owned();
    };

    // Detect closing: -}}, }}, -%}, %}, -#}, #}
    let (content, close) = if let Some(c) = rest.strip_suffix("-}}") {
        (c, "-}}")
    } else if let Some(c) = rest.strip_suffix("-%}") {
        (c, "-%}")
    } else if let Some(c) = rest.strip_suffix("-#}") {
        (c, "-#}")
    } else if let Some(c) = rest.strip_suffix("}}") {
        (c, "}}")
    } else if let Some(c) = rest.strip_suffix("%}") {
        (c, "%}")
    } else if let Some(c) = rest.strip_suffix("#}") {
        (c, "#}")
    } else {
        return text.to_owned();
    };

    // Trim only horizontal whitespace at boundaries (preserves multi-line interiors).
    let trimmed = content.trim_matches([' ', '\t']);
    if add_space {
        // jinja-lsp-q12j: only pad with a space when the adjacent content isn't
        // already a newline — otherwise the padding sits as trailing whitespace at
        // the end of the tag-open line (or leading whitespace before the close on
        // a multi-line comment/tag), which LSP-mode formatting never trims away
        // (FormatOptions::into_config disables trim_trailing_whitespace for editors).
        let lead = if trimmed.starts_with('\n') { "" } else { " " };
        let trail = if trimmed.ends_with('\n') { "" } else { " " };
        format!("{open}{lead}{trimmed}{trail}{close}")
    } else {
        format!("{open}{trimmed}{close}")
    }
}

// ── Post-processing helpers ───────────────────────────────────────────────────

/// Strip trailing horizontal whitespace from every line.
fn trim_trailing_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, line) in s.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line.trim_end_matches([' ', '\t']));
    }
    out
}

/// Ensure `s` ends with exactly one `\n`.
fn ensure_newline_at_eof(mut s: String) -> String {
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}
