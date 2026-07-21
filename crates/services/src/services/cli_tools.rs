//! App-managed CLI tool installer.
//!
//! Installs a curated, version-pinned catalog of vendor CLI tools (aws, az,
//! op, gam, mgc-beta, acli, gws) into the single app-owned directory
//! `assets::cli_tools_dir()`. Only `cli-tools/bin` is ever exposed on spawned
//! agents' PATH, and it is appended after host paths so a host-provided copy
//! (e.g. from nix) always wins over an app-installed one.
//!
//! Install is atomic with respect to agents: download + extraction happen
//! under `.staging/`, the version directory is renamed into place, and the
//! `bin/` symlink is swapped last (symlink + rename). A crash at any point
//! leaves either the previous install fully working or no symlink at all —
//! never a half-written binary on PATH.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, OwnedMutexGuard};
use ts_rs::TS;
use utils::{assets::cli_tools_dir, shell::resolve_executable_path};

// Version pins. Bumping a version REQUIRES refreshing the matching
// per-platform sha256 pins below — a stale hash makes installs fail loudly
// (nothing is written), it never installs an unverified artifact. The
// renovate custom manager opens bump PRs; they are labeled needs-review so a
// human refreshes the hashes before merge (see renovate.json).

// renovate: datasource=github-tags depName=aws/aws-cli
const AWS_CLI_VERSION: &str = "2.35.20";
// renovate: datasource=pypi depName=azure-cli
const AZURE_CLI_VERSION: &str = "2.88.0";
// 1Password publishes no renovate-compatible datasource; bump by hand.
const OP_CLI_VERSION: &str = "2.34.1";
// renovate: datasource=github-releases depName=GAM-team/GAM
const GAM_VERSION: &str = "7.46.08";
// renovate: datasource=github-releases depName=microsoftgraph/msgraph-beta-cli
const MGC_BETA_VERSION: &str = "0.2.3";
// Atlassian CLI currently publishes stable channel archives at a stable URL path.
const ACLI_VERSION: &str = "1.3.22-stable";
// renovate: datasource=github-releases depName=googleworkspace/cli
const GWS_VERSION: &str = "0.22.5";

/// How long a `<tool> --version` probe of a host-provided copy may run.
const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum CliToolError {
    #[error("unknown CLI tool")]
    UnknownTool,
    #[error("{0} is not supported on this platform/host: {1}")]
    Unsupported(String, String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("checksum verification failed for {url}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        url: String,
        expected: String,
        actual: String,
    },
    #[error("extraction failed: {0}")]
    Extract(String),
    #[error("install failed: {0}")]
    Install(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum CliToolId {
    Aws,
    Az,
    Op,
    Gam,
    MgcBeta,
    Acli,
    Gws,
}

impl CliToolId {
    pub const ALL: [CliToolId; 7] = [
        CliToolId::Aws,
        CliToolId::Az,
        CliToolId::Op,
        CliToolId::Gam,
        CliToolId::MgcBeta,
        CliToolId::Acli,
        CliToolId::Gws,
    ];

    /// Directory name under `cli-tools/` (also the serde wire id).
    fn dir_name(&self) -> &'static str {
        match self {
            CliToolId::Aws => "aws",
            CliToolId::Az => "az",
            CliToolId::Op => "op",
            CliToolId::Gam => "gam",
            CliToolId::MgcBeta => "mgc-beta",
            CliToolId::Acli => "acli",
            CliToolId::Gws => "gws",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Zip,
    TarGz,
    TarXz,
}

#[derive(Debug, Clone, Copy)]
pub enum InstallStrategy {
    /// Download an archive, extract it in staging, expose one binary via the
    /// `bin/` symlink.
    ArchiveBinary { archive: ArchiveKind },
    /// `python3 -m venv` + `pip install <package>==<version>`; expose the
    /// venv's entry-point script. Requires a host python3.
    PythonVenv { package: &'static str },
}

#[derive(Debug, Clone, Copy)]
pub struct PlatformSource {
    pub os: &'static str,   // std::env::consts::OS values: "linux", "macos"
    pub arch: &'static str, // std::env::consts::ARCH values: "x86_64", "aarch64"
    /// Download URL; `{version}` is replaced with the pinned version.
    pub url: &'static str,
    /// Pinned sha256 of the artifact at the pinned version (hex, lowercase).
    pub sha256: &'static str,
    /// Path of the executable inside the extracted archive for this platform.
    pub binary_path_in_archive: &'static str,
}

pub struct CliToolCatalogEntry {
    pub id: CliToolId,
    pub binary_name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub version: &'static str,
    /// Args used to probe a host-provided copy's version.
    pub version_args: &'static [&'static str],
    pub sources: &'static [PlatformSource],
    pub strategy: InstallStrategy,
    /// Vendor docs for authentication/setup — the app never manages
    /// credentials for these tools.
    pub docs_url: &'static str,
    pub auth: CliToolAuthStrategy,
}

#[derive(Debug, Clone, Copy)]
pub enum CliToolAuthStrategy {
    Command {
        login_args: &'static [&'static str],
        probe_args: &'static [&'static str],
    },
    Unsupported(&'static str),
}

pub fn catalog() -> &'static [CliToolCatalogEntry] {
    static CATALOG: &[CliToolCatalogEntry] = &[
        CliToolCatalogEntry {
            id: CliToolId::Aws,
            binary_name: "aws",
            display_name: "AWS CLI v2",
            description: "Amazon Web Services command line interface",
            version: AWS_CLI_VERSION,
            version_args: &["--version"],
            sources: &[
                PlatformSource {
                    os: "linux",
                    arch: "x86_64",
                    url: "https://awscli.amazonaws.com/awscli-exe-linux-x86_64-{version}.zip",
                    sha256: "a4aa00212a97e6a5abd38cde5524719f7f5f122a6dcfdbc974eefdf741de1be6",
                    binary_path_in_archive: "aws/dist/aws",
                },
                PlatformSource {
                    os: "linux",
                    arch: "aarch64",
                    url: "https://awscli.amazonaws.com/awscli-exe-linux-aarch64-{version}.zip",
                    sha256: "58799ce9276d4e8815fd19e4dc35649626c6b4fbd4d0e3df7433af9cfde41882",
                    binary_path_in_archive: "aws/dist/aws",
                },
                // macOS ships only a .pkg installer; no app-owned extraction.
            ],
            strategy: InstallStrategy::ArchiveBinary {
                archive: ArchiveKind::Zip,
            },
            docs_url: "https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-files.html",
            auth: CliToolAuthStrategy::Unsupported(
                "AWS SSO login is profile-specific; manage SSO profiles and sign in from the AWS section in Settings",
            ),
        },
        CliToolCatalogEntry {
            id: CliToolId::Az,
            binary_name: "az",
            display_name: "Azure CLI",
            description: "Microsoft Azure command line interface (installed from PyPI into an app-owned venv)",
            version: AZURE_CLI_VERSION,
            version_args: &["--version"],
            sources: &[], // PyPI via pip; no direct artifact URL
            strategy: InstallStrategy::PythonVenv {
                package: "azure-cli",
            },
            docs_url: "https://learn.microsoft.com/en-us/cli/azure/authenticate-azure-cli",
            auth: CliToolAuthStrategy::Command {
                login_args: &["login", "--use-device-code"],
                probe_args: &["account", "show", "--output", "none"],
            },
        },
        CliToolCatalogEntry {
            id: CliToolId::Op,
            binary_name: "op",
            display_name: "1Password CLI",
            description: "1Password command line interface",
            version: OP_CLI_VERSION,
            version_args: &["--version"],
            sources: &[
                PlatformSource {
                    os: "linux",
                    arch: "x86_64",
                    url: "https://cache.agilebits.com/dist/1P/op2/pkg/v{version}/op_linux_amd64_v{version}.zip",
                    sha256: "b13ed106335419ea0fb0ebd7ebbb3b48cf26a2f214eb4b2fd8d950548e7980ed",
                    binary_path_in_archive: "op",
                },
                PlatformSource {
                    os: "linux",
                    arch: "aarch64",
                    url: "https://cache.agilebits.com/dist/1P/op2/pkg/v{version}/op_linux_arm64_v{version}.zip",
                    sha256: "fd730a28ffa68376ac62b563d30e20e30ef59d3e2f142d9c6a959cfac5b50f60",
                    binary_path_in_archive: "op",
                },
                PlatformSource {
                    os: "macos",
                    arch: "aarch64",
                    url: "https://cache.agilebits.com/dist/1P/op2/pkg/v{version}/op_darwin_arm64_v{version}.zip",
                    sha256: "101b54dd194fbb6c63276b84f5eee1968be3558e2212519d9f5e26ab24a4ad05",
                    binary_path_in_archive: "op",
                },
            ],
            strategy: InstallStrategy::ArchiveBinary {
                archive: ArchiveKind::Zip,
            },
            docs_url: "https://developer.1password.com/docs/cli/get-started/",
            auth: CliToolAuthStrategy::Unsupported(
                "1Password sign-in sessions are shell-scoped; authenticate the host CLI using the vendor setup guide",
            ),
        },
        CliToolCatalogEntry {
            id: CliToolId::Gam,
            binary_name: "gam",
            display_name: "GAM7",
            description: "Google Workspace admin CLI (GAM-team/GAM). Credentials/config stay host-managed.",
            version: GAM_VERSION,
            version_args: &["version"],
            sources: &[
                PlatformSource {
                    os: "linux",
                    arch: "x86_64",
                    url: "https://github.com/GAM-team/GAM/releases/download/v{version}/gam-{version}-linux-x86_64-glibc2.35.tar.xz",
                    sha256: "7b5219ef0d4c797cb7016439d4a7e335bf23eef7d439df5c6878f69cacd1f36b",
                    binary_path_in_archive: "gam7/gam",
                },
                PlatformSource {
                    os: "linux",
                    arch: "aarch64",
                    url: "https://github.com/GAM-team/GAM/releases/download/v{version}/gam-{version}-linux-arm64-glibc2.35.tar.xz",
                    sha256: "edaff168b0a4e35aaacc8c85aa15c048e4a78d1e09dc19e100e45194eb3d54d1",
                    binary_path_in_archive: "gam7/gam",
                },
                PlatformSource {
                    os: "macos",
                    arch: "aarch64",
                    url: "https://github.com/GAM-team/GAM/releases/download/v{version}/gam-{version}-macos14.8-arm64.tar.xz",
                    sha256: "7995b64f96351fac9b45c19e28c349806a37f6f5efdcebeeb9d090057f888516",
                    binary_path_in_archive: "gam7/gam",
                },
            ],
            strategy: InstallStrategy::ArchiveBinary {
                archive: ArchiveKind::TarXz,
            },
            docs_url: "https://github.com/GAM-team/GAM/wiki",
            auth: CliToolAuthStrategy::Command {
                login_args: &["oauth", "create"],
                probe_args: &["oauth", "verify"],
            },
        },
        CliToolCatalogEntry {
            id: CliToolId::MgcBeta,
            binary_name: "mgc-beta",
            display_name: "Microsoft Graph CLI (beta)",
            description: "Microsoft Graph command line interface, beta API channel",
            version: MGC_BETA_VERSION,
            version_args: &["--version"],
            sources: &[
                PlatformSource {
                    os: "linux",
                    arch: "x86_64",
                    url: "https://github.com/microsoftgraph/msgraph-beta-cli/releases/download/v{version}/msgraph-beta-cli-linux-x64-{version}.tar.gz",
                    sha256: "d03394d1a7c3c9f23c0fc1429ee2bd9dca2ba5d36f996a68d4b58f06e18f3e9d",
                    binary_path_in_archive: "mgc-beta",
                },
                PlatformSource {
                    os: "macos",
                    arch: "aarch64",
                    url: "https://github.com/microsoftgraph/msgraph-beta-cli/releases/download/v{version}/msgraph-beta-cli-osx-arm64-{version}.tar.gz",
                    sha256: "5ba2ad2750c7d745b129a1cde5ec348f811440df66138de293d85f9e64c75570",
                    binary_path_in_archive: "mgc-beta",
                },
            ],
            strategy: InstallStrategy::ArchiveBinary {
                archive: ArchiveKind::TarGz,
            },
            docs_url: "https://learn.microsoft.com/en-us/graph/cli/overview",
            auth: CliToolAuthStrategy::Unsupported(
                "This pinned Graph CLI has no non-secret authentication status command; run mgc-beta login externally",
            ),
        },
        CliToolCatalogEntry {
            id: CliToolId::Acli,
            binary_name: "acli",
            display_name: "Atlassian CLI",
            description: "Atlassian Cloud command line interface",
            version: ACLI_VERSION,
            version_args: &["--version"],
            sources: &[
                PlatformSource {
                    os: "linux",
                    arch: "x86_64",
                    url: "https://acli.atlassian.com/linux/{version}/acli_{version}_linux_amd64.tar.gz",
                    sha256: "de9e0a60a556e4119428b9072f6ca787e75b9f9a538aa71ebcc8084deb8ca1a6",
                    binary_path_in_archive: "acli_1.3.22-stable_linux_amd64/acli",
                },
                PlatformSource {
                    os: "linux",
                    arch: "aarch64",
                    url: "https://acli.atlassian.com/linux/{version}/acli_{version}_linux_arm64.tar.gz",
                    sha256: "1a9e86d0b46a62a8f1992c1ef98b3af7e9a9ee3f76d0efa215fe1f2d1b2fd139",
                    binary_path_in_archive: "acli_1.3.22-stable_linux_arm64/acli",
                },
            ],
            strategy: InstallStrategy::ArchiveBinary {
                archive: ArchiveKind::TarGz,
            },
            docs_url: "https://developer.atlassian.com/cloud/acli/guides/install-acli/",
            auth: CliToolAuthStrategy::Unsupported(
                "Atlassian CLI authentication is site and account specific; use the vendor setup guide",
            ),
        },
        CliToolCatalogEntry {
            id: CliToolId::Gws,
            binary_name: "gws",
            display_name: "Google Workspace CLI",
            description: "Google Workspace command line interface for Drive, Gmail, Calendar, and more",
            version: GWS_VERSION,
            version_args: &["--version"],
            sources: &[
                PlatformSource {
                    os: "linux",
                    arch: "x86_64",
                    url: "https://github.com/googleworkspace/cli/releases/download/v{version}/google-workspace-cli-x86_64-unknown-linux-gnu.tar.gz",
                    sha256: "de78ecdbd2f1a84cca0063a7ecbc440240fc14b6ebccbb17f4646b792a8c5c1f",
                    binary_path_in_archive: "gws",
                },
                PlatformSource {
                    os: "linux",
                    arch: "aarch64",
                    url: "https://github.com/googleworkspace/cli/releases/download/v{version}/google-workspace-cli-aarch64-unknown-linux-gnu.tar.gz",
                    sha256: "94490295d9580e1e88574e715a0a162991747d12d62f8c7b8dcc8268b6c1cea0",
                    binary_path_in_archive: "gws",
                },
                PlatformSource {
                    os: "macos",
                    arch: "x86_64",
                    url: "https://github.com/googleworkspace/cli/releases/download/v{version}/google-workspace-cli-x86_64-apple-darwin.tar.gz",
                    sha256: "51f9bd731404d4bba26c36e2e30dd68c56dccd1f834c01252cb0b14d6a6544b2",
                    binary_path_in_archive: "gws",
                },
                PlatformSource {
                    os: "macos",
                    arch: "aarch64",
                    url: "https://github.com/googleworkspace/cli/releases/download/v{version}/google-workspace-cli-aarch64-apple-darwin.tar.gz",
                    sha256: "1d2a9ffd5bc9b2c2c4b48630daf082fad13d9e57d741988a2c248eed562f7dac",
                    binary_path_in_archive: "gws",
                },
            ],
            strategy: InstallStrategy::ArchiveBinary {
                archive: ArchiveKind::TarGz,
            },
            docs_url: "https://github.com/googleworkspace/cli#authentication",
            auth: CliToolAuthStrategy::Unsupported(
                "gws auth status exits successfully even when unauthenticated; run gws auth login externally",
            ),
        },
    ];
    CATALOG
}

fn entry(id: CliToolId) -> &'static CliToolCatalogEntry {
    catalog()
        .iter()
        .find(|e| e.id == id)
        .expect("every CliToolId has a catalog entry")
}

/// The only directory ever exposed on spawned agents' PATH.
pub fn cli_tools_bin_dir() -> PathBuf {
    cli_tools_dir().join("bin")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledManifest {
    version: String,
    installed_at: String,
    verification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct HostCopy {
    pub path: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AppCopy {
    pub version: String,
    pub outdated: bool,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CliToolStatus {
    pub id: CliToolId,
    pub binary_name: String,
    pub display_name: String,
    pub description: String,
    pub catalog_version: String,
    pub supported: bool,
    pub unsupported_reason: Option<String>,
    pub host: Option<HostCopy>,
    pub app: Option<AppCopy>,
    pub docs_url: String,
    pub login_supported: bool,
    pub auth_state: CliToolAuthState,
    pub auth_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CliToolAuthState {
    Authenticated,
    Unauthenticated,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone)]
pub struct CliToolLoginCommand {
    pub executable: PathBuf,
    pub args: Vec<String>,
}

/// Per-tool install locks: one writer per tool directory.
fn lock_for(id: CliToolId) -> &'static Mutex<()> {
    static LOCKS: OnceLock<HashMap<CliToolId, Mutex<()>>> = OnceLock::new();
    LOCKS
        .get_or_init(|| {
            CliToolId::ALL
                .iter()
                .map(|id| (*id, Mutex::new(())))
                .collect()
        })
        .get(&id)
        .expect("lock map covers all tool ids")
}

fn login_lock_for(id: CliToolId) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<HashMap<CliToolId, Arc<Mutex<()>>>> = OnceLock::new();
    LOCKS
        .get_or_init(|| {
            CliToolId::ALL
                .iter()
                .map(|id| (*id, Arc::new(Mutex::new(()))))
                .collect()
        })
        .get(&id)
        .expect("login lock map covers all tool ids")
        .clone()
}

pub fn try_begin_login(id: CliToolId) -> Option<OwnedMutexGuard<()>> {
    login_lock_for(id).try_lock_owned().ok()
}

fn platform_source(e: &CliToolCatalogEntry) -> Option<&'static PlatformSource> {
    platform_source_for(e, std::env::consts::OS, std::env::consts::ARCH)
}

fn platform_source_for(
    e: &CliToolCatalogEntry,
    os: &str,
    arch: &str,
) -> Option<&'static PlatformSource> {
    e.sources.iter().find(|s| s.os == os && s.arch == arch)
}

/// Why this tool can't be installed here, if it can't.
async fn unsupported_reason(e: &CliToolCatalogEntry) -> Option<String> {
    match e.strategy {
        InstallStrategy::ArchiveBinary { .. } => {
            if cfg!(windows) {
                return Some("not supported on Windows".to_string());
            }
            if platform_source(e).is_none() {
                return Some(format!(
                    "no {}/{} build published by the vendor",
                    std::env::consts::OS,
                    std::env::consts::ARCH
                ));
            }
            None
        }
        InstallStrategy::PythonVenv { .. } => {
            if cfg!(windows) {
                return Some("not supported on Windows".to_string());
            }
            if resolve_executable_path("python3").await.is_none() {
                return Some("requires python3 (>=3.8) on the host".to_string());
            }
            None
        }
    }
}

async fn probe_version(path: &Path, args: &[&str]) -> Option<String> {
    let output = tokio::time::timeout(
        VERSION_PROBE_TIMEOUT,
        tokio::process::Command::new(path)
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().find(|l| !l.trim().is_empty())?;
    Some(line.trim().chars().take(120).collect())
}

/// Host-provided copy of the tool, ignoring our own `cli-tools/bin`.
async fn detect_host_copy(e: &CliToolCatalogEntry) -> Option<HostCopy> {
    let path = resolve_executable_path(e.binary_name).await?;
    if path.starts_with(cli_tools_bin_dir()) {
        return None;
    }
    let version = probe_version(&path, e.version_args).await;
    Some(HostCopy {
        path: path.to_string_lossy().into_owned(),
        version,
    })
}

async fn effective_binary(e: &CliToolCatalogEntry) -> Option<PathBuf> {
    if let Some(host) = detect_host_copy(e).await {
        return Some(PathBuf::from(host.path));
    }
    detect_app_copy(e).map(|_| installed_binary_path(e))
}

/// Effective binary for a catalog tool (host copy wins over the app-managed
/// copy, exactly as agent execution resolves it). For features that build
/// runtime-parameterized commands on top of a managed tool, e.g. AWS SSO
/// profile login.
pub async fn effective_binary_for(id: CliToolId) -> Option<PathBuf> {
    effective_binary(entry(id)).await
}

pub async fn login_command(id: CliToolId) -> Result<CliToolLoginCommand, CliToolError> {
    let e = entry(id);
    let CliToolAuthStrategy::Command { login_args, .. } = e.auth else {
        let CliToolAuthStrategy::Unsupported(reason) = e.auth else {
            unreachable!()
        };
        return Err(CliToolError::Unsupported(
            e.display_name.to_string(),
            reason.to_string(),
        ));
    };
    let executable = effective_binary(e).await.ok_or_else(|| {
        CliToolError::Unsupported(
            e.display_name.to_string(),
            "tool is not available".to_string(),
        )
    })?;
    Ok(CliToolLoginCommand {
        executable,
        args: login_args.iter().map(|arg| (*arg).to_string()).collect(),
    })
}

async fn probe_auth(e: &CliToolCatalogEntry) -> (CliToolAuthState, Option<String>) {
    let CliToolAuthStrategy::Command { probe_args, .. } = e.auth else {
        let CliToolAuthStrategy::Unsupported(reason) = e.auth else {
            unreachable!()
        };
        return (CliToolAuthState::Unsupported, Some(reason.to_string()));
    };
    let Some(executable) = effective_binary(e).await else {
        return (
            CliToolAuthState::Unknown,
            Some("Tool is not available".to_string()),
        );
    };
    let mut command = tokio::process::Command::new(executable);
    command.args(probe_args).env_clear();
    for key in [
        "HOME",
        "USER",
        "PATH",
        "TMPDIR",
        "TEMP",
        "LANG",
        "LC_ALL",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "no_proxy",
        "REQUESTS_CA_BUNDLE",
        "CURL_CA_BUNDLE",
        "XDG_CONFIG_HOME",
        "AZURE_CONFIG_DIR",
        "GAMCFGDIR",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    match tokio::time::timeout(VERSION_PROBE_TIMEOUT, command.kill_on_drop(true).output()).await {
        Ok(Ok(output)) if output.status.success() => (CliToolAuthState::Authenticated, None),
        Ok(Ok(_)) => (CliToolAuthState::Unauthenticated, None),
        Ok(Err(_)) => (
            CliToolAuthState::Unknown,
            Some("Authentication check failed".to_string()),
        ),
        Err(_) => (
            CliToolAuthState::Unknown,
            Some("Authentication check timed out".to_string()),
        ),
    }
}

fn tool_dir(id: CliToolId) -> PathBuf {
    cli_tools_dir().join(entry(id).id.dir_name())
}

fn manifest_path(id: CliToolId) -> PathBuf {
    tool_dir(id).join("manifest.json")
}

fn bin_link_path(id: CliToolId) -> PathBuf {
    cli_tools_bin_dir().join(entry(id).binary_name)
}

fn read_manifest(id: CliToolId) -> Option<InstalledManifest> {
    let raw = std::fs::read_to_string(manifest_path(id)).ok()?;
    serde_json::from_str(&raw).ok()
}

fn detect_app_copy(e: &CliToolCatalogEntry) -> Option<AppCopy> {
    let manifest = read_manifest(e.id)?;
    // A manifest without a working bin symlink is a broken install: report
    // nothing so the UI offers a fresh install (which cleans it up).
    let link = bin_link_path(e.id);
    let target = std::fs::read_link(&link).ok()?;
    let resolved = if target.is_absolute() {
        target
    } else {
        link.parent()?.join(target)
    };
    if !resolved.exists() {
        return None;
    }
    Some(AppCopy {
        outdated: manifest.version != e.version,
        version: manifest.version,
        installed_at: manifest.installed_at,
    })
}

pub async fn status(id: CliToolId) -> CliToolStatus {
    let e = entry(id);
    let reason = unsupported_reason(e).await;
    let (auth_state, auth_message) = probe_auth(e).await;
    CliToolStatus {
        id: e.id,
        binary_name: e.binary_name.to_string(),
        display_name: e.display_name.to_string(),
        description: e.description.to_string(),
        catalog_version: e.version.to_string(),
        supported: reason.is_none(),
        unsupported_reason: reason,
        host: detect_host_copy(e).await,
        app: detect_app_copy(e),
        docs_url: e.docs_url.to_string(),
        login_supported: matches!(e.auth, CliToolAuthStrategy::Command { .. }),
        auth_state,
        auth_message,
    }
}

pub async fn status_all() -> Vec<CliToolStatus> {
    futures::future::join_all(CliToolId::ALL.into_iter().map(status)).await
}

/// Install (or update to) the catalog's pinned version. Idempotent.
pub async fn install(id: CliToolId) -> Result<CliToolStatus, CliToolError> {
    let e = entry(id);
    let _guard = lock_for(id).lock().await;

    if let Some(reason) = unsupported_reason(e).await {
        return Err(CliToolError::Unsupported(
            e.display_name.to_string(),
            reason,
        ));
    }
    if let Some(m) = read_manifest(id)
        && m.version == e.version
        && detect_app_copy(e).is_some()
    {
        drop(_guard);
        return Ok(status(id).await);
    }

    clean_staging(id).await;
    let verification = match e.strategy {
        InstallStrategy::ArchiveBinary { archive } => install_archive(e, archive).await?,
        InstallStrategy::PythonVenv { package } => install_python_venv(e, package).await?,
    };

    let manifest = InstalledManifest {
        version: e.version.to_string(),
        installed_at: chrono::Utc::now().to_rfc3339(),
        verification,
    };
    std::fs::write(
        manifest_path(id),
        serde_json::to_string_pretty(&manifest).expect("manifest serializes"),
    )?;

    // Expose last: symlink into a temp name, then atomically rename over the
    // final name.
    #[cfg(unix)]
    {
        let bin_dir = cli_tools_bin_dir();
        std::fs::create_dir_all(&bin_dir)?;
        let target = installed_binary_path(e);
        let tmp = bin_dir.join(format!(".tmp-{}", e.binary_name));
        let _ = std::fs::remove_file(&tmp);
        std::os::unix::fs::symlink(&target, &tmp)?;
        std::fs::rename(&tmp, bin_link_path(id))?;
    }

    remove_stale_versions(e);
    drop(_guard);
    Ok(status(id).await)
}

/// Remove the app-installed copy entirely. Idempotent.
pub async fn remove(id: CliToolId) -> Result<CliToolStatus, CliToolError> {
    let e = entry(id);
    let _guard = lock_for(id).lock().await;

    let link = bin_link_path(id);
    if link.symlink_metadata().is_ok() {
        std::fs::remove_file(&link)?;
    }
    let dir = tool_dir(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    drop(_guard);
    Ok(status(e.id).await)
}

/// Absolute path of the pinned version's binary inside the tool dir.
fn installed_binary_path(e: &CliToolCatalogEntry) -> PathBuf {
    let version_dir = tool_dir(e.id).join(e.version);
    match e.strategy {
        InstallStrategy::ArchiveBinary { .. } => {
            let source = platform_source(e).expect("checked by unsupported_reason");
            version_dir.join(source.binary_path_in_archive)
        }
        InstallStrategy::PythonVenv { .. } => version_dir.join("venv/bin").join(e.binary_name),
    }
}

/// Per-tool staging root, so concurrent installs of *different* tools never
/// touch each other's staging (the per-tool lock only guards one tool).
fn staging_dir(id: CliToolId) -> PathBuf {
    cli_tools_dir()
        .join(".staging")
        .join(entry(id).id.dir_name())
}

/// Best-effort removal of this tool's leftovers from crashed/failed installs.
/// Called under the tool's lock.
async fn clean_staging(id: CliToolId) {
    let dir = staging_dir(id);
    if dir.exists() {
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}

/// Delete version dirs other than the pinned one (kept layout: one version).
fn remove_stale_versions(e: &CliToolCatalogEntry) {
    let dir = tool_dir(e.id);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for d in entries.flatten() {
        let name = d.file_name();
        if d.path().is_dir() && name != std::ffi::OsStr::new(e.version) {
            let _ = std::fs::remove_dir_all(d.path());
        }
    }
}

/// Download `url` to `dest`, returning the artifact's sha256 (hex).
async fn download(url: &str, dest: &Path) -> Result<String, CliToolError> {
    let client = reqwest::Client::builder()
        .user_agent(format!("vibe-kanban/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| CliToolError::Download(e.to_string()))?;
    let mut resp = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| CliToolError::Download(format!("{url}: {e}")))?;

    let mut file = tokio::fs::File::create(dest).await?;
    let mut hasher = Sha256::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| CliToolError::Download(format!("{url}: {e}")))?
    {
        hasher.update(&chunk);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Shared tail of both install strategies: move the fully-built staging
/// version dir into place under the tool dir.
fn promote_staged_version(e: &CliToolCatalogEntry, staged: &Path) -> Result<(), CliToolError> {
    let dir = tool_dir(e.id);
    std::fs::create_dir_all(&dir)?;
    let version_dir = dir.join(e.version);
    if version_dir.exists() {
        std::fs::remove_dir_all(&version_dir)?;
    }
    std::fs::rename(staged, &version_dir).map_err(|err| {
        CliToolError::Install(format!(
            "failed to move staged install into place: {err} (staging and data dirs must share a filesystem)"
        ))
    })
}

async fn install_archive(
    e: &'static CliToolCatalogEntry,
    archive: ArchiveKind,
) -> Result<String, CliToolError> {
    let source = platform_source(e).expect("checked by unsupported_reason");
    let url = source.url.replace("{version}", e.version);

    let stage = staging_dir(e.id).join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&stage)?;
    let result = async {
        let archive_path = stage.join("artifact");
        let actual = download(&url, &archive_path).await?;
        if actual != source.sha256 {
            return Err(CliToolError::ChecksumMismatch {
                url: url.clone(),
                expected: source.sha256.to_string(),
                actual,
            });
        }

        let extract_dir = stage.join("extracted");
        std::fs::create_dir_all(&extract_dir)?;
        let extract_to = extract_dir.clone();
        tokio::task::spawn_blocking(move || extract(archive, &archive_path, &extract_to))
            .await
            .map_err(|e| CliToolError::Extract(format!("extraction task panicked: {e}")))??;

        let binary = extract_dir.join(source.binary_path_in_archive);
        if !binary.is_file() {
            return Err(CliToolError::Extract(format!(
                "expected binary {} not found in archive",
                source.binary_path_in_archive
            )));
        }
        promote_staged_version(e, &extract_dir)?;
        Ok(format!("sha256:{}", source.sha256))
    }
    .await;

    let _ = std::fs::remove_dir_all(&stage);
    result
}

fn extract(kind: ArchiveKind, archive_path: &Path, dest: &Path) -> Result<(), CliToolError> {
    match kind {
        ArchiveKind::Zip => {
            let file = std::fs::File::open(archive_path)?;
            let mut zip = zip::ZipArchive::new(file)
                .map_err(|e| CliToolError::Extract(format!("invalid zip: {e}")))?;
            zip.extract(dest)
                .map_err(|e| CliToolError::Extract(format!("unzip failed: {e}")))?;
            Ok(())
        }
        ArchiveKind::TarGz => {
            let file = std::fs::File::open(archive_path)?;
            let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
            tar.unpack(dest)
                .map_err(|e| CliToolError::Extract(format!("untar (gz) failed: {e}")))?;
            Ok(())
        }
        ArchiveKind::TarXz => {
            let file = std::fs::File::open(archive_path)?;
            let mut tar = tar::Archive::new(xz2::read::XzDecoder::new(file));
            tar.unpack(dest)
                .map_err(|e| CliToolError::Extract(format!("untar (xz) failed: {e}")))?;
            Ok(())
        }
    }
}

/// az: build the venv directly at its final location — venvs hardcode their
/// creation path in `pyvenv.cfg` and script shebangs, so they cannot be moved
/// after creation. Atomicity holds because exposure still happens only via
/// the `bin/` symlink, created after pip succeeds.
///
/// Supply-chain caveat (recorded in the manifest verification string): the
/// top-level package version is pinned, and pip checks each downloaded
/// artifact against the sha256 the PyPI index advertises, but the transitive
/// dependency closure is resolved live — weaker than the pinned-hash archive
/// strategy. A hashed requirements set (`--require-hashes`) is the recorded
/// follow-up if az's venv path ever graduates from laptop-fallback status;
/// on the NixOS host az is expected to stay nix-managed.
async fn install_python_venv(
    e: &'static CliToolCatalogEntry,
    package: &'static str,
) -> Result<String, CliToolError> {
    let python3 = resolve_executable_path("python3")
        .await
        .expect("checked by unsupported_reason");

    let version_dir = tool_dir(e.id).join(e.version);
    if version_dir.exists() {
        std::fs::remove_dir_all(&version_dir)?;
    }
    let venv_dir = version_dir.join("venv");
    std::fs::create_dir_all(&version_dir)?;

    let result = async {
        run_checked(
            tokio::process::Command::new(&python3)
                .args(["-m", "venv"])
                .arg(&venv_dir),
            "python3 -m venv",
        )
        .await?;
        run_checked(
            tokio::process::Command::new(venv_dir.join("bin/pip"))
                .args(["install", "--no-input", "--disable-pip-version-check"])
                .arg(format!("{package}=={}", e.version)),
            "pip install",
        )
        .await?;
        if !venv_dir.join("bin").join(e.binary_name).is_file() {
            return Err(CliToolError::Install(format!(
                "venv did not produce a {} entry point",
                e.binary_name
            )));
        }
        Ok(format!(
            "pypi (pinned {package}=={}; per-artifact index hashes; transitive deps resolver-dependent)",
            e.version
        ))
    }
    .await;

    if result.is_err() {
        let _ = std::fs::remove_dir_all(&version_dir);
    }
    result
}

async fn run_checked(
    command: &mut tokio::process::Command,
    label: &str,
) -> Result<(), CliToolError> {
    let output = command
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| CliToolError::Install(format!("{label}: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut start = stderr.len().saturating_sub(800);
        while !stderr.is_char_boundary(start) {
            start += 1;
        }
        let tail = &stderr[start..];
        return Err(CliToolError::Install(format!(
            "{label} exited with {}: {tail}",
            output.status
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_ids() -> Vec<CliToolId> {
        catalog().iter().map(|e| e.id).collect()
    }

    #[test]
    fn catalog_covers_every_tool_id_exactly_once() {
        let ids = catalog_ids();
        assert_eq!(ids.len(), CliToolId::ALL.len());
        for id in CliToolId::ALL {
            assert_eq!(ids.iter().filter(|i| **i == id).count(), 1);
        }
    }

    #[test]
    fn every_archive_source_has_a_pinned_sha256() {
        for e in catalog() {
            for s in e.sources {
                assert_eq!(
                    s.sha256.len(),
                    64,
                    "{}: sha256 must be pinned",
                    e.binary_name
                );
                assert!(
                    s.sha256.chars().all(|c| c.is_ascii_hexdigit()),
                    "{}: sha256 must be hex",
                    e.binary_name
                );
                assert!(
                    s.url.starts_with("https://"),
                    "{}: downloads must be HTTPS",
                    e.binary_name
                );
            }
        }
    }

    #[test]
    fn wire_ids_round_trip_kebab_case() {
        for id in CliToolId::ALL {
            let wire = serde_json::to_string(&id).unwrap();
            assert_eq!(wire.trim_matches('"'), entry(id).id.dir_name());
            let back: CliToolId = serde_json::from_str(&wire).unwrap();
            assert_eq!(back, id);
        }
    }

    #[test]
    fn acli_catalog_entry_is_pinned_and_linux_only() {
        let e = entry(CliToolId::Acli);
        assert_eq!(e.id.dir_name(), "acli");
        assert_eq!(
            serde_json::to_string(&CliToolId::Acli).unwrap(),
            r#""acli""#
        );
        assert_eq!(
            serde_json::from_str::<CliToolId>(r#""acli""#).unwrap(),
            CliToolId::Acli
        );
        assert_eq!(e.binary_name, "acli");
        assert_eq!(e.display_name, "Atlassian CLI");
        assert_eq!(e.description, "Atlassian Cloud command line interface");
        assert_eq!(e.version, "1.3.22-stable");
        assert_eq!(
            e.docs_url,
            "https://developer.atlassian.com/cloud/acli/guides/install-acli/"
        );
        assert_eq!(e.version_args, &["--version"]);
        assert!(matches!(
            e.strategy,
            InstallStrategy::ArchiveBinary {
                archive: ArchiveKind::TarGz
            }
        ));

        assert_eq!(e.sources.len(), 2);
        let amd64 = platform_source_for(e, "linux", "x86_64").unwrap();
        assert_eq!(
            amd64.url,
            "https://acli.atlassian.com/linux/{version}/acli_{version}_linux_amd64.tar.gz"
        );
        assert_eq!(
            amd64.sha256,
            "de9e0a60a556e4119428b9072f6ca787e75b9f9a538aa71ebcc8084deb8ca1a6"
        );
        assert_eq!(
            amd64.binary_path_in_archive,
            "acli_1.3.22-stable_linux_amd64/acli"
        );

        let arm64 = platform_source_for(e, "linux", "aarch64").unwrap();
        assert_eq!(
            arm64.url,
            "https://acli.atlassian.com/linux/{version}/acli_{version}_linux_arm64.tar.gz"
        );
        assert_eq!(
            arm64.sha256,
            "1a9e86d0b46a62a8f1992c1ef98b3af7e9a9ee3f76d0efa215fe1f2d1b2fd139"
        );
        assert_eq!(
            arm64.binary_path_in_archive,
            "acli_1.3.22-stable_linux_arm64/acli"
        );
        assert!(e.sources.iter().all(|s| s.os == "linux"));
    }

    #[test]
    fn archive_sources_preserve_existing_binary_paths() {
        let expected = [
            (CliToolId::Aws, "aws/dist/aws"),
            (CliToolId::Op, "op"),
            (CliToolId::Gam, "gam7/gam"),
            (CliToolId::MgcBeta, "mgc-beta"),
            (CliToolId::Gws, "gws"),
        ];

        for (id, binary_path) in expected {
            let e = entry(id);
            assert!(matches!(e.strategy, InstallStrategy::ArchiveBinary { .. }));
            assert!(
                !e.sources.is_empty(),
                "{} should keep explicit archive sources",
                e.binary_name
            );
            for source in e.sources {
                assert_eq!(
                    source.binary_path_in_archive, binary_path,
                    "{} {}/{} archive binary path changed",
                    e.binary_name, source.os, source.arch
                );
            }
        }
    }

    #[test]
    fn only_durably_verifiable_tools_offer_login() {
        for id in [CliToolId::Az, CliToolId::Gam] {
            assert!(matches!(
                entry(id).auth,
                CliToolAuthStrategy::Command { .. }
            ));
        }
        for id in [
            CliToolId::Aws,
            CliToolId::Op,
            CliToolId::MgcBeta,
            CliToolId::Acli,
            CliToolId::Gws,
        ] {
            assert!(matches!(
                entry(id).auth,
                CliToolAuthStrategy::Unsupported(_)
            ));
        }
    }

    #[test]
    fn acli_uses_existing_unsupported_platform_matrix() {
        let e = entry(CliToolId::Acli);
        assert!(platform_source_for(e, "linux", "x86_64").is_some());
        assert!(platform_source_for(e, "linux", "aarch64").is_some());
        assert!(platform_source_for(e, "linux", "arm").is_none());
        assert!(platform_source_for(e, "macos", "aarch64").is_none());
        assert!(platform_source_for(e, "windows", "x86_64").is_none());
    }

    #[test]
    fn login_lock_rejects_a_concurrent_session() {
        let first = try_begin_login(CliToolId::Az).expect("first login owns lock");
        assert!(try_begin_login(CliToolId::Az).is_none());
        drop(first);
        assert!(try_begin_login(CliToolId::Az).is_some());
    }

    #[test]
    fn manifest_round_trips() {
        let m = InstalledManifest {
            version: "1.2.3".into(),
            installed_at: "2026-07-10T00:00:00Z".into(),
            verification: "sha256:abc".into(),
        };
        let raw = serde_json::to_string(&m).unwrap();
        let back: InstalledManifest = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.version, m.version);
        assert_eq!(back.installed_at, m.installed_at);
        assert_eq!(back.verification, m.verification);
    }

    /// Real end-to-end install/remove against vendor download servers.
    /// Ignored by default (network + ~70MB); run explicitly for the
    /// acceptance pass: `cargo test -p services -- --ignored cli_tools`.
    /// Uses the debug-build asset dir (dev_assets/cli-tools) and cleans up.
    #[cfg(unix)]
    #[tokio::test]
    #[ignore]
    async fn install_and_remove_archive_tools_end_to_end() {
        for id in [
            CliToolId::Op,
            CliToolId::Aws,
            CliToolId::Acli,
            CliToolId::Gws,
        ] {
            let e = entry(id);
            let installed = install(id).await.expect("install should succeed");
            let app = installed.app.expect("app copy should be reported");
            assert_eq!(app.version, e.version);

            let link = bin_link_path(id);
            let target = std::fs::read_link(&link).expect("bin symlink exists");
            assert!(
                target.starts_with(cli_tools_dir()),
                "symlink must point inside the app-owned dir"
            );
            let out = std::process::Command::new(&link)
                .args(e.version_args)
                .output()
                .expect("installed binary runs");
            assert!(out.status.success(), "{} --version failed", e.binary_name);
            let stdout = String::from_utf8_lossy(&out.stdout);
            assert!(
                stdout.contains(e.version),
                "{} reported '{}', expected it to contain {}",
                e.binary_name,
                stdout.trim(),
                e.version
            );
            // Idempotent re-install is a no-op success.
            install(id).await.expect("re-install is idempotent");
            // Staging left clean.
            assert!(
                !staging_dir(id).exists()
                    || std::fs::read_dir(staging_dir(id)).unwrap().next().is_none()
            );

            let removed = remove(id).await.expect("remove should succeed");
            assert!(removed.app.is_none());
            assert!(link.symlink_metadata().is_err(), "symlink must be gone");
            assert!(!tool_dir(id).exists(), "tool dir must be gone");
        }
    }

    #[cfg(unix)]
    #[test]
    fn extract_rejects_garbage_archives() {
        let dir = tempfile::tempdir().unwrap();
        let junk = dir.path().join("junk");
        std::fs::write(&junk, b"not an archive").unwrap();
        for kind in [ArchiveKind::Zip, ArchiveKind::TarGz, ArchiveKind::TarXz] {
            let out = dir.path().join(format!("out-{kind:?}"));
            std::fs::create_dir_all(&out).unwrap();
            assert!(
                extract(kind, &junk, &out).is_err(),
                "{kind:?} accepted junk"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn extract_tar_gz_preserves_executable_bit() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        // Build a tar.gz containing one executable file.
        let tar_gz = dir.path().join("t.tar.gz");
        {
            let f = std::fs::File::create(&tar_gz).unwrap();
            let enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
            let mut builder = tar::Builder::new(enc);
            let payload = b"#!/bin/sh\necho hi\n";
            let mut header = tar::Header::new_gnu();
            header.set_size(payload.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, "tool", payload.as_slice())
                .unwrap();
            builder.into_inner().unwrap().finish().unwrap();
        }
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        extract(ArchiveKind::TarGz, &tar_gz, &out).unwrap();
        let mode = std::fs::metadata(out.join("tool"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "executable bit lost");
    }
}
