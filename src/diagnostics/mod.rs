// REQ-DIAG-01..06: diagnostic catalog, noqa suppression, select/ignore filter.

pub mod checks;

pub use filter::filter_by_config;
pub use noqa::{NoqaDirective, parse_noqa_directives, suppress_by_noqa};

mod filter;
mod noqa;

use crate::diagnostic::DiagnosticSeverity;

/// REQ-DIAG-01: every diagnostic has a stable kebab-case slug.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagCode {
    // Pass 1 — per-file
    E001, // syntax-error
    E102, // undefined-filter
    E104, // undefined-test
    W201, // unused-variable
    W301, // duplicate-block
    W302, // duplicate-macro
    W303, // duplicate-import-alias
    W304, // duplicate-from-import
    W305, // name-shadowing
    W106, // unknown-attribute (REQ-HINT-05: off by default, hint-gated)
    W107, // invalid-noqa
    // Pass 2 — cross-file
    E101, // undefined-variable
    E103, // undefined-function
    W202, // unused-macro
    W203, // unused-import
    E401, // invalid-super
    W402, // unreachable-content
    E403, // missing-required-block
    E404, // recursive-import
    E501, // wrong-call-args
    E601, // template-does-not-exist
}

impl DiagCode {
    /// jinja-lsp-rm5r: every variant, kept next to the enum so adding a new
    /// code and forgetting to list it here is a one-line diff to catch in
    /// review — the single source noqa's known-codes list derives from,
    /// instead of a hand-duplicated string array in a different file.
    pub const ALL: &'static [DiagCode] = &[
        Self::E001,
        Self::E102,
        Self::E104,
        Self::W201,
        Self::W301,
        Self::W302,
        Self::W303,
        Self::W304,
        Self::W305,
        Self::W106,
        Self::W107,
        Self::E101,
        Self::E103,
        Self::W202,
        Self::W203,
        Self::E401,
        Self::W402,
        Self::E403,
        Self::E404,
        Self::E501,
        Self::E601,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::E001 => "syntax-error",
            Self::E101 => "undefined-variable",
            Self::E102 => "undefined-filter",
            Self::E103 => "undefined-function",
            Self::E104 => "undefined-test",
            Self::W106 => "unknown-attribute",
            Self::W107 => "invalid-noqa",
            Self::W201 => "unused-variable",
            Self::W202 => "unused-macro",
            Self::W203 => "unused-import",
            Self::W301 => "duplicate-block",
            Self::W302 => "duplicate-macro",
            Self::W303 => "duplicate-import-alias",
            Self::W304 => "duplicate-from-import",
            Self::W305 => "name-shadowing",
            Self::E401 => "invalid-super",
            Self::W402 => "unreachable-content",
            Self::E403 => "missing-required-block",
            Self::E404 => "recursive-import",
            Self::E501 => "wrong-call-args",
            Self::E601 => "template-does-not-exist",
        }
    }

    /// ADR-003: derive severity from the code_str() prefix letter.
    pub fn severity(self) -> DiagnosticSeverity {
        DiagnosticSeverity::from_code_str(self.code_str())
    }

    pub fn code_str(self) -> &'static str {
        match self {
            Self::E001 => "JINJA-E001",
            Self::E101 => "JINJA-E101",
            Self::E102 => "JINJA-E102",
            Self::E103 => "JINJA-E103",
            Self::E104 => "JINJA-E104",
            Self::W106 => "JINJA-W106",
            Self::W107 => "JINJA-W107",
            Self::W201 => "JINJA-W201",
            Self::W202 => "JINJA-W202",
            Self::W203 => "JINJA-W203",
            Self::W301 => "JINJA-W301",
            Self::W302 => "JINJA-W302",
            Self::W303 => "JINJA-W303",
            Self::W304 => "JINJA-W304",
            Self::W305 => "JINJA-W305",
            Self::E401 => "JINJA-E401",
            Self::W402 => "JINJA-W402",
            Self::E403 => "JINJA-E403",
            Self::E404 => "JINJA-E404",
            Self::E501 => "JINJA-E501",
            Self::E601 => "JINJA-E601",
        }
    }
}

pub fn layer_name() -> &'static str {
    "diagnostics"
}
