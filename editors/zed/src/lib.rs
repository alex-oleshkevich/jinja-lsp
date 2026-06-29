// REQ-EDIT-07/08/12: Zed extension crate for jinja-lsp.
// Registers the tree-sitter-jinja grammar and the jinja-lsp language server.
// Language-server id: jinja2-lsp, language: Jinja2 (HTML) — ported from legacy .zed/settings.json.

use zed_extension_api::{self as zed, settings::LspSettings, Architecture, LanguageServerId, Os, Result};

struct JinjaLspExtension;

impl zed::Extension for JinjaLspExtension {
    fn new() -> Self {
        JinjaLspExtension
    }

    /// REQ-EDIT-07/08: return the jinja-lsp lsp command over stdio.
    /// REQ-EDIT-12: when jinja-lsp is not on PATH, download the release binary.
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
        let release_binary = download_release()?;
        Ok(zed::Command {
            command: release_binary,
            args: vec!["lsp".to_owned()],
            env: Default::default(),
        })
    }
}

/// REQ-EDIT-12: download the jinja-lsp release binary from GitHub.
fn download_release() -> Result<String> {
    let release = zed::latest_github_release(
        "alex-oleshkevich/jinja-lsp",
        zed::GithubReleaseOptions {
            require_assets: true,
            pre_release: false,
        },
    )?;

    let (os, arch) = zed::current_platform();
    let (target, archive_ext, file_type, binary_name) = platform_info(os, arch)?;

    let archive_name = format!("jinja-lsp-{target}.{archive_ext}");

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == archive_name)
        .ok_or_else(|| format!("jinja-lsp release asset '{archive_name}' not found"))?;

    // Download and extract the archive; returns the extraction directory.
    let install_dir = zed::download_file(
        &asset.download_url,
        &format!("jinja-lsp-{}", release.version),
        file_type,
    )?;

    let binary_path = format!("{install_dir}/{binary_name}");
    zed::make_file_executable(&binary_path)?;
    Ok(binary_path)
}

/// Map the current platform to (rust-target-triple, archive-ext, download-type, binary-name).
fn platform_info(
    os: Os,
    arch: Architecture,
) -> Result<(&'static str, &'static str, zed::DownloadedFileType, &'static str)> {
    match (os, arch) {
        (Os::Linux, Architecture::Aarch64) => Ok((
            "aarch64-unknown-linux-gnu",
            "tar.gz",
            zed::DownloadedFileType::GzipTar,
            "jinja-lsp",
        )),
        (Os::Linux, Architecture::X8664) => Ok((
            "x86_64-unknown-linux-gnu",
            "tar.gz",
            zed::DownloadedFileType::GzipTar,
            "jinja-lsp",
        )),
        (Os::Mac, Architecture::Aarch64) => Ok((
            "aarch64-apple-darwin",
            "tar.gz",
            zed::DownloadedFileType::GzipTar,
            "jinja-lsp",
        )),
        (Os::Windows, Architecture::X8664) => Ok((
            "x86_64-pc-windows-msvc",
            "zip",
            zed::DownloadedFileType::Zip,
            "jinja-lsp.exe",
        )),
        _ => Err("unsupported platform for jinja-lsp release binary".to_string()),
    }
}

zed::register_extension!(JinjaLspExtension);
