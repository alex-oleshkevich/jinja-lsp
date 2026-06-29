// REQ-EDIT-07/08/12: Zed extension crate for jinja-lsp.
// Registers the tree-sitter-jinja grammar and the jinja-lsp language server.
// Language-server id: jinja2-lsp, language: Jinja2 (HTML) — ported from legacy .zed/settings.json.

use zed_extension_api::{self as zed, settings::LspSettings, LanguageServerId, Result};

struct JinjaLspExtension;

impl zed::Extension for JinjaLspExtension {
    fn new() -> Self {
        JinjaLspExtension
    }

    /// REQ-EDIT-07/08: return the jinja-lsp lsp command over stdio.
    /// REQ-EDIT-12: when jinja-lsp is not on PATH, download and checksum-verify
    ///              the release binary before returning it.
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
                command: binary.path.unwrap_or_else(|| "jinja-lsp".to_owned()),
                args: vec!["lsp".to_owned()],
                env: Default::default(),
            });
        }

        // Prefer jinja-lsp on PATH.
        if let Some(path) = worktree.which("jinja-lsp") {
            return Ok(zed::Command {
                command: path,
                args: vec!["lsp".to_owned()],
                env: Default::default(),
            });
        }

        // REQ-EDIT-12: not on PATH — download the release binary.
        let release_binary = download_and_verify_release()?;
        Ok(zed::Command {
            command: release_binary,
            args: vec!["lsp".to_owned()],
            env: Default::default(),
        })
    }
}

/// REQ-EDIT-12: download the jinja-lsp release binary from GitHub and verify
/// its published checksum before returning the local path.
///
/// Rejects the binary (returns Err) if the checksum does not match.
fn download_and_verify_release() -> Result<String> {
    let release = zed::latest_github_release(
        "alex-oleshkevich/jinja-lsp",
        zed::GithubReleaseOptions {
            require_assets: true,
            pre_release: false,
        },
    )?;

    let (asset_name, checksum_name) = release_asset_names();

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| format!("jinja-lsp release asset '{asset_name}' not found"))?;

    let checksum_asset = release
        .assets
        .iter()
        .find(|a| a.name == checksum_name)
        .ok_or_else(|| format!("jinja-lsp checksum asset '{checksum_name}' not found"))?;

    // Download the binary.
    let binary_path = zed::download_file(
        &asset.download_url,
        &format!("jinja-lsp-{}", release.version),
        zed::DownloadedFileType::Uncompressed,
    )?;

    // Download and verify the published checksum (REQ-EDIT-12: single source of truth = F21).
    let checksum_content = zed::download_file(
        &checksum_asset.download_url,
        &format!("jinja-lsp-{}.sha256", release.version),
        zed::DownloadedFileType::Uncompressed,
    )?;

    zed::verify_file_against_checksum(&binary_path, &checksum_content)?;

    zed::make_file_executable(&binary_path)?;
    Ok(binary_path)
}

fn release_asset_names() -> (String, String) {
    let os = zed::current_platform().os.to_string();
    let arch = zed::current_platform().arch.to_string();
    let ext = if os == "windows" { ".exe" } else { "" };
    let asset = format!("jinja-lsp-{arch}-{os}{ext}");
    let checksum = format!("{asset}.sha256");
    (asset, checksum)
}

zed::register_extension!(JinjaLspExtension);
