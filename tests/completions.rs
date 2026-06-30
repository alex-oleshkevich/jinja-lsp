// F05 — Completions tests: REQ-CMP-01 through REQ-CMP-12.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::features::completions::{complete, resolve_doc};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── REQ-CMP-02: each cursor context maps to the right candidates ─────────────

#[test]
fn cmp02_filter_context_offers_filters() {
    let src = "{{ x | ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    assert!(!items.is_empty(), "filter context must offer items");
    // Must include built-in filters
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"truncate"), "truncate must be offered: {labels:?}");
    assert!(labels.contains(&"upper"), "upper must be offered: {labels:?}");
    assert!(labels.contains(&"lower"), "lower must be offered: {labels:?}");
}

#[test]
fn cmp02_filter_context_does_not_offer_variables() {
    let src = "{{ x | ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    // Should not offer non-filter items; all items should be filters
    for item in &items {
        assert!(
            item.detail.as_deref().unwrap_or("").contains("filter")
                || item.data.as_deref().unwrap_or("").contains("filter"),
            "item {:?} in filter context must be a filter",
            item.label
        );
    }
}

#[test]
fn cmp02_expression_context_offers_variables_and_functions() {
    let src = "{% set my_var = 1 %}{{ ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    assert!(!items.is_empty(), "expression context must offer items");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    // my_var should be in scope
    assert!(labels.contains(&"my_var"), "set variable must be offered: {labels:?}");
    // range() is a built-in function
    assert!(labels.contains(&"range"), "global function 'range' must be offered: {labels:?}");
}

#[test]
fn cmp02_statement_context_offers_keywords() {
    let src = "{% ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    assert!(!items.is_empty(), "statement context must offer items");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"for"), "keyword 'for' must be offered: {labels:?}");
    assert!(labels.contains(&"if"), "keyword 'if' must be offered: {labels:?}");
    assert!(labels.contains(&"block"), "keyword 'block' must be offered: {labels:?}");
    assert!(labels.contains(&"macro"), "keyword 'macro' must be offered: {labels:?}");
}

#[test]
fn cmp02_attribute_context_offers_attrs_for_known_receiver() {
    let src = "{% for i in items %}{{ loop.";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    assert!(!items.is_empty(), "attribute context for 'loop' must offer items");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"index"), "loop.index must be offered: {labels:?}");
    assert!(labels.contains(&"first"), "loop.first must be offered: {labels:?}");
}

// ─── REQ-CMP-03: attribute completions only for resolvable receivers ──────────

#[test]
fn cmp03_unknown_receiver_returns_empty() {
    let src = "{{ unknown_obj.";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    assert!(items.is_empty(), "unknown receiver must yield no completions: {items:?}");
}

// ─── REQ-CMP-05: lazy documentation resolve ───────────────────────────────────

#[test]
fn cmp05_resolve_returns_markdown_body() {
    let reg = Registry::load_core();
    // "filter:truncate" is the data key
    let doc = resolve_doc("filter:truncate", &reg);
    assert!(doc.is_some(), "resolve must return doc for truncate");
    let d = doc.unwrap();
    assert!(d.contains("truncate"), "resolved doc must mention truncate");
}

#[test]
fn cmp05_resolve_returns_none_for_unknown() {
    let reg = Registry::load_core();
    let doc = resolve_doc("filter:no_such_filter_xyz", &reg);
    assert!(doc.is_none(), "resolve must return None for unknown symbol");
}

// ─── REQ-CMP-06: nothing outside Jinja delimiters ────────────────────────────

#[test]
fn cmp06_outside_delimiter_returns_empty() {
    let src = "<p>Hello world</p>";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor in the middle of plain HTML
    let (items, _) = complete(src, 0, 5, &idx, &reg, &ws);
    assert!(items.is_empty(), "plain HTML must return empty completions");
}

#[test]
fn cmp06_inside_comment_returns_empty() {
    let src = "{# some comment text #}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, 10, &idx, &reg, &ws);
    assert!(items.is_empty(), "comment interior must return empty completions");
}

// ─── REQ-CMP-07: item fields ─────────────────────────────────────────────────

#[test]
fn cmp07_filter_item_has_required_fields() {
    let src = "{{ x | ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let truncate = items.iter().find(|i| i.label == "truncate").unwrap();
    assert!(!truncate.label.is_empty(), "label must be set");
    // detail should contain filter information
    assert!(truncate.detail.is_some(), "detail must be set");
    // data must be set for lazy resolve
    assert!(truncate.data.is_some(), "data must be set for resolve");
    let data = truncate.data.as_deref().unwrap();
    assert!(data.contains("truncate"), "data must contain the symbol name");
}

#[test]
fn cmp07_item_has_no_documentation_initially() {
    // Items ship without documentation; resolve adds it (REQ-CMP-05).
    let src = "{{ x | ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let any_with_doc = items.iter().any(|i| i.documentation.is_some());
    assert!(!any_with_doc, "items must not have documentation until resolved");
}

// ─── REQ-CMP-11: scope-local variables in expression position ────────────────

#[test]
fn cmp11_for_loop_var_offered_in_expression() {
    // The loop variable "item" should be offered inside {{ }} within the loop.
    let src = "{% for item in items %}{{ ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"item"), "for loop variable must be offered: {labels:?}");
}

#[test]
fn cmp11_set_var_offered_in_expression() {
    let src = "{% set title = 'Hello' %}{{ ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"title"), "set variable must be offered: {labels:?}");
}

// ─── REQ-CMP-12: template paths one directory at a time ──────────────────────

#[test]
fn cmp12_path_context_offers_workspace_templates() {
    let src = r#"{% extends ""#;
    let idx = extract(src);
    let reg = Registry::load_core();
    // Workspace with one template
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", "{% block content %}{% endblock %}");
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"base.html"), "workspace template must be offered: {labels:?}");
}

#[test]
fn cmp12_single_quoted_path_also_triggers_template_completion() {
    // Single-quoted string in extends must also enter TemplatePath context.
    let src = "{% extends '";
    let idx = extract(src);
    let reg = Registry::load_core();
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", "{% block content %}{% endblock %}");
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"base.html"), "single-quoted extends must offer workspace templates: {labels:?}");
}

#[test]
fn cmp12_include_single_quoted_path_triggers_completion() {
    let src = "{% include '";
    let idx = extract(src);
    let reg = Registry::load_core();
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("nav.html", "");
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"nav.html"), "single-quoted include must offer workspace templates: {labels:?}");
}

// ─── REQ-CMP-08: keyword-argument name completion inside call parens ──────────

#[test]
fn cmp08_local_macro_params_offered_in_call() {
    // Macro defined in same template; cursor inside the call parens.
    let src = "{% macro render(title, body='') %}{% endmacro %}{{ render(";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.iter().any(|l| l.starts_with("title")), "title param must be offered: {labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("body")), "body param must be offered: {labels:?}");
}

#[test]
fn cmp08_params_offered_after_comma() {
    // After typing the first arg and a comma, still offer remaining params.
    let src = "{% macro greet(name, msg) %}{% endmacro %}{{ greet(name='hi', ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.iter().any(|l| l.starts_with("msg")), "msg param must be offered after comma: {labels:?}");
}

#[test]
fn cmp08_from_import_macro_params_offered() {
    // Macro imported via from-import; cursor inside call parens.
    let src = r#"{% from "macros.html" import card %}{{ card("#;
    let idx = extract(src);
    // Simulate workspace providing macro definition.
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("macros.html", "{% macro card(title, variant='default') %}{% endmacro %}");
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.iter().any(|l| l.starts_with("title")), "title param must be offered for from-import: {labels:?}");
}

fn reg() -> Registry { Registry::load_core() }

#[test]
fn cmp08_filter_call_params_offered() {
    // REQ-CMP-08: cursor inside a filter call must offer that filter's keyword args.
    // `{{ x | truncate( }}` — the open paren wins over the bare `|` context.
    let src = "{{ x | truncate( }}";
    let idx = extract(src);
    let ws = WorkspaceIndex::default();
    let col = src.find('(').unwrap() as u32 + 1;
    let (items, _) = complete(src, 0, col, &idx, &reg(), &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.contains("length")),
        "truncate 'length=' must be offered inside filter call: {labels:?}"
    );
    // Must NOT be the full filter list
    assert!(
        !labels.iter().any(|l| *l == "upper"),
        "full filter list must NOT be offered inside filter call: {labels:?}"
    );
}

#[test]
fn cmp08_filter_call_kwargs_are_keyword_arg_kind() {
    // Items inside a filter-call context must have KeywordArg kind, not Filter.
    let src = "{{ x | truncate( }}";
    let idx = extract(src);
    let ws = WorkspaceIndex::default();
    let col = src.find('(').unwrap() as u32 + 1;
    let (items, _) = complete(src, 0, col, &idx, &reg(), &ws);
    assert!(!items.is_empty(), "must have items inside filter call paren");
    for item in &items {
        assert_eq!(
            item.kind,
            jinja_lsp::features::completions::CompletionKind::KeywordArg,
            "filter call kwarg items must have kind=KeywordArg, got {:?} for {:?}",
            item.kind, item.label
        );
    }
}

#[test]
fn bovp_detect_context_mid_multibyte_char_does_not_panic() {
    // "é" is 2 bytes; byte 1 is NOT a char boundary.
    // detect_context must not panic when given a mid-char byte offset.
    let src = "{{ é }}";
    let idx = extract(src);
    let ws = WorkspaceIndex::default();
    let (_items, _) = complete(src, 0, 4, &idx, &reg(), &ws); // byte 4 is mid-char (é = bytes 3-4)
}

// ─── REQ-CMP-04: import-name completion after `from "x" import` ──────────────

#[test]
fn cmp04_from_import_offers_macro_names() {
    // After `from "macros.html" import ` — cursor at the end
    // → should offer "post_url" and "card" from macros.html
    let src = r#"{% from "macros.html" import "#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("macros.html", "{% macro post_url(post) %}{% endmacro %}{% macro card() %}{% endmacro %}");
    let idx = extract(src);
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"post_url"), "post_url must be offered; got: {labels:?}");
    assert!(labels.contains(&"card"), "card must be offered; got: {labels:?}");
}

#[test]
fn cmp04_from_import_after_comma_offers_remaining_macros() {
    // After `from "macros.html" import post_url, ` — cursor at end
    let src = r#"{% from "macros.html" import post_url, "#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("macros.html", "{% macro post_url(post) %}{% endmacro %}{% macro card() %}{% endmacro %}");
    let idx = extract(src);
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"card"), "card must be offered after comma; got: {labels:?}");
}

#[test]
fn cmp04_from_import_unknown_source_returns_empty() {
    let src = r#"{% from "nonexistent.html" import "#;
    let ws = WorkspaceIndex::default(); // no templates
    let idx = extract(src);
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &ws);
    assert!(items.is_empty(), "unknown source must return no completions; got: {items:?}");
}

// ─── REQ-CMP-09: block-aware end-tag completion ──────────────────────────────

#[test]
fn cmp09_end_inside_for_offers_only_endfor() {
    let src = "{% for item in items %}{% end";
    let idx = extract(src);
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &WorkspaceIndex::default());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["endfor"], "only endfor must be offered inside a for loop: {labels:?}");
}

#[test]
fn cmp09_end_inside_block_offers_only_endblock() {
    let src = "{% block content %}{% end";
    let idx = extract(src);
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &WorkspaceIndex::default());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["endblock"], "only endblock must be offered inside a block: {labels:?}");
}

#[test]
fn cmp09_end_with_no_open_block_returns_empty() {
    let src = "{% end";
    let idx = extract(src);
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &WorkspaceIndex::default());
    assert!(items.is_empty(), "no open block → no end-tag completions: {items:?}");
}

// ─── REQ-CMP-10: scope-gated special objects ─────────────────────────────────

#[test]
fn cmp10_loop_offered_inside_for() {
    let src = "{% for x in xs %}{{ ";
    let idx = extract(src);
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &WorkspaceIndex::default());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"loop"), "loop must be offered inside for: {labels:?}");
}

#[test]
fn cmp10_loop_not_offered_outside_for() {
    let src = "{{ ";
    let idx = extract(src);
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &WorkspaceIndex::default());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(!labels.contains(&"loop"), "loop must not be offered outside for: {labels:?}");
}

// ─── REQ-CMP-12: directory-level path completion ─────────────────────────────

#[test]
fn cmp12_folder_items_have_folder_kind() {
    let src = r#"{% extends ""#;
    let idx = extract(src);
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("blog/post.html", "");
    ws.index_inline("blog/index.html", "");
    ws.index_inline("base.html", "");
    let (items, is_incomplete) = complete(src, 0, src.len() as u32, &idx, &reg(), &ws);
    use jinja_lsp::features::completions::CompletionKind;
    let folder_labels: Vec<&str> = items.iter()
        .filter(|i| i.kind == CompletionKind::Folder)
        .map(|i| i.label.as_str()).collect();
    let file_labels: Vec<&str> = items.iter()
        .filter(|i| i.kind == CompletionKind::File)
        .map(|i| i.label.as_str()).collect();
    assert!(folder_labels.contains(&"blog/"), "blog/ must be offered as a folder: {folder_labels:?}");
    assert!(file_labels.contains(&"base.html"), "base.html must be offered as a file: {file_labels:?}");
    assert!(is_incomplete, "is_incomplete must be true when folders are present");
}

#[test]
fn cmp12_prefix_filters_to_correct_level() {
    let src = r#"{% extends "blog/"#;
    let idx = extract(src);
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("blog/post.html", "");
    ws.index_inline("blog/index.html", "");
    ws.index_inline("base.html", "");
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"blog/post.html"), "post.html must appear after typing blog/: {labels:?}");
    assert!(!labels.contains(&"base.html"), "base.html must not appear after prefix blog/");
}

// ─── jinja-lsp-fq3s: negative contract — string literals, raw blocks, alias slots ───

#[test]
fn fq3s_pipe_inside_string_literal_returns_empty() {
    // `{{ "foo|bar" }}` — the `|` is inside a string literal, not a filter operator.
    // Must not classify as Filter context.
    let src = r#"{{ "foo|bar" }}"#;
    let idx = extract(src);
    let ws = WorkspaceIndex::default();
    let col = src.find('|').unwrap() as u32 + 1;
    let (items, _) = complete(src, 0, col, &idx, &reg(), &ws);
    assert!(items.is_empty(), "pipe inside string must not offer filters: {items:?}");
}

#[test]
fn fq3s_cursor_in_raw_block_body_returns_empty() {
    // Inside `{% raw %}...{% endraw %}` content is literal text — nothing to complete.
    let src = "{% raw %}{{ x| }}{% endraw %}";
    let idx = extract(src);
    let ws = WorkspaceIndex::default();
    let col = src.find('|').unwrap() as u32 + 1;
    let (items, _) = complete(src, 0, col, &idx, &reg(), &ws);
    assert!(items.is_empty(), "cursor inside raw block must return empty: {items:?}");
}

#[test]
fn fq3s_import_alias_slot_returns_empty() {
    // `{% import "x" as ` — cursor in the alias identifier slot, not an expression.
    let src = r#"{% import "macros.html" as "#;
    let idx = extract(src);
    let ws = WorkspaceIndex::default();
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &ws);
    assert!(items.is_empty(), "import alias slot must return empty: {items:?}");
}

#[test]
fn fq3s_from_import_alias_slot_returns_empty() {
    // `{% from "x" import card as ` — cursor after `as`, in the alias slot.
    // Must return empty even when the source template has macros.
    let src = r#"{% from "macros.html" import card as "#;
    let idx = extract(src);
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("macros.html", "{% macro card(title) %}{% endmacro %}{% macro header() %}{% endmacro %}");
    let (items, _) = complete(src, 0, src.len() as u32, &idx, &reg(), &ws);
    assert!(items.is_empty(), "from-import alias slot must return empty: {items:?}");
}

// ─── jinja-lsp-n6su: attr completion data must resolve to docs ───────────────

#[test]
fn n6su_resolve_doc_handles_attr_prefix() {
    let reg = Registry::load_core();
    // "loop" is a built-in special object with attribute docs (e.g. loop.index).
    let doc = resolve_doc("attr:loop.index", &reg);
    assert!(doc.is_some(), "resolve_doc must handle 'attr:parent.attr' data keys");
}

// ─── jinja-lsp-7gxo: self. completes block names ─────────────────────────────

#[test]
fn cmp10_self_dot_offers_block_names_inside_block() {
    // When inside a block, `{{ self. }}` should offer all block names in this template.
    let src = "{% block content %}{{ self. }}{% endblock %}{% block sidebar %}{% endblock %}";
    let idx = extract(src);
    // position after "self." inside the block
    let col = src.find("self.").unwrap() as u32 + "self.".len() as u32;
    let (items, _) = complete(src, 0, col, &idx, &reg(), &WorkspaceIndex::default());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"content"), "self. must offer 'content' block: {labels:?}");
    assert!(labels.contains(&"sidebar"), "self. must offer 'sidebar' block: {labels:?}");
}

#[test]
fn n6su_attr_item_data_key_is_resolvable() {
    // Attribute completions for "loop" must carry a data key that resolve_doc can handle.
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let src = "{% for x in y %}{{ loop. }}{% endfor %}";
    let idx = extract(src);
    // position after "loop." — the dot triggers attribute completion
    let col = src.find("loop.").unwrap() as u32 + "loop.".len() as u32;
    let (items, _) = complete(src, 0, col, &idx, &reg, &ws);
    for item in &items {
        if let Some(data) = &item.data {
            let doc = resolve_doc(data, &reg);
            assert!(doc.is_some(), "attr item data '{data}' must resolve; got None");
        }
    }
}
