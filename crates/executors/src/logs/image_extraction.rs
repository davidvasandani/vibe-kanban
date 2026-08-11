//! Persist image content blocks returned by MCP tools into a worktree's
//! `.vibe-attachments/` directory so they render inline in the chat.
//!
//! MCP tool results can carry `image` content blocks (a base64 payload plus a
//! MIME type) or hosted `resource_link` image blocks. Executors otherwise
//! collapse tool results to text or dump the raw JSON (base64 and all), so the
//! image is never shown. This module persists embedded images and produces a
//! Markdown rendering for both forms.
//! The frontend's WYSIWYG chat renderer turns that Markdown into an inline
//! thumbnail (see `packages/ui/src/components/image-node.tsx`), and the backend
//! serves the file straight from the worktree — no DB record required.

use std::{
    fs,
    io::Read,
    net::{IpAddr, SocketAddr},
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::Url;
use sha2::{Digest, Sha256};
use workspace_utils::path::VIBE_ATTACHMENTS_DIR;

const MAX_REMOTE_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
const REMOTE_IMAGE_TIMEOUT: Duration = Duration::from_secs(15);
const REMOTE_DNS_TIMEOUT: Duration = Duration::from_secs(5);
const REMOTE_IMPORT_BATCH_TIMEOUT: Duration = Duration::from_secs(21);
const MAX_REMOTE_IMAGES_PER_RESULT: usize = 8;
const ALLOWED_ORIGINS_ENV: &str = "VIBE_MCP_IMAGE_ALLOWED_ORIGINS";
const FIRECRAWL_URL_ENV: &str = "FIRECRAWL_BROWSER_URL";

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
    let mut hosted_indices = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| {
            block.get("type").and_then(|value| value.as_str()) == Some("resource_link")
                && block
                    .get("mimeType")
                    .and_then(|value| value.as_str())
                    .is_some_and(|mime| mime.to_ascii_lowercase().starts_with("image/"))
                && block
                    .get("uri")
                    .and_then(|value| value.as_str())
                    .and_then(|uri| Url::parse(uri).ok())
                    .is_some_and(|uri| matches!(uri.scheme(), "http" | "https"))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    hosted_indices.truncate(MAX_REMOTE_IMAGES_PER_RESULT);
    let mut hosted_imports = vec![None; blocks.len()];

    // A single result may contain several images. Import a bounded number in
    // parallel and apply one aggregate deadline so slow links cannot serialize
    // into minutes of stalled log processing.
    let (sender, receiver) = mpsc::channel();
    for index in &hosted_indices {
        let sender = sender.clone();
        let worktree_path = worktree_path.to_path_buf();
        let block = blocks[*index].clone();
        let index = *index;
        std::thread::spawn(move || {
            let imported = import_hosted_image(&worktree_path, &block);
            let _ = sender.send((index, imported));
        });
    }
    drop(sender);
    let deadline = Instant::now() + REMOTE_IMPORT_BATCH_TIMEOUT;
    for _ in 0..hosted_indices.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Ok((index, imported)) = receiver.recv_timeout(remaining) else {
            break;
        };
        hosted_imports[index] = imported;
    }

    for (index, block) in blocks.iter().enumerate() {
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
            Some("resource_link") => {
                if let Some(rel_path) = hosted_imports[index].take() {
                    found_image = true;
                    parts.push(format!("![image]({rel_path})"));
                } else if block
                    .get("mimeType")
                    .and_then(|value| value.as_str())
                    .is_some_and(|mime| mime.to_ascii_lowercase().starts_with("image/"))
                    && let Ok(json) = serde_json::to_string_pretty(block)
                {
                    parts.push(format!("```json\n{json}\n```"));
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

/// Run image normalization on Tokio's blocking pool so transient network
/// transfers never occupy an async runtime worker.
pub async fn rewrite_blocks_with_images_async(
    worktree_path: &Path,
    blocks: &[serde_json::Value],
) -> Option<String> {
    let worktree_path = worktree_path.to_path_buf();
    let blocks = blocks.to_vec();
    tokio::task::spawn_blocking(move || rewrite_blocks_with_images(&worktree_path, &blocks))
        .await
        .ok()
        .flatten()
}

/// Return whether a serialized executor event contains a hosted MCP image that
/// can trigger blocking import work during synchronous normalization.
pub fn contains_hosted_image_resource_link(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(values) => values.iter().any(contains_hosted_image_resource_link),
        serde_json::Value::Object(object) => {
            (object.get("type").and_then(|value| value.as_str()) == Some("resource_link")
                && object
                    .get("mimeType")
                    .and_then(|value| value.as_str())
                    .is_some_and(|mime| mime.to_ascii_lowercase().starts_with("image/")))
                || object.values().any(contains_hosted_image_resource_link)
        }
        _ => false,
    }
}

/// Import a transient hosted MCP image into the worktree attachment store.
///
/// Callers running on Tokio move normalization onto the blocking pool before a
/// hosted image can reach this synchronous transfer path.
fn import_hosted_image(worktree_path: &Path, block: &serde_json::Value) -> Option<String> {
    let mime = block.get("mimeType").and_then(|value| value.as_str())?;
    if !mime.to_ascii_lowercase().starts_with("image/") {
        return None;
    }

    let uri = block.get("uri").and_then(|value| value.as_str())?;
    let parsed = Url::parse(uri).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }

    download_and_persist_image(worktree_path, parsed, mime)
}

fn download_and_persist_image(
    worktree_path: &Path,
    uri: Url,
    _declared_mime: &str,
) -> Option<String> {
    let host = uri.host_str()?.to_string();
    let port = uri.port_or_known_default()?;
    let addresses = resolve_with_timeout(host.clone(), port)?;
    if addresses.is_empty() || (!origin_is_allowed(&uri) && addresses.iter().any(is_non_public)) {
        return None;
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(REMOTE_IMAGE_TIMEOUT)
        // Redirect destinations require their own DNS/address validation. MCP
        // artifact URLs are direct, so reject redirects instead of following
        // an unvalidated second destination.
        .redirect(reqwest::redirect::Policy::none())
        // Proxy-side DNS would bypass the address validation and pinning above.
        .no_proxy()
        .resolve_to_addrs(&host, &addresses)
        .build()
        .ok()?;
    let response = client.get(uri).send().ok()?;
    if response.status().is_redirection() || !response.status().is_success() {
        return None;
    }

    if response
        .content_length()
        .is_some_and(|size| size > MAX_REMOTE_IMAGE_BYTES)
    {
        return None;
    }

    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| value.to_ascii_lowercase().starts_with("image/"))?;

    let mut bytes = Vec::new();
    response
        .take(MAX_REMOTE_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_REMOTE_IMAGE_BYTES {
        return None;
    }

    // Do not trust the HTTP header alone. Persist only raster formats with an
    // unambiguous signature; in particular, remote SVG is excluded because it
    // is active XML content rather than a self-contained raster image.
    let mime = sniff_raster_image_mime(&bytes)?;
    persist_image_bytes(worktree_path, &bytes, mime)
}

fn resolve_with_timeout(host: String, port: u16) -> Option<Vec<SocketAddr>> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(vec![SocketAddr::new(ip, port)]);
    }

    use hickory_resolver::{TokioResolver, name_server::TokioConnectionProvider};

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime.block_on(async move {
        let resolver = TokioResolver::builder(TokioConnectionProvider::default())
            .ok()?
            .build();
        let lookup = tokio::time::timeout(REMOTE_DNS_TIMEOUT, resolver.lookup_ip(host))
            .await
            .ok()?
            .ok()?;
        Some(lookup.iter().map(|ip| SocketAddr::new(ip, port)).collect())
    })
}

fn sniff_raster_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        Some("image/tiff")
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else {
        None
    }
}

fn origin_is_allowed(uri: &Url) -> bool {
    #[cfg(test)]
    if uri
        .host_str()
        .and_then(|host| host.parse::<IpAddr>().ok())
        .is_some_and(|ip| ip.is_loopback())
    {
        return true;
    }

    [ALLOWED_ORIGINS_ENV, FIRECRAWL_URL_ENV]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .flat_map(|origins| {
            origins
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .filter_map(|origin| Url::parse(origin).ok())
                .collect::<Vec<_>>()
        })
        .any(|allowed| {
            allowed.scheme() == uri.scheme()
                && allowed.host_str() == uri.host_str()
                && allowed.port_or_known_default() == uri.port_or_known_default()
        })
}

fn is_non_public(address: &SocketAddr) -> bool {
    match address.ip() {
        IpAddr::V4(ip) => is_non_public_ipv4(ip),
        IpAddr::V6(ip) => {
            ip.to_ipv4_mapped().is_some_and(is_non_public_ipv4)
                || ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
                || ip.segments()[0] & 0xe000 != 0x2000
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8)
        }
    }
}

fn is_non_public_ipv4(ip: std::net::Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || a == 0
        || a >= 240
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 88 && c == 99)
        || (a == 198 && (18..=19).contains(&b))
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

    persist_image_bytes(worktree_path, &bytes, mime)
}

fn persist_image_bytes(worktree_path: &Path, bytes: &[u8], mime: &str) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }

    let hash = format!("{:x}", Sha256::digest(bytes));
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
        fs::write(&dest, bytes).ok()?;
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
    use std::{
        io::{Read as _, Write as _},
        net::TcpListener,
    };

    use super::*;

    // 1x1 transparent PNG.
    const PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M8AAAMBAQDJ/pLvAAAAAElFTkSuQmCC";

    fn tmp_worktree(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vk-img-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn serve_once(
        status: &str,
        headers: &[(&str, &str)],
        body: &'static [u8],
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let headers = headers
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<Vec<_>>();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            write!(stream, "HTTP/1.1 {status}\r\n").unwrap();
            let has_content_length = headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("content-length"));
            for (name, value) in headers {
                write!(stream, "{name}: {value}\r\n").unwrap();
            }
            if !has_content_length {
                write!(stream, "Content-Length: {}\r\n", body.len()).unwrap();
            }
            write!(stream, "Connection: close\r\n\r\n").unwrap();
            stream.write_all(body).unwrap();
        });
        (format!("http://{address}/screenshot"), handle)
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

    #[test]
    fn hosted_image_resource_link_is_imported_and_rendered_locally() {
        let wt = tmp_worktree("resource-link");
        let bytes = STANDARD.decode(PNG_B64).unwrap().leak();
        let (uri, server) = serve_once("200 OK", &[("Content-Type", "image/png")], bytes);
        let content = serde_json::json!([
            { "type": "text", "text": "captured page" },
            {
                "type": "resource_link",
                "uri": uri,
                "mimeType": "image/jpeg",
                "name": "screenshot"
            },
            {
                "type": "resource_link",
                "uri": "http://127.0.0.1:1/expired.jpg",
                "mimeType": "image/jpeg"
            }
        ]);

        let md = rewrite_content_with_images(&wt, &content).expect("should rewrite");
        server.join().unwrap();
        assert!(md.starts_with("captured page\n\n![image](.vibe-attachments/mcp-"));
        let rel = md
            .lines()
            .find_map(|line| line.strip_prefix("![image](")?.strip_suffix(')'))
            .unwrap();
        assert!(rel.ends_with(".png"));
        assert_eq!(fs::read(wt.join(rel)).unwrap(), bytes);
        assert!(md.contains("http://127.0.0.1:1/expired.jpg"));
    }

    #[test]
    fn unsafe_or_non_image_resource_links_are_not_rewritten() {
        let wt = tmp_worktree("rejected-resource-links");
        for content in [
            serde_json::json!([{
                "type": "resource_link",
                "uri": "file:///tmp/screenshot.jpg",
                "mimeType": "image/jpeg"
            }]),
            serde_json::json!([{
                "type": "resource_link",
                "uri": "https://artifacts.example/report.json",
                "mimeType": "application/json"
            }]),
        ] {
            assert!(rewrite_content_with_images(&wt, &content).is_none());
        }
    }

    #[test]
    fn failed_or_non_image_download_is_not_rewritten() {
        let wt = tmp_worktree("failed-download");
        let (uri, server) = serve_once("200 OK", &[("Content-Type", "application/json")], b"{}");
        let content = serde_json::json!([{
            "type": "resource_link",
            "uri": uri,
            "mimeType": "image/jpeg"
        }]);
        assert!(rewrite_content_with_images(&wt, &content).is_none());
        server.join().unwrap();

        let bytes = STANDARD.decode(PNG_B64).unwrap().leak();
        let (uri, server) = serve_once(
            "302 Found",
            &[("Content-Type", "image/png"), ("Location", "/next")],
            bytes,
        );
        let content = serde_json::json!([{
            "type": "resource_link",
            "uri": uri,
            "mimeType": "image/png"
        }]);
        assert!(rewrite_content_with_images(&wt, &content).is_none());
        server.join().unwrap();

        let (uri, server) = serve_once(
            "200 OK",
            &[("Content-Type", "image/png")],
            b"not actually an image",
        );
        let content = serde_json::json!([{
            "type": "resource_link",
            "uri": uri,
            "mimeType": "image/png"
        }]);
        assert!(rewrite_content_with_images(&wt, &content).is_none());
        server.join().unwrap();

        let content = serde_json::json!([{
            "type": "resource_link",
            "uri": "http://127.0.0.1:1/missing.jpg",
            "mimeType": "image/jpeg"
        }]);
        assert!(rewrite_content_with_images(&wt, &content).is_none());
        assert!(!wt.join(".vibe-attachments").exists());
    }

    #[test]
    fn oversized_hosted_image_is_not_persisted() {
        let wt = tmp_worktree("oversized-download");
        let oversized = (MAX_REMOTE_IMAGE_BYTES + 1).to_string();
        let (uri, server) = serve_once(
            "200 OK",
            &[
                ("Content-Type", "image/jpeg"),
                ("Content-Length", &oversized),
            ],
            b"x",
        );
        let content = serde_json::json!([{
            "type": "resource_link",
            "uri": uri,
            "mimeType": "image/jpeg"
        }]);
        assert!(rewrite_content_with_images(&wt, &content).is_none());
        server.join().unwrap();
        assert!(!wt.join(".vibe-attachments").exists());
    }

    #[test]
    fn private_and_ipv4_mapped_private_addresses_are_non_public() {
        for address in [
            "127.0.0.1:80",
            "10.0.0.1:80",
            "[::1]:80",
            "[::ffff:127.0.0.1]:80",
            "[::ffff:10.0.0.1]:80",
            "100.64.0.1:80",
            "198.18.0.1:80",
            "0.1.2.3:80",
            "[2001:db8::1]:80",
        ] {
            assert!(is_non_public(&address.parse().unwrap()), "{address}");
        }
        assert!(!is_non_public(&"1.1.1.1:443".parse().unwrap()));
    }
}
