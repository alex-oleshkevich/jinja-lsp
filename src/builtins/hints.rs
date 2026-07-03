// REQ-HINT-01..08: user hint discovery and loading.

use std::path::{Path, PathBuf};

use crate::builtins::registry::{Registry, Source, parse_doc_str};

/// REQ-HINT-01: return the sidecar path for a template, if it exists.
/// A sidecar is `<template_path>.hints.md` in the same directory.
pub fn find_sidecar(template_path: &Path) -> Option<PathBuf> {
    let mut sidecar = template_path.to_owned();
    let name = sidecar.file_name()?.to_string_lossy().to_string();
    sidecar.set_file_name(format!("{name}.hints.md"));
    sidecar.exists().then_some(sidecar)
}

/// REQ-HINT-01: load the sidecar hint beside `template_path` into `registry`, if present.
pub fn load_sidecar(template_path: &Path, registry: &mut Registry) {
    if let Some(path) = find_sidecar(template_path) {
        load_hint_file(&path, registry);
    }
}

/// REQ-HINT-02: scan every `.md` file in `dir`, loading each as a hint.
/// Malformed files are logged and skipped (REQ-HINT-08).
impl Registry {
    pub fn load_hints_from_dir(&mut self, dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            load_hint_file(&path, self);
        }
    }
}

fn load_hint_file(path: &Path, registry: &mut Registry) {
    let Ok(src) = std::fs::read_to_string(path) else {
        tracing::warn!("hint: could not read {:?}", path);
        return;
    };
    match parse_doc_str(&src, Source::Hint) {
        Some((entry, attrs)) => {
            registry.insert(entry);
            for attr in attrs {
                registry.insert_attr(attr);
            }
        }
        None => {
            tracing::warn!("hint: skipping malformed {:?}", path);
        }
    }
}
