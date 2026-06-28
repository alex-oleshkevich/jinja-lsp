// One module per diagnostic check (REQ-FOLD-04).
pub mod checks;

pub fn layer_name() -> &'static str {
    "diagnostics"
}
