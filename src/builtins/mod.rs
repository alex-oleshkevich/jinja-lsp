// Unified builtin registry, embedded-doc loader, extension packs,
// custom-builtins disk loader, and user-hint loader (REQ-FOLD-05).

pub mod packs;
pub mod registry;

pub fn layer_name() -> &'static str {
    "builtins"
}
