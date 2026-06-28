/// Byte-range + line-col span in a source file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Span {
    /// True when `other` is contained within this span.
    pub fn contains(&self, other: &Span) -> bool {
        self.start_byte <= other.start_byte && other.end_byte <= self.end_byte
    }
}

/// Recorded syntax error (JINJA-E001) from a partial parse (REQ-CONV-01).
#[derive(Debug, Clone)]
pub struct SyntaxError {
    pub span: Span,
}

/// REQ-DATA-01
#[derive(Debug, Clone)]
pub struct MacroDefinition {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub body: Span,
    pub span: Span,
}

/// A single macro parameter (positional; optional default makes it keyword-friendly).
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub default: Option<String>,
}

/// REQ-DATA-02
#[derive(Debug, Clone)]
pub struct BlockDefinition {
    pub name: String,
    pub scoped: bool,
    pub required: bool,
    pub body: Span,
    pub span: Span,
}

/// REQ-DATA-03
#[derive(Debug, Clone)]
pub struct VariableDefinition {
    pub name: String,
    pub scope: VariableScope,
    pub span: Span,
    pub valid_range: Span,
}

/// REQ-DATA-04
#[derive(Debug, Clone)]
pub struct ImportAlias {
    pub alias: String,
    pub source: String,
    pub span: Span,
}

/// REQ-DATA-04
#[derive(Debug, Clone)]
pub struct FromImport {
    pub source: String,
    pub names: Vec<ImportedName>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ImportedName {
    pub name: String,
    pub alias: Option<String>,
}

/// REQ-DATA-05
#[derive(Debug, Clone)]
pub struct TemplateReference {
    pub kind: TemplateRefKind,
    pub path: String,
    pub ignore_missing: bool,
    pub is_dynamic: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateRefKind {
    Extends,
    Include,
    Import,
    From,
}

/// REQ-DATA-06: a usage site of a name.
#[derive(Debug, Clone)]
pub struct Reference {
    pub name: String,
    pub kind: ReferenceKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReferenceKind {
    Identifier,
    Attribute,
    Filter,
    Function,
    Test,
}

/// REQ-DATA-07: exactly nine variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariableScope {
    Template,
    Block,
    ForLoop,
    Macro,
    With,
    CallBlock,
    Trans,
    Filter,
    Autoescape,
}
