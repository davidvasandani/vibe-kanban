use std::{fs, path::Path, process::Command};

fn main() {
    // Load .env from the workspace root
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let env_file = workspace_root.join(".env");
    dotenv::from_path(&env_file).ok();

    // Re-run build script when these env vars or .env file change
    println!("cargo:rerun-if-env-changed=POSTHOG_API_KEY");
    println!("cargo:rerun-if-env-changed=POSTHOG_API_ENDPOINT");
    println!("cargo:rerun-if-env-changed=VK_SHARED_API_BASE");
    println!("cargo:rerun-if-env-changed=SENTRY_DSN");
    if env_file.exists() {
        println!("cargo:rerun-if-changed={}", env_file.display());
    }

    // Capture the short git commit hash at build time so the running app can
    // display the exact deployed revision. Re-run whenever HEAD moves.
    let git_dir = workspace_root.join(".git");
    if git_dir.exists() {
        println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
        if let Ok(head) = fs::read_to_string(git_dir.join("HEAD"))
            && let Some(reference) = head.strip_prefix("ref: ").map(str::trim)
        {
            let ref_path = git_dir.join(reference);
            if ref_path.exists() {
                println!("cargo:rerun-if-changed={}", ref_path.display());
            }
        }
    }
    let git_sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&workspace_root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(sha) = git_sha {
        println!("cargo:rustc-env=VK_GIT_SHA={}", sha);
    }

    // Ensure build script re-runs when these env vars change
    println!("cargo:rerun-if-env-changed=POSTHOG_API_KEY");
    println!("cargo:rerun-if-env-changed=POSTHOG_API_ENDPOINT");
    println!("cargo:rerun-if-env-changed=VK_SHARED_API_BASE");

    if let Ok(api_key) = std::env::var("POSTHOG_API_KEY") {
        println!("cargo:rustc-env=POSTHOG_API_KEY={}", api_key);
    }
    if let Ok(api_endpoint) = std::env::var("POSTHOG_API_ENDPOINT") {
        println!("cargo:rustc-env=POSTHOG_API_ENDPOINT={}", api_endpoint);
    }
    if let Ok(vk_shared_api_base) = std::env::var("VK_SHARED_API_BASE") {
        println!("cargo:rustc-env=VK_SHARED_API_BASE={}", vk_shared_api_base);
    }
    if let Ok(vk_shared_relay_api_base) = std::env::var("VK_SHARED_RELAY_API_BASE") {
        println!(
            "cargo:rustc-env=VK_SHARED_RELAY_API_BASE={}",
            vk_shared_relay_api_base
        );
    }

    // Create packages/local-web/dist directory if it doesn't exist
    let dist_path = Path::new("../../packages/local-web/dist");
    if !dist_path.exists() {
        println!("cargo:warning=Creating dummy packages/local-web/dist directory for compilation");
        fs::create_dir_all(dist_path).unwrap();

        // Create a dummy index.html
        let dummy_html = r#"<!DOCTYPE html>
<html><head><title>Build web app first</title></head>
<body><h1>Please build @vibe/local-web first</h1></body></html>"#;

        fs::write(dist_path.join("index.html"), dummy_html).unwrap();
    }
}
