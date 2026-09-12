use std::process::Command;

pub const BUILD_GIT_SHA_ENV: &str = "VK_BUILD_GIT_SHA";

fn validate_full_sha(value: &str) -> Result<&str, String> {
    if value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(value)
    } else {
        Err(format!(
            "{BUILD_GIT_SHA_ENV} must be exactly 40 lowercase hexadecimal characters"
        ))
    }
}

pub fn select_short_sha(
    explicit_sha: Option<&str>,
    workspace_root: &std::path::Path,
) -> Result<Option<String>, String> {
    if let Some(value) = explicit_sha {
        let full_sha = validate_full_sha(value)?;
        return Ok(Some(full_sha[..7].to_owned()));
    }

    Ok(Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(workspace_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty()))
}
