use directories::ProjectDirs;
use rust_embed::RustEmbed;

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

pub fn asset_dir() -> std::path::PathBuf {
    let path = if cfg!(debug_assertions) {
        std::path::PathBuf::from(PROJECT_ROOT).join("../../dev_assets")
    } else {
        prod_asset_dir_path()
    };

    // Ensure the directory exists
    if !path.exists() {
        std::fs::create_dir_all(&path).expect("Failed to create asset directory");
    }

    path
    // ✔ macOS → ~/Library/Application Support/MyApp
    // ✔ Linux → ~/.local/share/myapp   (respects XDG_DATA_HOME)
    // ✔ Windows → %APPDATA%\Example\MyApp
}

pub fn prod_asset_dir_path() -> std::path::PathBuf {
    ProjectDirs::from("ai", "bloop", "vibe-kanban")
        .expect("OS didn't give us a home directory")
        .data_dir()
        .to_path_buf()
}

pub fn config_path() -> std::path::PathBuf {
    asset_dir().join("config.json")
}

/// App-owned directory for CLI tools installed by the CLI tool manager.
/// Its `bin/` subdirectory is the only path exposed on workspace processes'
/// PATH.
pub fn cli_tools_dir() -> std::path::PathBuf {
    if let Some(path) = cli_tools_dir_override(std::env::var_os("VIBE_KANBAN_CLI_TOOLS_DIR")) {
        return path;
    }
    asset_dir().join("cli-tools")
}

fn cli_tools_dir_override(value: Option<std::ffi::OsString>) -> Option<std::path::PathBuf> {
    value
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::cli_tools_dir_override;

    #[test]
    fn cli_tools_override_requires_a_nonempty_absolute_path() {
        assert_eq!(
            cli_tools_dir_override(Some("/srv/shared/cli-tools".into())),
            Some("/srv/shared/cli-tools".into())
        );
        assert_eq!(
            cli_tools_dir_override(Some("relative/cli-tools".into())),
            None
        );
        assert_eq!(cli_tools_dir_override(Some("".into())), None);
        assert_eq!(cli_tools_dir_override(None), None);
    }
}

pub fn profiles_path() -> std::path::PathBuf {
    asset_dir().join("profiles.json")
}

pub fn credentials_path() -> std::path::PathBuf {
    asset_dir().join("credentials.json")
}

pub fn trusted_keys_path() -> std::path::PathBuf {
    asset_dir().join("trusted_ed25519_public_keys.json")
}

pub fn server_signing_key_path() -> std::path::PathBuf {
    asset_dir().join("server_ed25519_signing_key")
}

/// Host-local key used to encrypt shared MCP credentials stored in SQLite.
pub fn mcp_gateway_key_path() -> std::path::PathBuf {
    asset_dir().join("mcp_gateway_aead_key")
}

pub fn relay_host_credentials_path() -> std::path::PathBuf {
    asset_dir().join("relay_host_credentials.json")
}

#[derive(RustEmbed)]
#[folder = "../../assets/sounds"]
pub struct SoundAssets;

#[derive(RustEmbed)]
#[folder = "../../assets/scripts"]
pub struct ScriptAssets;
