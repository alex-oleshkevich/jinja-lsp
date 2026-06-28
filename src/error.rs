/// Tree-sitter wrapper failed to obtain a tree (rare). Record JINJA-E001, continue.
#[derive(Debug)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// A query or symbol-extraction step failed for one node. Log at `warn`, skip the node.
#[derive(Debug)]
pub struct ExtractionError {
    message: String,
}

impl ExtractionError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Config parsing or validation failed. Surface as workspace diagnostic; retain prior config.
#[derive(Debug)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// A check could not complete. Non-fatal: skip that code, run the rest.
#[derive(Debug)]
pub struct DiagnosticError {
    message: String,
}

impl DiagnosticError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
    pub fn message(&self) -> &str {
        &self.message
    }
}
