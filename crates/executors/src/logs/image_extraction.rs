//! Persist image content blocks returned by MCP tools into a worktree's
//! `.vibe-attachments/` directory so they render inline in the chat.
//!
//! MCP tool results can carry `image` content blocks (a base64 payload plus a
//! MIME type). Executors otherwise collapse tool results to text or dump the
//! raw JSON (base64 and all), so the image is never shown. This module extracts
//! those blocks, writes the decoded bytes into the worktree, and produces a
//! Markdown rendering that references them as `![alt](.vibe-attachments/<file>)`.
//! The frontend's WYSIWYG chat renderer turns that Markdown into an inline
//! thumbnail (see `packages/ui/src/components/image-node.tsx`), and the backend
//! serves the file straight from the worktree — no DB record required.

use std::{fs, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use workspace_utils::path::VIBE_ATTACHMENTS_DIR;

/// Rewrite an MCP tool-result `content` value, persisting any image blocks and
/// interleaving text blocks with `![alt](.vibe-attachments/..)` references.
///
/// Accepts either an array of content blocks or a single block object. Returns
/// `None` when no image block was found (or none could be decoded/written) so
/// callers can keep their existing text/JSON normalization behaviour.
pub fn rewrite_content_with_images(
    worktree_path: &Path,
    content: &serde_json::Value,
) -> Option<String> {
    match content {
        serde_json::Value::Array(blocks) => rewrite_blocks_with_images(worktree_path, blocks),
        serde_json::Value::Object(_) => {
            rewrite_blocks_with_images(worktree_path, std::slice::from_ref(content))
        }
        _ => None,
    }
}

/// Like [`rewrite_content_with_images`] but for content already available as a
/// slice of blocks (e.g. Codex's `CallToolResult.content`).
pub fn rewrite_blocks_with_images(
    worktree_path: &Path,
    blocks: &[serde_json::Value],
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut found_image = false;

    for block in blocks {
        match block.get("type").and_then(|t| t.as_str()) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    let text = text.trim();
                    if !text.is_empty() {
                        parts.push(text.to_string());
                    }
                }
            }
            Some("image") => {
                if let Some((data, mime)) = extract_image_payload(block)
                    && let Some(rel_path) = persist_image(worktree_path, data, &mime)
                {
                    found_image = true;
                    parts.push(format!("![image]({rel_path})"));
                }
            }
            _ => {}
        }
    }

    if found_image {
        Some(parts.join("\n\n"))
    } else {
        None
    }
}

/// Pull the base64 payload and MIME type out of an `image` content block,
/// supporting both the MCP shape (`{data, mimeType}`) and the Anthropic shape
/// (`{source: {data, media_type}}`).
fn extract_image_payload(block: &serde_json::Value) -> Option<(&str, String)> {
    // MCP native: { "type": "image", "data": "..", "mimeType": ".." }
    if let Some(data) = block.get("data").and_then(|d| d.as_str()) {
        let mime = block
            .get("mimeType")
            .and_then(|m| m.as_str())
            .unwrap_or("image/png")
            .to_string();
        return Some((data, mime));
    }

    // Anthropic block: { "type": "image", "source": { "data": "..", "media_type": ".." } }
    if let Some(source) = block.get("source")
        && let Some(data) = source.get("data").and_then(|d| d.as_str())
    {
        let mime = source
            .get("media_type")
            .and_then(|m| m.as_str())
            .unwrap_or("image/png")
            .to_string();
        return Some((data, mime));
    }

    None
}

/// Decode a base64 image and write it into `<worktree>/.vibe-attachments/`,
/// returning the worktree-relative path (e.g. `.vibe-attachments/mcp-ab12.png`).
/// The filename is content-addressed so repeated screenshots dedupe.
fn persist_image(worktree_path: &Path, data_b64: &str, mime: &str) -> Option<String> {
    // Tolerate a data-URL prefix (`data:image/png;base64,....`).
    let payload = match data_b64.split_once(";base64,") {
        Some((_, rest)) => rest,
        None => data_b64,
    };

    let bytes = STANDARD.decode(payload.trim()).ok()?;
    if bytes.is_empty() {
        return None;
    }

    let hash = format!("{:x}", Sha256::digest(&bytes));
    let ext = extension_for_mime(mime);
    let filename = format!("mcp-{}.{ext}", &hash[..16]);

    let attachments_dir = worktree_path.join(VIBE_ATTACHMENTS_DIR);
    fs::create_dir_all(&attachments_dir).ok()?;

    // Match the attachment store convention: ignore everything in this dir.
    let gitignore = attachments_dir.join(".gitignore");
    if !gitignore.exists() {
        let _ = fs::write(&gitignore, "*\n");
    }

    let dest = attachments_dir.join(&filename);
    if !dest.exists() {
        fs::write(&dest, &bytes).ok()?;
    }

    Some(format!("{VIBE_ATTACHMENTS_DIR}/{filename}"))
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        "image/tiff" => "tiff",
        "image/x-icon" | "image/vnd.microsoft.icon" => "ico",
        _ => "png",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1x1 transparent PNG.
    const PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M8AAAMBAQDJ/pLvAAAAAElFTkSuQmCC";

    fn tmp_worktree(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vk-img-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn mcp_shape_image_is_persisted_and_referenced() {
        let wt = tmp_worktree("mcp");
        let content = serde_json::json!([
            { "type": "text", "text": "here is your screenshot" },
            { "type": "image", "data": PNG_B64, "mimeType": "image/png" }
        ]);

        let md = rewrite_content_with_images(&wt, &content).expect("should rewrite");
        assert!(md.contains("here is your screenshot"));

        // Extract the referenced path and assert the file physically exists.
        let rel = md
            .lines()
            .find_map(|l| {
                l.strip_prefix("![image](")
                    .and_then(|s| s.strip_suffix(")"))
            })
            .expect("image reference present");
        assert!(rel.starts_with(".vibe-attachments/mcp-"));
        assert!(rel.ends_with(".png"));
        assert!(wt.join(rel).exists(), "decoded image written to worktree");
        assert!(wt.join(".vibe-attachments/.gitignore").exists());
    }

    #[test]
    fn anthropic_source_shape_is_supported() {
        let wt = tmp_worktree("anthropic");
        let content = serde_json::json!([
            { "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": PNG_B64 } }
        ]);

        let md = rewrite_content_with_images(&wt, &content).expect("should rewrite");
        let rel = md
            .strip_prefix("![image](")
            .and_then(|s| s.strip_suffix(")"))
            .expect("image reference");
        assert!(rel.ends_with(".jpg"));
        assert!(wt.join(rel).exists());
    }

    #[test]
    fn text_only_result_returns_none() {
        let wt = tmp_worktree("textonly");
        let content = serde_json::json!([{ "type": "text", "text": "no images here" }]);
        assert!(rewrite_content_with_images(&wt, &content).is_none());
        // Nothing should have been written.
        assert!(!wt.join(".vibe-attachments").exists());
    }

    #[test]
    fn identical_images_dedupe_to_same_file() {
        let wt = tmp_worktree("dedupe");
        let content = serde_json::json!([
            { "type": "image", "data": PNG_B64, "mimeType": "image/png" },
            { "type": "image", "data": PNG_B64, "mimeType": "image/png" }
        ]);
        let md = rewrite_content_with_images(&wt, &content).unwrap();
        let refs: Vec<&str> = md
            .lines()
            .filter_map(|l| {
                l.strip_prefix("![image](")
                    .and_then(|s| s.strip_suffix(")"))
            })
            .collect();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0], refs[1], "content-addressed filenames dedupe");
    }
}
