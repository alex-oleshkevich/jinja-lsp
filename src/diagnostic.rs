// REQ-TEST-04: the canonical diagnostic shape — identical across check CLI,
// LSP publishDiagnostics, and expected-diagnostics.json golden files.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub code: String,
    pub slug: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl DiagnosticSeverity {
    /// ADR-003: derive severity from the code string prefix letter.
    /// "JINJA-E###" → Error, "JINJA-W###" → Warning, "JINJA-I###" → Info,
    /// "JINJA-H###" → Hint. Unknown prefixes default to Error.
    pub fn from_code_str(code: &str) -> Self {
        match code.strip_prefix("JINJA-").and_then(|s| s.chars().next()) {
            Some('E') => Self::Error,
            Some('W') => Self::Warning,
            Some('I') => Self::Info,
            Some('H') => Self::Hint,
            _ => Self::Error,
        }
    }
}
