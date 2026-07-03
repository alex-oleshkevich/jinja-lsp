// REQ-CFG-01..11: config discovery, parsing, validation, overlay.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::format::FormatterConfig;

const KNOWN_EXTRAS: &[&str] = &["flask", "starlette", "starlette-babel", "starlette-flash"];

/// The full resolved config, built from defaults + file + overlay.
#[derive(Debug, Clone, PartialEq)]
pub struct JinjaConfig {
    /// Raw templates list as written in the config (may contain "...").
    pub templates_raw: Vec<String>,
    pub extensions: Vec<String>,
    pub extras: Vec<String>,
    pub custom_builtins: Vec<String>,
    pub hints: Vec<String>,
    pub inline_patterns: Vec<String>,
    pub lint: LintConfig,
    /// Formatter behaviour (jinja.toml `[format]` section).
    pub format: FormatterConfig,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LintConfig {
    pub select: Vec<String>,
    pub ignore: Vec<String>,
}

impl Default for JinjaConfig {
    fn default() -> Self {
        Self {
            templates_raw: vec!["...".to_owned()],
            extensions: vec![
                "html".to_owned(),
                "jinja".to_owned(),
                "jinja2".to_owned(),
                "j2".to_owned(),
            ],
            extras: vec![],
            custom_builtins: vec![],
            hints: vec![],
            inline_patterns: vec!["render_template_string".to_owned()],
            lint: LintConfig::default(),
            format: FormatterConfig::default(),
        }
    }
}

impl JinjaConfig {
    /// REQ-CFG-01: walk up from `root` looking for jinja.toml, then pyproject.toml.
    pub fn discover(root: &Path) -> Result<Self, ConfigError> {
        Ok(Self::discover_with_path(root)?.0)
    }

    /// Load config from an explicit file path (must be a jinja.toml file).
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;
        Self::from_jinja_toml(&raw)
    }

    /// Like `discover`, but also returns the path of the config file that was found.
    /// Returns `(config, Some(path))` when a file was found, or `(defaults, None)`.
    pub fn discover_with_path(
        root: &Path,
    ) -> Result<(Self, Option<std::path::PathBuf>), ConfigError> {
        let mut dir = root.to_owned();
        loop {
            // 1. jinja.toml
            let jinja_toml = dir.join("jinja.toml");
            if jinja_toml.is_file() {
                let raw =
                    fs::read_to_string(&jinja_toml).map_err(|e| ConfigError::Io(e.to_string()))?;
                return Ok((Self::from_jinja_toml(&raw)?, Some(jinja_toml)));
            }
            // 2. pyproject.toml with [tool.jinja]
            let pyproject = dir.join("pyproject.toml");
            if pyproject.is_file() {
                let raw =
                    fs::read_to_string(&pyproject).map_err(|e| ConfigError::Io(e.to_string()))?;
                if let Some(cfg) = Self::from_pyproject(&raw)? {
                    return Ok((cfg, Some(pyproject)));
                }
            }
            match dir.parent() {
                Some(p) => dir = p.to_owned(),
                None => break,
            }
        }
        // no config found — return zero-config defaults
        Ok((Self::default(), None))
    }

    fn from_jinja_toml(raw: &str) -> Result<Self, ConfigError> {
        let table: TomlConfig =
            toml::from_str(raw).map_err(|e| ConfigError::Parse(e.to_string()))?;
        Ok(table.into_config())
    }

    fn from_pyproject(raw: &str) -> Result<Option<Self>, ConfigError> {
        let doc: toml::Value =
            toml::from_str(raw).map_err(|e| ConfigError::Parse(e.to_string()))?;
        let table = doc.get("tool").and_then(|t| t.get("jinja")).cloned();
        match table {
            None => Ok(None),
            Some(v) => {
                let tc: TomlConfig = v
                    .try_into()
                    .map_err(|e: toml::de::Error| ConfigError::Parse(e.to_string()))?;
                Ok(Some(tc.into_config()))
            }
        }
    }

    /// REQ-CFG-02: auto-discover template dirs relative to `root` when no explicit list.
    pub fn zero_config_dirs(root: &Path) -> Vec<PathBuf> {
        let candidates = ["templates", "jinja", "j2"];
        let mut dirs = vec![];
        for name in candidates {
            let d = root.join(name);
            if d.is_dir() {
                dirs.push(d);
            }
        }
        // <project-name>/templates — read from pyproject.toml [project].name / [tool.poetry].name
        if let Some(project_name) = Self::project_name_from_pyproject(root) {
            let d = root.join(&project_name).join("templates");
            if d.is_dir() {
                dirs.push(d);
            }
        }
        dirs
    }

    fn project_name_from_pyproject(root: &Path) -> Option<String> {
        let raw = fs::read_to_string(root.join("pyproject.toml")).ok()?;
        let doc: toml::Value = toml::from_str(&raw).ok()?;
        doc.get("project")
            .and_then(|p| p.get("name"))
            .or_else(|| {
                doc.get("tool")
                    .and_then(|t| t.get("poetry"))
                    .and_then(|p| p.get("name"))
            })
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
    }

    /// REQ-CFG-03: expand "..." sentinel and return resolved absolute paths.
    pub fn resolved_template_dirs(&self, root: &Path) -> Vec<PathBuf> {
        let auto = Self::zero_config_dirs(root);
        let mut out = vec![];
        for entry in &self.templates_raw {
            if entry == "..." {
                out.extend_from_slice(&auto);
            } else {
                let d = root.join(entry);
                if d.is_dir() {
                    out.push(d);
                }
            }
        }
        out
    }

    /// REQ-CFG-07: validate the config; return warnings (non-fatal) via the Ok variant.
    pub fn validate(&self) -> Result<Vec<ConfigWarning>, ConfigError> {
        let mut warnings = vec![];

        // unknown extras names
        for extra in &self.extras {
            if !KNOWN_EXTRAS.contains(&extra.as_str()) {
                return Err(ConfigError::UnknownExtra(extra.clone()));
            }
        }

        // lint filter: only JINJA-ENNN, JINJA-W, class-prefix, or JINJA-* patterns
        for code in self.lint.select.iter().chain(self.lint.ignore.iter()) {
            if !is_valid_lint_filter(code) {
                return Err(ConfigError::InvalidLintFilter(code.clone()));
            }
        }

        // overlapping select/ignore → warning, ignore wins
        for code in &self.lint.ignore {
            if self.lint.select.iter().any(|s| code_matches(s, code)) {
                warnings.push(ConfigWarning::OverlappingFilter(code.clone()));
            }
        }

        Ok(warnings)
    }

    /// REQ-CFG-11: apply editor-supplied overlay on top of this config (per-key).
    pub fn apply_overlay(&mut self, overlay: ConfigOverlay) {
        if let Some(v) = overlay.extensions {
            self.extensions = v;
        }
        if let Some(v) = overlay.extras {
            self.extras = v;
        }
        if let Some(v) = overlay.custom_builtins {
            self.custom_builtins = v;
        }
        if let Some(v) = overlay.hints {
            self.hints = v;
        }
        if let Some(v) = overlay.inline_patterns {
            self.inline_patterns = v;
        }
        if let Some(v) = overlay.templates {
            self.templates_raw = v;
        }
        if let Some(lint) = overlay.lint {
            if let Some(s) = lint.select {
                self.lint.select = s;
            }
            if let Some(i) = lint.ignore {
                self.lint.ignore = i;
            }
        }
    }
}

fn is_valid_lint_filter(code: &str) -> bool {
    // Valid: "JINJA-E", "JINJA-W", "JINJA-E1", "JINJA-W203", etc.
    // Invalid: "JINJA-" alone, "JINJA-zzz", or slugs like "unused-variable".
    let rest = match code.strip_prefix("JINJA-") {
        Some(r) => r,
        None => return false,
    };
    let mut chars = rest.chars();
    match chars.next() {
        Some('E') | Some('W') => chars.all(|c| c.is_ascii_digit()),
        _ => false,
    }
}

fn code_matches(filter: &str, code: &str) -> bool {
    code.starts_with(filter)
}

// ---------- Internal TOML deserialization shape ----------------------------

#[derive(Debug, Deserialize, Default)]
struct TomlConfig {
    #[serde(default)]
    templates: Option<Vec<String>>,
    #[serde(default)]
    extensions: Option<Vec<String>>,
    #[serde(default)]
    extras: Option<Vec<String>>,
    #[serde(default)]
    custom_builtins: Option<Vec<String>>,
    #[serde(default)]
    hints: Option<Vec<String>>,
    #[serde(default)]
    inline_patterns: Option<Vec<String>>,
    #[serde(default)]
    lint: Option<TomlLint>,
    #[serde(default)]
    format: Option<FormatterConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct TomlLint {
    #[serde(default)]
    select: Option<Vec<String>>,
    #[serde(default)]
    ignore: Option<Vec<String>>,
}

impl TomlConfig {
    fn into_config(self) -> JinjaConfig {
        let defaults = JinjaConfig::default();
        JinjaConfig {
            templates_raw: self.templates.unwrap_or(defaults.templates_raw),
            extensions: self.extensions.unwrap_or(defaults.extensions),
            extras: self.extras.unwrap_or_default(),
            custom_builtins: self.custom_builtins.unwrap_or_default(),
            hints: self.hints.unwrap_or_default(),
            inline_patterns: self.inline_patterns.unwrap_or(defaults.inline_patterns),
            lint: match self.lint {
                None => LintConfig::default(),
                Some(l) => LintConfig {
                    select: l.select.unwrap_or_default(),
                    ignore: l.ignore.unwrap_or_default(),
                },
            },
            format: self.format.unwrap_or_default(),
        }
    }
}

// ---------- ConfigOverlay (for initializationOptions / didChangeConfiguration) --

/// REQ-CFG-11: mirrors the key set exactly; each field is optional so partial
/// overlays only override the keys they mention.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct ConfigOverlay {
    pub templates: Option<Vec<String>>,
    pub extensions: Option<Vec<String>>,
    pub extras: Option<Vec<String>>,
    pub custom_builtins: Option<Vec<String>>,
    pub hints: Option<Vec<String>>,
    pub inline_patterns: Option<Vec<String>>,
    pub lint: Option<LintOverlay>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct LintOverlay {
    pub select: Option<Vec<String>>,
    pub ignore: Option<Vec<String>>,
}

impl ConfigOverlay {
    /// jinja-lsp-isj4: true when no field carries a value. Since every field is
    /// `Option` and unrecognized JSON keys are silently ignored by serde, ANY JSON
    /// object unrelated to jinja-lsp (including `{}`) deserializes into an overlay
    /// where this is true — the caller must treat that as "no jinja-lsp-relevant
    /// settings changed" rather than "the user cleared every setting".
    pub fn is_empty(&self) -> bool {
        self.templates.is_none()
            && self.extensions.is_none()
            && self.extras.is_none()
            && self.custom_builtins.is_none()
            && self.hints.is_none()
            && self.inline_patterns.is_none()
            && self.lint.is_none()
    }
}

// ---------- Errors & warnings -----------------------------------------------

#[derive(Debug, PartialEq)]
pub enum ConfigError {
    Io(String),
    Parse(String),
    UnknownExtra(String),
    InvalidLintFilter(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(s) => write!(f, "config I/O error: {s}"),
            Self::Parse(s) => write!(f, "config parse error: {s}"),
            Self::UnknownExtra(s) => write!(f, "unknown extras name: {s}"),
            Self::InvalidLintFilter(s) => write!(f, "invalid lint filter: {s}"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ConfigWarning {
    OverlappingFilter(String),
}
