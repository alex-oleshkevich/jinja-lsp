// REQ-BLTN-01..07: unified builtin registry with four-source merge.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Filter,
    Function,
    Test,
    Variable,
    #[serde(rename = "context_variable")]
    ContextVariable,
}

/// REQ-BLTN-02: source priority — higher number wins on collision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Core,
    Custom,
    Pack(String),
    Hint,
}

impl Source {
    fn priority(&self) -> u8 {
        match self {
            Self::Core => 0,
            Self::Custom => 1,
            Self::Pack(_) => 2,
            Self::Hint => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Option<String>,
    pub default: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub struct DocEntry {
    pub name: String,
    pub category: Category,
    pub signature: Option<String>,
    pub since: Option<String>,
    pub params: Vec<Param>,
    pub body: String,
    pub source: Source,
    /// REQ-HINT-03: Python type name, informational (hints only).
    pub ty: Option<String>,
    /// REQ-HINT-03: template scope for hint entries; `None` means global.
    pub template: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AttrDoc {
    pub parent: String,
    pub attr: String,
    pub ty: Option<String>,
}

// ── Serde helpers for parsing YAML frontmatter ───────────────────────────────

#[derive(Deserialize)]
struct FrontmatterAttr {
    name: String,
    #[serde(rename = "type")]
    ty: Option<String>,
}

#[derive(Deserialize)]
struct FrontmatterParam {
    name: String,
    #[serde(rename = "type")]
    ty: Option<String>,
    default: Option<String>,
    #[serde(default)]
    required: bool,
}

#[derive(Deserialize)]
struct Frontmatter {
    name: String,
    category: Category,
    signature: Option<String>,
    since: Option<String>,
    /// REQ-HINT-03: Python type name (hints only).
    #[serde(rename = "type")]
    ty: Option<String>,
    /// REQ-HINT-03: template scope (hints only); absent = global.
    template: Option<String>,
    #[serde(default)]
    params: Vec<FrontmatterParam>,
    /// REQ-BLTN-05 / REQ-HINT-03: attribute list (loop.* or context_variable attrs).
    #[serde(default, alias = "attributes")]
    attrs: Vec<FrontmatterAttr>,
}

// ── Doc parsing ───────────────────────────────────────────────────────────────

/// REQ-BLTN-04: parse a `.md` doc string once; return `None` on malformed frontmatter.
/// Returns `(DocEntry, attribute docs)`. `serde_yaml` is used ONLY here — never for config.
pub fn parse_doc_str(src: &str, source: Source) -> Option<(DocEntry, Vec<AttrDoc>)> {
    let src = src.trim_start_matches('\u{feff}'); // strip BOM if present
    let rest = src.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let yaml = &rest[..end];
    let body_start = end + 4; // skip "\n---"
    let body = rest.get(body_start..)?.trim_start_matches('\n');

    let fm: Frontmatter = serde_yaml::from_str(yaml).ok()?;
    if fm.name.is_empty() {
        return None;
    }

    let attrs: Vec<AttrDoc> = fm
        .attrs
        .into_iter()
        .map(|a| AttrDoc {
            parent: fm.name.clone(),
            attr: a.name,
            ty: a.ty,
        })
        .collect();

    let entry = DocEntry {
        name: fm.name,
        category: fm.category,
        signature: fm.signature,
        since: fm.since,
        params: fm
            .params
            .into_iter()
            .map(|p| Param {
                name: p.name,
                ty: p.ty,
                default: p.default,
                required: p.required,
            })
            .collect(),
        body: body.to_owned(),
        source,
        ty: fm.ty,
        template: fm.template,
    };

    Some((entry, attrs))
}

// ── Registry ──────────────────────────────────────────────────────────────────

/// REQ-BLTN-01: registry keyed by (category, name).
pub struct Registry {
    entries: HashMap<(Category, String), DocEntry>,
    attributes: HashMap<(String, String), AttrDoc>,
}

impl Registry {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            attributes: HashMap::new(),
        }
    }

    /// REQ-BLTN-02: insert with priority merge — higher-priority source wins.
    pub fn insert(&mut self, entry: DocEntry) {
        let key = (entry.category, entry.name.clone());
        if let Some(existing) = self.entries.get(&key) {
            if existing.source.priority() >= entry.source.priority() {
                return; // existing wins
            }
        }
        self.entries.insert(key, entry);
    }

    pub(crate) fn insert_attr(&mut self, attr: AttrDoc) {
        self.attributes
            .insert((attr.parent.clone(), attr.attr.clone()), attr);
    }

    /// REQ-BLTN-01: exact lookup by (category, name).
    pub fn get(&self, category: Category, name: &str) -> Option<&DocEntry> {
        self.entries.get(&(category, name.to_owned()))
    }

    /// REQ-BLTN-01: scan by name across all categories.
    pub fn scan_by_name(&self, name: &str) -> Vec<&DocEntry> {
        self.entries
            .values()
            .filter(|e| e.name == name)
            .collect()
    }

    /// REQ-BLTN-05: attribute lookup by (parent, attr).
    pub fn get_attr(&self, parent: &str, attr: &str) -> Option<&AttrDoc> {
        self.attributes.get(&(parent.to_owned(), attr.to_owned()))
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn count_by_category(&self, category: Category) -> usize {
        self.entries.values().filter(|e| e.category == category).count()
    }

    /// Iterate all entries of a given category (order unspecified).
    pub fn iter_by_category(&self, category: Category) -> Vec<&DocEntry> {
        self.entries.values().filter(|e| e.category == category).collect()
    }

    /// All attribute docs whose parent matches `parent`.
    pub fn attrs_for(&self, parent: &str) -> Vec<&AttrDoc> {
        self.attributes.values().filter(|a| a.parent == parent).collect()
    }

    /// REQ-BLTN-06: load all 94 core embedded docs.
    pub fn load_core() -> Self {
        let mut reg = Self::new();
        for (src_str, _path) in CORE_DOCS {
            if let Some((entry, attrs)) = parse_doc_str(src_str, Source::Core) {
                reg.insert(entry);
                for attr in attrs {
                    reg.insert_attr(attr);
                }
            }
        }
        reg
    }

    /// REQ-BLTN-07: load custom builtins from a directory, non-fatally.
    pub fn load_custom_builtins(&mut self, dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&path) else {
                tracing::warn!("custom builtin: could not read {:?}", path);
                continue;
            };
            match parse_doc_str(&src, Source::Custom) {
                Some((doc, attrs)) => {
                    self.insert(doc);
                    for attr in attrs {
                        self.insert_attr(attr);
                    }
                }
                None => {
                    tracing::warn!("custom builtin: skipping malformed doc {:?}", path);
                }
            }
        }
    }
}

// ── Embedded core docs ────────────────────────────────────────────────────────
// REQ-BLTN-06: all 94 docs embedded via include_str!(), path preserved as tag.

macro_rules! doc {
    ($path:literal) => {
        (include_str!(concat!("docs/", $path)), $path)
    };
}

static CORE_DOCS: &[(&str, &str)] = &[
    // ── filters (50) ──────────────────────────────────────────────────────────
    doc!("jinja/filter_abs.md"),
    doc!("jinja/filter_attr.md"),
    doc!("jinja/filter_batch.md"),
    doc!("jinja/filter_capitalize.md"),
    doc!("jinja/filter_center.md"),
    doc!("jinja/filter_default.md"),
    doc!("jinja/filter_dictsort.md"),
    doc!("jinja/filter_escape.md"),
    doc!("jinja/filter_filesizeformat.md"),
    doc!("jinja/filter_first.md"),
    doc!("jinja/filter_float.md"),
    doc!("jinja/filter_forceescape.md"),
    doc!("jinja/filter_format.md"),
    doc!("jinja/filter_groupby.md"),
    doc!("jinja/filter_indent.md"),
    doc!("jinja/filter_int.md"),
    doc!("jinja/filter_join.md"),
    doc!("jinja/filter_last.md"),
    doc!("jinja/filter_length.md"),
    doc!("jinja/filter_list.md"),
    doc!("jinja/filter_lower.md"),
    doc!("jinja/filter_map.md"),
    doc!("jinja/filter_max.md"),
    doc!("jinja/filter_min.md"),
    doc!("jinja/filter_pprint.md"),
    doc!("jinja/filter_random.md"),
    doc!("jinja/filter_reject.md"),
    doc!("jinja/filter_rejectattr.md"),
    doc!("jinja/filter_replace.md"),
    doc!("jinja/filter_reverse.md"),
    doc!("jinja/filter_round.md"),
    doc!("jinja/filter_safe.md"),
    doc!("jinja/filter_select.md"),
    doc!("jinja/filter_selectattr.md"),
    doc!("jinja/filter_slice.md"),
    doc!("jinja/filter_sort.md"),
    doc!("jinja/filter_string.md"),
    doc!("jinja/filter_striptags.md"),
    doc!("jinja/filter_sum.md"),
    doc!("jinja/filter_title.md"),
    doc!("jinja/filter_tojson.md"),
    doc!("jinja/filter_trim.md"),
    doc!("jinja/filter_truncate.md"),
    doc!("jinja/filter_unique.md"),
    doc!("jinja/filter_upper.md"),
    doc!("jinja/filter_urlencode.md"),
    doc!("jinja/filter_urlize.md"),
    doc!("jinja/filter_wordcount.md"),
    doc!("jinja/filter_wordwrap.md"),
    doc!("jinja/filter_xmlattr.md"),
    // ── functions (8) ─────────────────────────────────────────────────────────
    doc!("jinja/func_cycler.md"),
    doc!("jinja/func_dict.md"),
    doc!("jinja/func_joiner.md"),
    doc!("jinja/func_lipsum.md"),
    doc!("jinja/func_namespace.md"),
    doc!("jinja/func_range.md"),
    doc!("jinja/func_super.md"),
    doc!("jinja/func_items.md"),
    // ── tests (31) ────────────────────────────────────────────────────────────
    doc!("jinja/test_boolean.md"),
    doc!("jinja/test_callable.md"),
    doc!("jinja/test_defined.md"),
    doc!("jinja/test_divisibleby.md"),
    doc!("jinja/test_eq.md"),
    doc!("jinja/test_equalto.md"),
    doc!("jinja/test_escaped.md"),
    doc!("jinja/test_even.md"),
    doc!("jinja/test_false.md"),
    doc!("jinja/test_float.md"),
    doc!("jinja/test_ge.md"),
    doc!("jinja/test_greaterthan.md"),
    doc!("jinja/test_gt.md"),
    doc!("jinja/test_in.md"),
    doc!("jinja/test_integer.md"),
    doc!("jinja/test_iterable.md"),
    doc!("jinja/test_le.md"),
    doc!("jinja/test_lessthan.md"),
    doc!("jinja/test_lower.md"),
    doc!("jinja/test_lt.md"),
    doc!("jinja/test_mapping.md"),
    doc!("jinja/test_ne.md"),
    doc!("jinja/test_none.md"),
    doc!("jinja/test_number.md"),
    doc!("jinja/test_odd.md"),
    doc!("jinja/test_sameas.md"),
    doc!("jinja/test_sequence.md"),
    doc!("jinja/test_string.md"),
    doc!("jinja/test_true.md"),
    doc!("jinja/test_undefined.md"),
    doc!("jinja/test_upper.md"),
    // ── variables (5) ─────────────────────────────────────────────────────────
    doc!("jinja/var_caller.md"),
    doc!("jinja/var_kwargs.md"),
    doc!("jinja/var_loop.md"),
    doc!("jinja/var_self.md"),
    doc!("jinja/var_varargs.md"),
];
