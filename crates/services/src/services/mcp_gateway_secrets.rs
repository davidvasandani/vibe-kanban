//! Host-bound authenticated encryption for shared MCP credentials.
//!
//! Errors intentionally contain no plaintext, key bytes, or provider payloads.

use std::{fmt, fs::OpenOptions, io::Write, path::Path};

use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, Payload},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const VERSION: u8 = 1;

#[derive(Clone)]
pub struct McpGatewaySecretStore {
    cipher: Aes256Gcm,
}

impl fmt::Debug for McpGatewaySecretStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("McpGatewaySecretStore([REDACTED])")
    }
}

#[derive(Debug, Error)]
pub enum SecretStoreError {
    #[error("failed to access the MCP gateway host key")]
    KeyIo(#[source] std::io::Error),
    #[error("the MCP gateway host key is invalid")]
    InvalidKey,
    #[error("failed to encode an MCP credential envelope")]
    Encode,
    #[error("failed to decrypt the MCP credential envelope")]
    Decrypt,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    v: u8,
    nonce: String,
    ciphertext: String,
}

impl McpGatewaySecretStore {
    pub fn load_or_generate(path: &Path) -> Result<Self, SecretStoreError> {
        let key = match std::fs::read(path) {
            Ok(key) => key,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => create_key(path)?,
            Err(e) => return Err(SecretStoreError::KeyIo(e)),
        };
        if key.len() != 32 {
            return Err(SecretStoreError::InvalidKey);
        }
        Ok(Self {
            cipher: Aes256Gcm::new_from_slice(&key).map_err(|_| SecretStoreError::InvalidKey)?,
        })
    }

    pub fn encrypt(&self, plaintext: &[u8], binding: &[u8]) -> Result<String, SecretStoreError> {
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .encrypt((&nonce).into(), Payload { msg: plaintext, aad: binding })
            .map_err(|_| SecretStoreError::Encode)?;
        serde_json::to_string(&Envelope {
            v: VERSION,
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
        })
        .map_err(|_| SecretStoreError::Encode)
    }

    pub fn decrypt(&self, envelope: &str, binding: &[u8]) -> Result<Vec<u8>, SecretStoreError> {
        let envelope: Envelope =
            serde_json::from_str(envelope).map_err(|_| SecretStoreError::Decrypt)?;
        if envelope.v != VERSION {
            return Err(SecretStoreError::Decrypt);
        }
        let nonce = URL_SAFE_NO_PAD
            .decode(envelope.nonce)
            .map_err(|_| SecretStoreError::Decrypt)?;
        let ciphertext = URL_SAFE_NO_PAD
            .decode(envelope.ciphertext)
            .map_err(|_| SecretStoreError::Decrypt)?;
        if nonce.len() != 12 {
            return Err(SecretStoreError::Decrypt);
        }
        self.cipher
            .decrypt(nonce.as_slice().into(), Payload { msg: &ciphertext, aad: binding })
            .map_err(|_| SecretStoreError::Decrypt)
    }
}

fn create_key(path: &Path) -> Result<Vec<u8>, SecretStoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(SecretStoreError::KeyIo)?;
    }
    let mut key = vec![0_u8; 32];
    OsRng.fill_bytes(&mut key);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(&key).map_err(SecretStoreError::KeyIo)?;
            file.sync_all().map_err(SecretStoreError::KeyIo)?;
            Ok(key)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::read(path).map_err(SecretStoreError::KeyIo)
        }
        Err(e) => Err(SecretStoreError::KeyIo(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_is_bound_and_redacted() {
        let dir = tempfile::tempdir().unwrap();
        let store = McpGatewaySecretStore::load_or_generate(&dir.path().join("key")).unwrap();
        let envelope = store.encrypt(b"access-token", b"user|host|server").unwrap();
        assert!(!envelope.contains("access-token"));
        assert_eq!(store.decrypt(&envelope, b"user|host|server").unwrap(), b"access-token");
        assert!(store.decrypt(&envelope, b"other").is_err());
        assert!(!format!("{:?}", store).contains("access-token"));
    }
}
