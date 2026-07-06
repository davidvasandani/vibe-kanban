use std::path::{Path, PathBuf};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tokio::fs;

use super::{BlobProperties, BlobStorage, PresignedUpload, StorageError};

type HmacSha256 = Hmac<Sha256>;

/// Sidecar file suffix that stores a blob's content type alongside the bytes.
const CONTENT_TYPE_SUFFIX: &str = ".ct";

/// Filesystem-backed [`BlobStorage`]. Presigned URLs are HMAC-signed links to
/// this server's own `/v1/blobs/*` routes; the browser uploads/reads there just
/// as it would against a cloud object store.
pub struct LocalDiskStorage {
    data_dir: PathBuf,
    /// Public origin used to build absolute URLs (no trailing slash).
    public_base_url: String,
    signing_key: Vec<u8>,
    presign_expiry: chrono::Duration,
}

impl LocalDiskStorage {
    pub fn new(
        data_dir: PathBuf,
        public_base_url: String,
        signing_key: Vec<u8>,
        presign_expiry_secs: u64,
    ) -> Self {
        Self {
            data_dir,
            public_base_url: public_base_url.trim_end_matches('/').to_string(),
            signing_key,
            presign_expiry: chrono::Duration::seconds(presign_expiry_secs as i64),
        }
    }

    fn sign(&self, op: &str, blob_path: &str, exp: i64) -> String {
        let mut mac =
            HmacSha256::new_from_slice(&self.signing_key).expect("HMAC accepts keys of any length");
        mac.update(op.as_bytes());
        mac.update(b"\n");
        mac.update(blob_path.as_bytes());
        mac.update(b"\n");
        mac.update(exp.to_string().as_bytes());
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }

    fn signed_url(&self, op: &str, blob_path: &str, expires_at: DateTime<Utc>) -> String {
        let exp = expires_at.timestamp();
        let sig = self.sign(op, blob_path, exp);
        format!(
            "{}/v1/blobs/{}?op={}&exp={}&sig={}",
            self.public_base_url,
            encode_path(blob_path),
            op,
            exp,
            sig
        )
    }

    /// Verify a signed `/v1/blobs/*` request. `op` is `"put"` or `"get"`.
    pub fn verify_signature(
        &self,
        op: &str,
        blob_path: &str,
        exp: i64,
        sig: &str,
    ) -> Result<(), StorageError> {
        if Utc::now().timestamp() > exp {
            return Err(StorageError::Signing("signature expired".to_string()));
        }
        let expected = self.sign(op, blob_path, exp);
        // Constant-time comparison to avoid signature-forgery timing oracles.
        if bool::from(expected.as_bytes().ct_eq(sig.as_bytes())) {
            Ok(())
        } else {
            Err(StorageError::Signing("invalid signature".to_string()))
        }
    }

    /// Write blob bytes, persisting the content type in a sidecar file so reads
    /// can serve a correct `Content-Type` for `<img src>`.
    pub async fn write_blob(
        &self,
        blob_path: &str,
        content_type: Option<&str>,
        data: &[u8],
    ) -> Result<(), StorageError> {
        let path = self.resolve(blob_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| StorageError::Io(e.to_string()))?;
        }
        fs::write(&path, data)
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        if let Some(ct) = content_type {
            let ct_path = self.content_type_path(blob_path)?;
            if let Some(parent) = ct_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| StorageError::Io(e.to_string()))?;
            }
            fs::write(&ct_path, ct.as_bytes())
                .await
                .map_err(|e| StorageError::Io(e.to_string()))?;
        }
        Ok(())
    }

    /// Read blob bytes plus a resolved content type (sidecar, else inferred).
    pub async fn read_blob(&self, blob_path: &str) -> Result<(Vec<u8>, String), StorageError> {
        let path = self.resolve(blob_path)?;
        let data = match fs::read(&path).await {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(StorageError::NotFound(blob_path.to_string()));
            }
            Err(e) => return Err(StorageError::Io(e.to_string())),
        };
        let content_type = self.read_content_type(blob_path).await;
        Ok((data, content_type))
    }

    async fn read_content_type(&self, blob_path: &str) -> String {
        if let Ok(ct_path) = self.content_type_path(blob_path)
            && let Ok(ct) = fs::read_to_string(&ct_path).await
        {
            let trimmed = ct.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        infer_content_type(blob_path)
    }

    /// Blob bytes live under `<data_dir>/blobs/…`.
    fn resolve(&self, blob_path: &str) -> Result<PathBuf, StorageError> {
        safe_join(&self.data_dir.join("blobs"), blob_path)
    }

    /// Content-type sidecars live under a SEPARATE `<data_dir>/meta/…` tree, so a
    /// blob literally named `x.ct` can never collide with the sidecar of `x`.
    fn content_type_path(&self, blob_path: &str) -> Result<PathBuf, StorageError> {
        safe_join(
            &self.data_dir.join("meta"),
            &format!("{blob_path}{CONTENT_TYPE_SUFFIX}"),
        )
    }
}

#[async_trait]
impl BlobStorage for LocalDiskStorage {
    fn create_upload_url(&self, blob_path: &str) -> Result<PresignedUpload, StorageError> {
        validate_blob_path(blob_path)?;
        let expires_at = Utc::now() + self.presign_expiry;
        Ok(PresignedUpload {
            upload_url: self.signed_url("put", blob_path, expires_at),
            blob_path: blob_path.to_string(),
            expires_at,
        })
    }

    fn create_read_url(&self, blob_path: &str) -> Result<String, StorageError> {
        validate_blob_path(blob_path)?;
        // Read URLs are always short-lived, independent of the upload window.
        let expires_at = Utc::now() + chrono::Duration::minutes(5);
        Ok(self.signed_url("get", blob_path, expires_at))
    }

    async fn get_blob_properties(&self, blob_path: &str) -> Result<BlobProperties, StorageError> {
        let path = self.resolve(blob_path)?;
        let meta = match fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(StorageError::NotFound(blob_path.to_string()));
            }
            Err(e) => return Err(StorageError::Io(e.to_string())),
        };
        Ok(BlobProperties {
            content_length: meta.len() as i64,
        })
    }

    async fn download_blob(&self, blob_path: &str) -> Result<Vec<u8>, StorageError> {
        let (data, _content_type) = self.read_blob(blob_path).await?;
        if data.is_empty() {
            return Err(StorageError::NotFound(blob_path.to_string()));
        }
        Ok(data)
    }

    async fn upload_blob(
        &self,
        blob_path: &str,
        data: Vec<u8>,
        content_type: String,
    ) -> Result<(), StorageError> {
        self.write_blob(blob_path, Some(&content_type), &data).await
    }

    async fn delete_blob(&self, blob_path: &str) -> Result<(), StorageError> {
        let path = self.resolve(blob_path)?;
        match fs::remove_file(&path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(StorageError::Io(e.to_string())),
        }
        if let Ok(ct_path) = self.content_type_path(blob_path) {
            let _ = fs::remove_file(&ct_path).await;
        }
        Ok(())
    }

    fn local_disk(&self) -> Option<&LocalDiskStorage> {
        Some(self)
    }
}

/// Reject anything that could escape the data dir. Blob paths are server-
/// generated (`attachments/{uuid}/...`, `thumbnails/...`), but validate anyway.
fn validate_blob_path(blob_path: &str) -> Result<(), StorageError> {
    if blob_path.is_empty() || blob_path.starts_with('/') || blob_path.contains('\0') {
        return Err(StorageError::InvalidPath(blob_path.to_string()));
    }
    for component in blob_path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(StorageError::InvalidPath(blob_path.to_string()));
        }
    }
    Ok(())
}

fn safe_join(root: &Path, blob_path: &str) -> Result<PathBuf, StorageError> {
    validate_blob_path(blob_path)?;
    let mut path = root.to_path_buf();
    for component in blob_path.split('/') {
        path.push(component);
    }
    Ok(path)
}

/// Percent-encode each path segment while preserving `/` separators, so the
/// signed URL round-trips back to the exact `blob_path` after axum decodes it.
fn encode_path(blob_path: &str) -> String {
    blob_path
        .split('/')
        .map(|seg| urlencoding::encode(seg).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn infer_content_type(blob_path: &str) -> String {
    let ext = Path::new(blob_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn store(dir: PathBuf) -> LocalDiskStorage {
        LocalDiskStorage::new(
            dir,
            "https://example.test/".to_string(),
            b"test-signing-key".to_vec(),
            3600,
        )
    }

    #[test]
    fn sign_verify_round_trip() {
        let s = store(PathBuf::from("/tmp/does-not-matter"));
        let exp = (Utc::now() + chrono::Duration::minutes(5)).timestamp();
        let sig = s.sign("put", "attachments/a/b.png", exp);
        assert!(
            s.verify_signature("put", "attachments/a/b.png", exp, &sig)
                .is_ok()
        );
    }

    #[test]
    fn verify_rejects_expired() {
        let s = store(PathBuf::from("/tmp/does-not-matter"));
        let exp = (Utc::now() - chrono::Duration::seconds(1)).timestamp();
        let sig = s.sign("get", "attachments/a/b.png", exp);
        assert!(
            s.verify_signature("get", "attachments/a/b.png", exp, &sig)
                .is_err()
        );
    }

    #[test]
    fn verify_rejects_wrong_op() {
        let s = store(PathBuf::from("/tmp/does-not-matter"));
        let exp = (Utc::now() + chrono::Duration::minutes(5)).timestamp();
        let sig = s.sign("put", "attachments/a/b.png", exp);
        // A put signature must not authorize a get and vice versa.
        assert!(
            s.verify_signature("get", "attachments/a/b.png", exp, &sig)
                .is_err()
        );
    }

    #[test]
    fn verify_rejects_tampered_path() {
        let s = store(PathBuf::from("/tmp/does-not-matter"));
        let exp = (Utc::now() + chrono::Duration::minutes(5)).timestamp();
        let sig = s.sign("get", "attachments/a/b.png", exp);
        assert!(
            s.verify_signature("get", "attachments/a/evil.png", exp, &sig)
                .is_err()
        );
    }

    #[test]
    fn path_traversal_rejected() {
        assert!(validate_blob_path("../etc/passwd").is_err());
        assert!(validate_blob_path("attachments/../../etc").is_err());
        assert!(validate_blob_path("/abs/path").is_err());
        assert!(validate_blob_path("").is_err());
        assert!(validate_blob_path("attachments/ok/file.png").is_ok());
    }

    #[tokio::test]
    async fn write_read_delete_round_trip() {
        let dir = tempdir().unwrap();
        let s = store(dir.path().to_path_buf());
        let blob = "attachments/proj/xyz_file.png";

        s.write_blob(blob, Some("image/png"), b"hello")
            .await
            .unwrap();

        let (data, ct) = s.read_blob(blob).await.unwrap();
        assert_eq!(data, b"hello");
        assert_eq!(ct, "image/png");

        let props = s.get_blob_properties(blob).await.unwrap();
        assert_eq!(props.content_length, 5);

        s.delete_blob(blob).await.unwrap();
        assert!(matches!(
            s.read_blob(blob).await,
            Err(StorageError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn ct_named_blob_does_not_collide_with_sidecar() {
        // A blob literally named `foo.ct` must not share storage with the
        // content-type sidecar of a blob named `foo` (separate blobs/ vs meta/).
        let dir = tempdir().unwrap();
        let s = store(dir.path().to_path_buf());

        s.write_blob("attachments/p/foo", Some("image/png"), b"real")
            .await
            .unwrap();
        s.write_blob("attachments/p/foo.ct", Some("image/gif"), b"decoy")
            .await
            .unwrap();

        let (foo, foo_ct) = s.read_blob("attachments/p/foo").await.unwrap();
        assert_eq!(foo, b"real");
        assert_eq!(foo_ct, "image/png");

        let (decoy, decoy_ct) = s.read_blob("attachments/p/foo.ct").await.unwrap();
        assert_eq!(decoy, b"decoy");
        assert_eq!(decoy_ct, "image/gif");
    }

    #[tokio::test]
    async fn read_infers_content_type_without_sidecar() {
        let dir = tempdir().unwrap();
        let s = store(dir.path().to_path_buf());
        let blob = "attachments/proj/pic.jpg";
        s.write_blob(blob, None, b"bytes").await.unwrap();
        let (_data, ct) = s.read_blob(blob).await.unwrap();
        assert_eq!(ct, "image/jpeg");
    }
}
