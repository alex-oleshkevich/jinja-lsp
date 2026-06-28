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
