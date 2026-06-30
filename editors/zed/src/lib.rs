// REQ-EDIT-07/08/10/12: Zed extension crate for jinja-lsp.
// Registers the tree-sitter-jinja grammar and the jinja-lsp language server.
// Language-server id: jinja2-lsp, language: Jinja2 (HTML) — ported from legacy .zed/settings.json.

use zed_extension_api::{self as zed, settings::LspSettings, serde_json, Architecture, LanguageServerId, Os, Result};

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
                args: binary.arguments.unwrap_or_else(|| vec!["lsp".to_owned()]),
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
        let release_binary = download_release(language_server_id)?;
        Ok(zed::Command {
            command: release_binary,
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

/// File name (in the extension working dir) where we cache the last installed version string.
const INSTALLED_VERSION_FILE: &str = "installed_version";

/// REQ-EDIT-12: return the path to the jinja-lsp binary, downloading it if needed.
///
/// On every launch the cached version (if any) is checked first. A GitHub API call is
/// made only when no locally-cached binary is found, avoiding redundant network traffic.
fn download_release(language_server_id: &LanguageServerId) -> Result<String> {
    let (os, arch) = zed::current_platform();
    let (target, archive_ext, file_type, binary_name) = platform_info(os, arch)?;

    // Fast path: if we already have a cached binary, return it immediately.
    if let Ok(version) = std::fs::read_to_string(INSTALLED_VERSION_FILE) {
        let version = version.trim();
        if !version.is_empty() {
            let binary_path = format!("jinja-lsp-{version}/{binary_name}");
            if std::path::Path::new(&binary_path).exists() {
                return Ok(binary_path);
            }
        }
    }

    // Slow path: cached binary missing — fetch latest release from GitHub and download.
    zed::set_language_server_installation_status(
        language_server_id,
        &zed::LanguageServerInstallationStatus::CheckingForUpdate,
    );

    let release = zed::latest_github_release(
        "alex-oleshkevich/jinja-lsp",
        zed::GithubReleaseOptions {
            require_assets: true,
            pre_release: false,
        },
    )?;

    // REQ-REL-05: archives are named jinja-lsp-vX.Y.Z-<target>.<ext> (version-prefixed).
    let archive_name = format!("jinja-lsp-{}-{target}.{archive_ext}", release.version);

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == archive_name)
        .ok_or_else(|| format!("jinja-lsp release asset '{archive_name}' not found"))?;

    // REQ-EDIT-12: fetch the binary checksum published by F21 (release.yml).
    // The `.binary.sha256` asset contains the SHA256 hex of the extracted binary (not the archive).
    let checksum_asset_name = format!("{archive_name}.binary.sha256");
    let checksum_asset = release
        .assets
        .iter()
        .find(|a| a.name == checksum_asset_name)
        .ok_or_else(|| format!("checksum asset '{checksum_asset_name}' not found in release"))?;

    zed::set_language_server_installation_status(
        language_server_id,
        &zed::LanguageServerInstallationStatus::Downloading,
    );

    // Download the binary checksum (plain text hex, no compression).
    let checksum_file = format!("{archive_name}.binary.sha256.txt");
    zed::download_file(&checksum_asset.download_url, &checksum_file, zed::DownloadedFileType::Uncompressed)?;
    let expected_hex = std::fs::read_to_string(&checksum_file)
        .map_err(|e| format!("failed to read checksum file: {e}"))?;

    // Download and extract the archive into a versioned directory.
    let install_dir = format!("jinja-lsp-{}", release.version);
    zed::download_file(&asset.download_url, &install_dir, file_type)?;

    let binary_path = format!("{install_dir}/{binary_name}");
    zed::make_file_executable(&binary_path)?;

    // REQ-EDIT-12: verify the extracted binary against the published checksum.
    // A mismatch means the download is corrupt or tampered — reject it.
    verify_binary_checksum(&binary_path, expected_hex.trim())?;

    // Persist the installed version so future launches can skip the GitHub API call.
    let _ = std::fs::write(INSTALLED_VERSION_FILE, &release.version);

    Ok(binary_path)
}

/// Compute SHA-256 of the binary at `path` and compare against the hex string in `expected`.
/// `expected` may be bare hex or in `sha256sum` format (`<hex>  <filename>`).
fn verify_binary_checksum(path: &str, expected: &str) -> Result<()> {
    use sha2::{Digest, Sha256};

    let bytes = std::fs::read(path)
        .map_err(|e| format!("failed to read binary for checksum verification: {e}"))?;

    let digest = Sha256::digest(&bytes);
    let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();

    // Accept both bare hex and `sha256sum`-style lines.
    let expected_hex = expected.split_whitespace().next().unwrap_or("").to_lowercase();

    if actual != expected_hex {
        return Err(format!(
            "checksum mismatch for {path}: expected {expected_hex}, got {actual}"
        ));
    }
    Ok(())
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
