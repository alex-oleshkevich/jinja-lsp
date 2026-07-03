// REQ-EXT-01..05: extension pack activation and embedded pack catalogs.

use crate::builtins::registry::{Registry, Source, parse_doc_str};

/// The four valid pack names accepted by the `extras` config key.
pub const KNOWN_PACK_NAMES: &[&str] = &["flask", "starlette", "starlette-babel", "starlette-flash"];

/// REQ-EXT-01: a validated pack name; unknown names produce `PackError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnownPack {
    Flask,
    Starlette,
    StarletteBabel,
    StarletteFlash, // starlette-flash
}

/// REQ-EXT-01: error type for an unrecognised extras name.
#[derive(Debug)]
pub enum PackError {
    UnknownPack(String),
}

impl KnownPack {
    pub fn parse(name: &str) -> Result<Self, PackError> {
        match name {
            "flask" => Ok(Self::Flask),
            "starlette" => Ok(Self::Starlette),
            "starlette-babel" => Ok(Self::StarletteBabel),
            "starlette-flash" => Ok(Self::StarletteFlash),
            other => Err(PackError::UnknownPack(other.to_owned())),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Flask => "flask",
            Self::Starlette => "starlette",
            Self::StarletteBabel => "starlette-babel",
            Self::StarletteFlash => "starlette-flash",
        }
    }

    fn docs(&self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Flask => FLASK_DOCS,
            Self::Starlette => STARLETTE_DOCS,
            Self::StarletteBabel => STARLETTE_BABEL_DOCS,
            Self::StarletteFlash => STARLETTE_FLASH_DOCS,
        }
    }
}

impl Registry {
    /// REQ-EXT-02, REQ-EXT-03: load docs for each active pack name, skip unknown.
    /// Returns the total number of docs successfully loaded.
    pub fn load_packs(&mut self, extras: &[&str]) -> usize {
        let mut loaded = 0;
        for &name in extras {
            let pack = match KnownPack::parse(name) {
                Ok(p) => p,
                Err(_) => continue, // unknown name — caller validates; we skip
            };
            let source = Source::Pack(name.to_owned());
            for (src_str, _path) in pack.docs() {
                if let Some((entry, attrs)) = parse_doc_str(src_str, source.clone()) {
                    self.insert(entry);
                    for attr in attrs {
                        self.insert_attr(attr);
                    }
                    loaded += 1;
                }
            }
        }
        loaded
    }
}

// ── Embedded pack docs ────────────────────────────────────────────────────────
// REQ-EXT-02/04: all 19 pack docs embedded via include_str!().

macro_rules! doc {
    ($path:literal) => {
        (include_str!(concat!("docs/", $path)), $path)
    };
}

static FLASK_DOCS: &[(&str, &str)] = &[
    doc!("flask/func_url_for.md"),
    doc!("flask/func_get_flashed_messages.md"),
    doc!("flask/var_request.md"),
    doc!("flask/var_session.md"),
    doc!("flask/var_g.md"),
    doc!("flask/var_config.md"),
];

static STARLETTE_DOCS: &[(&str, &str)] = &[
    doc!("starlette/func_url_for.md"),
    doc!("starlette/var_request.md"),
];

static STARLETTE_BABEL_DOCS: &[(&str, &str)] = &[
    doc!("starlette_babel/filter_date.md"),
    doc!("starlette_babel/filter_datetime.md"),
    doc!("starlette_babel/filter_time.md"),
    doc!("starlette_babel/filter_timedelta.md"),
    doc!("starlette_babel/filter_number.md"),
    doc!("starlette_babel/filter_currency.md"),
    doc!("starlette_babel/filter_percent.md"),
    doc!("starlette_babel/filter_scientific.md"),
    doc!("starlette_babel/func__.md"),
    doc!("starlette_babel/func__p.md"),
];

static STARLETTE_FLASH_DOCS: &[(&str, &str)] =
    &[doc!("starlette_flash/func_get_flashed_messages.md")];
