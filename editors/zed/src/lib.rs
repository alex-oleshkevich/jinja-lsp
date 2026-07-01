// REQ-EDIT-07/08/10: Zed extension crate for jinja-lsp.
// Registers the tree-sitter-jinja grammar and the jinja-lsp language server.
// Language-server id: jinja2-lsp, language: Jinja2 (HTML) — ported from legacy .zed/settings.json.

use zed_extension_api::{self as zed, serde_json, settings::LspSettings, LanguageServerId, Result};

const SERVER_NAME: &str = "jinja-lsp";

struct JinjaLspExtension;

impl zed::Extension for JinjaLspExtension {
    fn new() -> Self {
        JinjaLspExtension
    }

    /// REQ-EDIT-07/08: return the jinja-lsp lsp command over stdio.
    /// The binary is never downloaded — the user must install it manually.
    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|s| s.binary);

        // If the user has explicitly set a binary path, use it.
        if let Some(binary) = settings {
            return Ok(zed::Command {
                command: binary.path.unwrap_or_else(|| SERVER_NAME.to_owned()),
                args: binary.arguments.unwrap_or_else(|| vec!["lsp".to_owned()]),
                env: Default::default(),
            });
        }

        // Otherwise require jinja-lsp on PATH; never download it.
        let binary = worktree
            .which(SERVER_NAME)
            .ok_or_else(binary_not_found_message)?;
        Ok(zed::Command {
            command: binary,
            args: vec!["lsp".to_owned()],
            env: Default::default(),
        })
    }

    /// REQ-EDIT-08/10: forward lsp.jinja2-lsp.initialization_options from Zed settings as
    /// the server's InitializationOptions, overlaid on the config file per REQ-CFG-11.
    fn language_server_initialization_options(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|s| s.initialization_options);
        Ok(settings)
    }
}

/// Message shown in Zed's LSP logs when the server binary is missing. Kept identical
/// across all alex-oleshkevich LSP extensions (see the lsp-maker skill). These
/// extensions never download the binary — the user installs it manually.
fn binary_not_found_message() -> String {
    format!(
        "{SERVER_NAME} was not found on your PATH.\n\
         This extension does not download it — you must install it manually.\n\
         Repository: https://github.com/alex-oleshkevich/{SERVER_NAME}\n\
         Releases:   https://github.com/alex-oleshkevich/{SERVER_NAME}/releases"
    )
}

zed::register_extension!(JinjaLspExtension);
