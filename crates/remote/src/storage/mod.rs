//! Backend-agnostic blob storage for issue/comment attachments.
//!
//! The [`BlobStorage`] trait abstracts over concrete backends. The presigned
//! upload/read URL model mirrors what an object store (S3/GCS/Azure) exposes,
//! so a future cloud backend can hand the browser a direct URL, while the
//! bundled [`LocalDiskStorage`] serves those URLs from the API itself via
//! HMAC-signed `/v1/blobs/*` routes.

mod local_disk;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
pub use local_disk::LocalDiskStorage;
use sha2::Sha256;

/// A presigned upload target handed to the client.
#[derive(Debug)]
pub struct PresignedUpload {
    pub upload_url: String,
    pub blob_path: String,
    pub expires_at: DateTime<Utc>,
}

/// Server-visible metadata for a stored blob.
#[derive(Debug)]
pub struct BlobProperties {
    pub content_length: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("storage io error: {0}")]
    Io(String),
    #[error("blob not found: {0}")]
    NotFound(String),
    #[error("invalid blob path: {0}")]
    InvalidPath(String),
    #[error("url signing error: {0}")]
    Signing(String),
}

#[async_trait]
pub trait BlobStorage: Send + Sync {
    /// Presigned URL the browser `PUT`s the file bytes to.
    fn create_upload_url(&self, blob_path: &str) -> Result<PresignedUpload, StorageError>;

    /// Short-lived presigned URL used directly as an `<img src>` / download link.
    fn create_read_url(&self, blob_path: &str) -> Result<String, StorageError>;

    async fn get_blob_properties(&self, blob_path: &str) -> Result<BlobProperties, StorageError>;

    async fn download_blob(&self, blob_path: &str) -> Result<Vec<u8>, StorageError>;

    async fn upload_blob(
        &self,
        blob_path: &str,
        data: Vec<u8>,
        content_type: String,
    ) -> Result<(), StorageError>;

    async fn delete_blob(&self, blob_path: &str) -> Result<(), StorageError>;

    /// Downcast hook for the local-disk backend, whose presigned URLs are served
    /// by our own `/v1/blobs/*` routes. Cloud backends return `None` because
    /// their URLs point straight at the object store.
    fn local_disk(&self) -> Option<&LocalDiskStorage> {
        None
    }
}

/// Derive a dedicated, domain-separated HMAC key for signing blob URLs from an
/// existing secret (the remote JWT secret), so no new env var is required.
pub fn derive_signing_key(secret: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts keys of any length");
    mac.update(b"attachments-url-signing");
    mac.finalize().into_bytes().to_vec()
}
