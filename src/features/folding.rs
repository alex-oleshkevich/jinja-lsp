// REQ-FOLD2-01..06: folding ranges for the Jinja layer.
//
// The tree-sitter Jinja grammar uses FLAT statement nodes — block_statement,
// for_statement etc. cover only the opening tag, not the full block.  We
// therefore compute folds via source-text scanning:
//
//   1. Scan `{%…%}`, `{{…}}`, and `{#…#}` delimiters for tag events.
//   2. For block-like tags, use the universal `end<name>` convention and a
//      name-keyed stack to match openers with their closers.
//   3. Emit a FoldRange for each matched pair (and for multi-line comments /
//      multi-line tags) when start_line != end_line.
//
// Unmatched openers (no matching closer) and stray closers produce no range —
// satisfying REQ-FOLD2-06.

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldKind {
    Region,
    Comment,
}

#[derive(Debug, Clone)]
pub struct FoldRange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: FoldKind,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Compute all Jinja-layer folding ranges for `source`.
///
/// Returns one `FoldRange` per:
/// - Matched `{% name %}…{% endname %}` block pair (kind=Region, REQ-FOLD2-01).
/// - Multi-line `{# … #}` comment (kind=Comment, REQ-FOLD2-02).
/// - Multi-line `{{ … }}` or `{% … %}` tag (kind=Region, REQ-FOLD2-03).
///
/// Only ranges where `start_line != end_line` are returned (REQ-FOLD2-04).
pub fn fold_ranges(source: &str) -> Vec<FoldRange> {
    let events = scan_events(source);
    build_ranges(&events)
}

// ── Event scanning ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum EventKind {
    BlockOpen(String), // keyword name (e.g. "block", "for", "cache")
    BlockClose(String), // end-keyword without "end" prefix (e.g. "block", "for")
    Tag,               // any `{%…%}` or `{{…}}` (for multi-line tag fold)
    Comment,           // `{#…#}`
}

#[derive(Debug, Clone)]
struct Event {
    kind: EventKind,
    start_line: u32,
    end_line: u32,
}

fn count_newlines(s: &str) -> u32 {
    s.bytes().filter(|&b| b == b'\n').count() as u32
}

fn scan_events(source: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut current_line: u32 = 0;
    // REQ-FOLD2-01/§5.6: track whether we are inside a {% raw %}…{% endraw %} body.
    // While in_raw, only {% endraw %} is processed; all other delimiters are skipped.
    let mut in_raw = false;

    while i < bytes.len() {
        if i + 1 >= bytes.len() {
            if bytes[i] == b'\n' {
                current_line += 1;
            }
            i += 1;
            continue;
        }

        if bytes[i] == b'{' && bytes[i + 1] == b'%' {
            if let Some(close_rel) = source[i + 2..].find("%}") {
                let inner = &source[i + 2..i + 2 + close_rel];
                let tag_end = i + 2 + close_rel + 2;
                let sl = current_line;
                let el = current_line + count_newlines(&source[i..tag_end.saturating_sub(1)]);
                let keyword = inner.trim_matches('-').trim().split_whitespace().next().unwrap_or("");
                if in_raw {
                    // Inside raw body: only endraw exits raw mode; other tags are literal text.
                    if keyword == "endraw" {
                        in_raw = false;
                        events.push(Event { kind: EventKind::BlockClose("raw".to_owned()), start_line: sl, end_line: el });
                    }
                } else {
                    if keyword == "raw" {
                        in_raw = true;
                    }
                    let kind = classify_statement(inner);
                    events.push(Event { kind, start_line: sl, end_line: el });
                }
                current_line += count_newlines(&source[i..tag_end]);
                i = tag_end;
                continue;
            }
        } else if !in_raw && bytes[i] == b'{' && bytes[i + 1] == b'#' {
            if let Some(close_rel) = source[i + 2..].find("#}") {
                let tag_end = i + 2 + close_rel + 2;
                let sl = current_line;
                let el = current_line + count_newlines(&source[i..tag_end.saturating_sub(1)]);
                events.push(Event { kind: EventKind::Comment, start_line: sl, end_line: el });
                current_line += count_newlines(&source[i..tag_end]);
                i = tag_end;
                continue;
            }
        } else if !in_raw && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(close_rel) = source[i + 2..].find("}}") {
                let tag_end = i + 2 + close_rel + 2;
                let sl = current_line;
                let el = current_line + count_newlines(&source[i..tag_end.saturating_sub(1)]);
                events.push(Event { kind: EventKind::Tag, start_line: sl, end_line: el });
                current_line += count_newlines(&source[i..tag_end]);
                i = tag_end;
                continue;
            }
        }

        if bytes[i] == b'\n' {
            current_line += 1;
        }
        i += source[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }

    events
}

/// Classify a `{% … %}` statement inner content as block-open, block-close, or tag.
fn classify_statement(inner: &str) -> EventKind {
    let s = inner.trim_matches('-').trim();
    let first = s.split_whitespace().next().unwrap_or("");
    // Strip an argument list that follows the keyword without whitespace (e.g. `call(x)`).
    let keyword = first.find('(').map(|p| &first[..p]).unwrap_or(first);

    // Intermediate clauses are neither openers nor closers.
    if matches!(keyword, "elif" | "else" | "pluralize") {
        return EventKind::Tag;
    }

    // Closers: end<name>
    if let Some(suffix) = keyword.strip_prefix("end") {
        if !suffix.is_empty() {
            return EventKind::BlockClose(suffix.to_owned());
        }
    }

    // Openers: any non-empty, non-standalone keyword.
    if !keyword.is_empty() && !is_standalone(keyword) {
        return EventKind::BlockOpen(keyword.to_owned());
    }

    EventKind::Tag
}

/// Keywords that never open a paired block.
fn is_standalone(keyword: &str) -> bool {
    matches!(
        keyword,
        "extends" | "include" | "import" | "from" | "set" | "break" | "continue" | "debug" | "do"
    )
}

// ── Range building ────────────────────────────────────────────────────────────

fn build_ranges(events: &[Event]) -> Vec<FoldRange> {
    let mut result = Vec::new();
    // Stack of (name, start_line) for open block tags.
    let mut stack: Vec<(String, u32)> = Vec::new();

    for event in events {
        match &event.kind {
            EventKind::BlockOpen(name) => {
                // REQ-FOLD2-03: multi-line opener tag itself also gets a fold.
                if event.start_line != event.end_line {
                    result.push(FoldRange {
                        start_line: event.start_line,
                        end_line: event.end_line,
                        kind: FoldKind::Region,
                    });
                }
                stack.push((name.clone(), event.start_line));
            }

            EventKind::BlockClose(name) => {
                // REQ-FOLD2-03: multi-line closer tag itself also gets a fold.
                if event.start_line != event.end_line {
                    result.push(FoldRange {
                        start_line: event.start_line,
                        end_line: event.end_line,
                        kind: FoldKind::Region,
                    });
                }
                // Match ONLY the top-of-stack opener (REQ-FOLD2-06): if an intervening
                // opener sits above this closer on the stack, the tags are interleaved and
                // neither produces a valid fold. rposition would wrongly skip past them.
                if stack.last().map(|(n, _)| n == name).unwrap_or(false) {
                    let (_, open_line) = stack.pop().unwrap();
                    if open_line != event.end_line {
                        result.push(FoldRange {
                            start_line: open_line,
                            end_line: event.end_line,
                            kind: FoldKind::Region,
                        });
                    }
                }
                // Stray/interleaved closers are silently ignored (REQ-FOLD2-06).
            }

            EventKind::Comment => {
                if event.start_line != event.end_line {
                    result.push(FoldRange {
                        start_line: event.start_line,
                        end_line: event.end_line,
                        kind: FoldKind::Comment,
                    });
                }
            }

            EventKind::Tag => {
                // Multi-line tag fold (REQ-FOLD2-03).
                if event.start_line != event.end_line {
                    result.push(FoldRange {
                        start_line: event.start_line,
                        end_line: event.end_line,
                        kind: FoldKind::Region,
                    });
                }
            }
        }
    }

    // Unclosed openers remaining on stack → no range (REQ-FOLD2-06).
    result
}
