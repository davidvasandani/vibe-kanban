use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use cluster_protocol::{
    CODEX_ROLLOUT_MAX_FILE_BYTES, CODEX_ROLLOUT_MAX_LINEAGE_BYTES, CODEX_ROLLOUT_MAX_LINEAGE_FILES,
    CodexRolloutArtifact, CodexRolloutManifest, CodexRolloutManifestEntry, CodexRolloutStageResult,
    CodexRolloutVerification,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const MAX_METADATA_LINE_BYTES: u64 = 1024 * 1024;
const PARTIAL_RETENTION: Duration = Duration::from_secs(24 * 60 * 60);
const VERIFIED_RETENTION: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const MAX_CLEANUP_FILES: usize = 100;
const RECEIPT_DIR: &str = ".vibe-kanban-transfers";

#[derive(Debug, Error)]
pub enum RolloutTransferError {
    #[error("Codex sessions directory is unavailable")]
    SessionsUnavailable,
    #[error("rollout for thread {0} was not found")]
    Missing(Uuid),
    #[error("rollout path is not safely contained in the Codex sessions directory")]
    UnsafePath,
    #[error("rollout for thread {thread_id} has invalid canonical metadata: {reason}")]
    InvalidMetadata {
        thread_id: Uuid,
        reason: &'static str,
    },
    #[error("rollout lineage for thread {0} is cyclic")]
    Cycle(Uuid),
    #[error("rollout {thread_id} exceeds the {limit} byte limit")]
    FileTooLarge { thread_id: Uuid, limit: u64 },
    #[error("rollout lineage exceeds its configured limits")]
    LineageTooLarge,
    #[error("rollout for thread {0} changed while it was being transferred")]
    SourceChanged(Uuid),
    #[error("rollout checksum does not match the authorized manifest for thread {0}")]
    ChecksumMismatch(Uuid),
    #[error("target already has conflicting rollout content for thread {0}")]
    TargetConflict(Uuid),
    #[error("transfer manifest is invalid")]
    InvalidManifest,
    #[error("rollout transfer I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("rollout transfer encoding failed")]
    Encoding,
}

#[derive(Debug, Deserialize)]
struct RolloutEnvelope {
    #[serde(rename = "type")]
    kind: String,
    payload: CanonicalMeta,
}

#[derive(Debug, Deserialize)]
struct CanonicalMeta {
    id: Uuid,
    #[serde(default)]
    forked_from_id: Option<Uuid>,
    #[serde(default)]
    parent_thread_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TransferReceipt {
    thread_id: Uuid,
    relative_path: String,
    sha256: String,
    last_needed_unix_seconds: u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RolloutCleanupResult {
    pub partials_removed: usize,
    pub verified_removed: usize,
}

#[derive(Debug, Clone)]
pub struct CodexRolloutStore {
    sessions_root: PathBuf,
    receipts_root: PathBuf,
}

impl CodexRolloutStore {
    pub fn new(codex_home: impl AsRef<Path>) -> Result<Self, RolloutTransferError> {
        let sessions = codex_home.as_ref().join("sessions");
        fs::create_dir_all(&sessions)?;
        let sessions_root = sessions
            .canonicalize()
            .map_err(|_| RolloutTransferError::SessionsUnavailable)?;
        let receipts_root = sessions_root.join(RECEIPT_DIR);
        fs::create_dir_all(&receipts_root)?;
        set_private_directory_permissions(&receipts_root)?;
        Ok(Self {
            sessions_root,
            receipts_root,
        })
    }

    pub fn resolve_manifest(
        &self,
        operation_id: Uuid,
        workspace_id: Uuid,
        source_execution_id: Uuid,
        source_worker_node_id: Uuid,
        target_worker_node_id: Uuid,
        leaf_thread_id: Uuid,
    ) -> Result<CodexRolloutManifest, RolloutTransferError> {
        let index = self.index_rollouts()?;
        let mut entries = Vec::new();
        let mut visiting = HashSet::new();
        let mut total = 0u64;
        self.resolve_ancestors(
            leaf_thread_id,
            &index,
            &mut visiting,
            &mut entries,
            &mut total,
        )?;
        let mut manifest = CodexRolloutManifest {
            operation_id,
            workspace_id,
            source_execution_id,
            source_worker_node_id,
            target_worker_node_id,
            leaf_thread_id,
            entries,
            manifest_sha256: String::new(),
        };
        manifest.manifest_sha256 = manifest_digest(&manifest);
        Ok(manifest)
    }

    fn resolve_ancestors(
        &self,
        thread_id: Uuid,
        index: &HashMap<Uuid, PathBuf>,
        visiting: &mut HashSet<Uuid>,
        entries: &mut Vec<CodexRolloutManifestEntry>,
        total: &mut u64,
    ) -> Result<(), RolloutTransferError> {
        if entries.iter().any(|entry| entry.thread_id == thread_id) {
            return Ok(());
        }
        if !visiting.insert(thread_id) {
            return Err(RolloutTransferError::Cycle(thread_id));
        }
        if visiting.len() > CODEX_ROLLOUT_MAX_LINEAGE_FILES {
            return Err(RolloutTransferError::LineageTooLarge);
        }
        let path = index
            .get(&thread_id)
            .ok_or(RolloutTransferError::Missing(thread_id))?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(RolloutTransferError::UnsafePath);
        }
        if metadata.len() > CODEX_ROLLOUT_MAX_FILE_BYTES {
            return Err(RolloutTransferError::FileTooLarge {
                thread_id,
                limit: CODEX_ROLLOUT_MAX_FILE_BYTES,
            });
        }
        let meta = read_canonical_meta(path, thread_id)?;
        let parent = meta.parent_thread_id.or(meta.forked_from_id);
        if let Some(parent_id) = parent {
            self.resolve_ancestors(parent_id, index, visiting, entries, total)?;
        }
        *total = total
            .checked_add(metadata.len())
            .ok_or(RolloutTransferError::LineageTooLarge)?;
        if *total > CODEX_ROLLOUT_MAX_LINEAGE_BYTES
            || entries.len() >= CODEX_ROLLOUT_MAX_LINEAGE_FILES
        {
            return Err(RolloutTransferError::LineageTooLarge);
        }
        let relative_path = self.relative_path(path)?;
        entries.push(CodexRolloutManifestEntry {
            thread_id,
            parent_thread_id: parent,
            relative_path,
            size_bytes: metadata.len(),
            sha256: sha256_file(path, CODEX_ROLLOUT_MAX_FILE_BYTES)?,
        });
        visiting.remove(&thread_id);
        Ok(())
    }

    pub fn read_artifact(
        &self,
        manifest: &CodexRolloutManifest,
        thread_id: Uuid,
    ) -> Result<CodexRolloutArtifact, RolloutTransferError> {
        validate_manifest(manifest)?;
        let entry = manifest
            .entries
            .iter()
            .find(|entry| entry.thread_id == thread_id)
            .ok_or(RolloutTransferError::InvalidManifest)?;
        let path = self.safe_existing_path(&entry.relative_path)?;
        let before = fs::metadata(&path)?;
        if before.len() != entry.size_bytes {
            return Err(RolloutTransferError::SourceChanged(thread_id));
        }
        let mut bytes = Vec::with_capacity(entry.size_bytes as usize);
        File::open(&path)?
            .take(CODEX_ROLLOUT_MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        let after = fs::metadata(&path)?;
        let digest = sha256_bytes(&bytes);
        if before.len() != after.len()
            || bytes.len() as u64 != entry.size_bytes
            || digest != entry.sha256
        {
            return Err(RolloutTransferError::SourceChanged(thread_id));
        }
        Ok(CodexRolloutArtifact {
            thread_id,
            size_bytes: entry.size_bytes,
            sha256: digest,
            data_base64: BASE64_STANDARD.encode(bytes),
        })
    }

    pub fn stage_artifact(
        &self,
        manifest: &CodexRolloutManifest,
        artifact: &CodexRolloutArtifact,
    ) -> Result<CodexRolloutStageResult, RolloutTransferError> {
        validate_manifest(manifest)?;
        let entry = manifest
            .entries
            .iter()
            .find(|entry| entry.thread_id == artifact.thread_id)
            .ok_or(RolloutTransferError::InvalidManifest)?;
        if artifact.size_bytes != entry.size_bytes || artifact.sha256 != entry.sha256 {
            return Err(RolloutTransferError::ChecksumMismatch(artifact.thread_id));
        }
        let bytes = BASE64_STANDARD
            .decode(&artifact.data_base64)
            .map_err(|_| RolloutTransferError::Encoding)?;
        if bytes.len() as u64 != entry.size_bytes || sha256_bytes(&bytes) != entry.sha256 {
            return Err(RolloutTransferError::ChecksumMismatch(artifact.thread_id));
        }
        let destination = self.safe_destination_path(&entry.relative_path)?;
        if destination.exists() {
            let digest = sha256_file(&destination, CODEX_ROLLOUT_MAX_FILE_BYTES)?;
            if fs::metadata(&destination)?.len() == entry.size_bytes && digest == entry.sha256 {
                if self.receipt_path(entry.thread_id).is_file() {
                    self.write_receipt(entry)?;
                }
                return Ok(CodexRolloutStageResult {
                    thread_id: entry.thread_id,
                    reused: true,
                    verified_sha256: digest,
                });
            }
            return Err(RolloutTransferError::TargetConflict(entry.thread_id));
        }
        let parent = destination
            .parent()
            .ok_or(RolloutTransferError::UnsafePath)?;
        create_safe_directories(&self.sessions_root, parent)?;
        let tmp = parent.join(format!(
            ".vk-transfer-{}-{}.partial",
            manifest.operation_id, entry.thread_id
        ));
        match fs::symlink_metadata(&tmp) {
            Ok(metadata)
                if metadata.file_type().is_file() && !metadata.file_type().is_symlink() =>
            {
                fs::remove_file(&tmp)?;
            }
            Ok(_) => return Err(RolloutTransferError::UnsafePath),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        set_private_permissions(&file)?;
        if let Err(error) = (|| -> Result<(), RolloutTransferError> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::hard_link(&tmp, &destination).map_err(|error| {
                if destination.exists() {
                    RolloutTransferError::TargetConflict(entry.thread_id)
                } else {
                    RolloutTransferError::Io(error)
                }
            })?;
            let digest = sha256_file(&destination, CODEX_ROLLOUT_MAX_FILE_BYTES)?;
            if digest != entry.sha256 {
                if paths_are_same_file(&tmp, &destination) {
                    fs::remove_file(&destination)?;
                }
                return Err(RolloutTransferError::ChecksumMismatch(entry.thread_id));
            }
            fs::remove_file(&tmp)?;
            Ok(())
        })() {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
        self.write_receipt(entry)?;
        Ok(CodexRolloutStageResult {
            thread_id: entry.thread_id,
            reused: false,
            verified_sha256: entry.sha256.clone(),
        })
    }

    pub fn verify_manifest(
        &self,
        manifest: &CodexRolloutManifest,
    ) -> Result<CodexRolloutVerification, RolloutTransferError> {
        validate_manifest(manifest)?;
        for entry in &manifest.entries {
            let path = self.safe_existing_path(&entry.relative_path)?;
            if fs::metadata(&path)?.len() != entry.size_bytes
                || sha256_file(&path, CODEX_ROLLOUT_MAX_FILE_BYTES)? != entry.sha256
            {
                return Err(RolloutTransferError::ChecksumMismatch(entry.thread_id));
            }
            read_canonical_meta(&path, entry.thread_id)?;
        }
        Ok(CodexRolloutVerification {
            manifest_sha256: manifest.manifest_sha256.clone(),
            verified_thread_ids: manifest
                .entries
                .iter()
                .map(|entry| entry.thread_id)
                .collect(),
        })
    }

    pub fn cleanup_expired(
        &self,
        now: SystemTime,
        allow_verified_removal: bool,
    ) -> Result<RolloutCleanupResult, RolloutTransferError> {
        let mut result = RolloutCleanupResult::default();
        let mut stack = vec![self.sessions_root.clone()];
        while let Some(directory) = stack.pop() {
            for entry in fs::read_dir(directory)? {
                if result.partials_removed >= MAX_CLEANUP_FILES {
                    break;
                }
                let entry = entry?;
                let file_type = entry.file_type()?;
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    if entry.path() != self.receipts_root {
                        stack.push(entry.path());
                    }
                    continue;
                }
                if !file_type.is_file()
                    || !entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".vk-transfer-")
                    || !entry.file_name().to_string_lossy().ends_with(".partial")
                {
                    continue;
                }
                let modified = entry.metadata()?.modified()?;
                if now.duration_since(modified).unwrap_or_default() >= PARTIAL_RETENTION {
                    fs::remove_file(entry.path())?;
                    result.partials_removed += 1;
                }
            }
        }

        for entry in fs::read_dir(&self.receipts_root)?.take(MAX_CLEANUP_FILES) {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_file() || file_type.is_symlink() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') && name.ends_with(".partial") {
                let modified = entry.metadata()?.modified()?;
                if now.duration_since(modified).unwrap_or_default() >= PARTIAL_RETENTION {
                    fs::remove_file(entry.path())?;
                    result.partials_removed += 1;
                }
                continue;
            }
            if !allow_verified_removal {
                continue;
            }
            let receipt: TransferReceipt = match serde_json::from_reader(File::open(entry.path())?)
            {
                Ok(receipt) => receipt,
                Err(_) => continue,
            };
            let last_needed = UNIX_EPOCH + Duration::from_secs(receipt.last_needed_unix_seconds);
            if now.duration_since(last_needed).unwrap_or_default() < VERIFIED_RETENTION {
                continue;
            }
            let rollout = match self.safe_existing_path(&receipt.relative_path) {
                Ok(path) => path,
                Err(_) => continue,
            };
            if sha256_file(&rollout, CODEX_ROLLOUT_MAX_FILE_BYTES)? != receipt.sha256 {
                continue;
            }
            fs::remove_file(rollout)?;
            fs::remove_file(entry.path())?;
            result.verified_removed += 1;
        }
        Ok(result)
    }

    fn receipt_path(&self, thread_id: Uuid) -> PathBuf {
        self.receipts_root.join(format!("{thread_id}.json"))
    }

    fn write_receipt(&self, entry: &CodexRolloutManifestEntry) -> Result<(), RolloutTransferError> {
        let receipt = TransferReceipt {
            thread_id: entry.thread_id,
            relative_path: entry.relative_path.clone(),
            sha256: entry.sha256.clone(),
            last_needed_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        let destination = self.receipt_path(entry.thread_id);
        let temporary =
            self.receipts_root
                .join(format!(".{}-{}.partial", entry.thread_id, Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        set_private_permissions(&file)?;
        serde_json::to_writer(&mut file, &receipt)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        file.sync_all()?;
        fs::rename(&temporary, destination)?;
        Ok(())
    }

    fn index_rollouts(&self) -> Result<HashMap<Uuid, PathBuf>, RolloutTransferError> {
        let mut index = HashMap::new();
        let mut stack = vec![self.sessions_root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let ty = entry.file_type()?;
                if ty.is_symlink() {
                    continue;
                }
                if ty.is_dir() {
                    stack.push(entry.path());
                    continue;
                }
                if !ty.is_file() {
                    continue;
                }
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if let Some(id) = rollout_id_from_name(&name)
                    && index.insert(id, entry.path()).is_some()
                {
                    return Err(RolloutTransferError::InvalidMetadata {
                        thread_id: id,
                        reason: "duplicate rollout identity",
                    });
                }
            }
        }
        Ok(index)
    }

    fn relative_path(&self, path: &Path) -> Result<String, RolloutTransferError> {
        let canonical = path.canonicalize()?;
        let relative = canonical
            .strip_prefix(&self.sessions_root)
            .map_err(|_| RolloutTransferError::UnsafePath)?;
        validate_relative(relative)?;
        Ok(relative.to_string_lossy().into_owned())
    }

    fn safe_existing_path(&self, relative: &str) -> Result<PathBuf, RolloutTransferError> {
        let relative = Path::new(relative);
        validate_relative(relative)?;
        ensure_no_symlink_components(&self.sessions_root, relative)?;
        let path = self.sessions_root.join(relative);
        let meta = fs::symlink_metadata(&path)?;
        if !meta.file_type().is_file()
            || meta.file_type().is_symlink()
            || !path.canonicalize()?.starts_with(&self.sessions_root)
        {
            return Err(RolloutTransferError::UnsafePath);
        }
        Ok(path)
    }

    fn safe_destination_path(&self, relative: &str) -> Result<PathBuf, RolloutTransferError> {
        let relative = Path::new(relative);
        validate_relative(relative)?;
        Ok(self.sessions_root.join(relative))
    }
}

#[cfg(unix)]
fn paths_are_same_file(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    match (fs::metadata(left), fs::metadata(right)) {
        (Ok(left), Ok(right)) => left.dev() == right.dev() && left.ino() == right.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn paths_are_same_file(_left: &Path, _right: &Path) -> bool {
    false
}

fn ensure_no_symlink_components(root: &Path, relative: &Path) -> Result<(), RolloutTransferError> {
    let mut current = root.to_path_buf();
    let component_count = relative.components().count();
    for (index, component) in relative.components().enumerate() {
        let Component::Normal(part) = component else {
            return Err(RolloutTransferError::UnsafePath);
        };
        current.push(part);
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink()
            || (index + 1 < component_count && !metadata.file_type().is_dir())
        {
            return Err(RolloutTransferError::UnsafePath);
        }
    }
    Ok(())
}

fn read_canonical_meta(path: &Path, expected: Uuid) -> Result<CanonicalMeta, RolloutTransferError> {
    let mut line = String::new();
    BufReader::new(File::open(path)?.take(MAX_METADATA_LINE_BYTES + 1)).read_line(&mut line)?;
    if line.len() as u64 > MAX_METADATA_LINE_BYTES {
        return Err(RolloutTransferError::InvalidMetadata {
            thread_id: expected,
            reason: "metadata line is oversized",
        });
    }
    let envelope: RolloutEnvelope =
        serde_json::from_str(&line).map_err(|_| RolloutTransferError::InvalidMetadata {
            thread_id: expected,
            reason: "first line is not valid session metadata",
        })?;
    if envelope.kind != "session_meta" || envelope.payload.id != expected {
        return Err(RolloutTransferError::InvalidMetadata {
            thread_id: expected,
            reason: "canonical thread identity mismatch",
        });
    }
    Ok(envelope.payload)
}

fn rollout_id_from_name(name: &str) -> Option<Uuid> {
    let stem = name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    let id = stem
        .rsplit_once('-')
        .and_then(|_| stem.get(stem.len().checked_sub(36)?..))?;
    Uuid::parse_str(id).ok()
}

fn validate_relative(path: &Path) -> Result<(), RolloutTransferError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(RolloutTransferError::UnsafePath);
    }
    Ok(())
}

fn create_safe_directories(root: &Path, parent: &Path) -> Result<(), RolloutTransferError> {
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| RolloutTransferError::UnsafePath)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(RolloutTransferError::UnsafePath);
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => return Err(RolloutTransferError::UnsafePath),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&current)?,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn validate_manifest(manifest: &CodexRolloutManifest) -> Result<(), RolloutTransferError> {
    if manifest.entries.is_empty()
        || manifest.entries.len() > CODEX_ROLLOUT_MAX_LINEAGE_FILES
        || manifest.entries.last().map(|entry| entry.thread_id) != Some(manifest.leaf_thread_id)
        || manifest_digest(manifest) != manifest.manifest_sha256
    {
        return Err(RolloutTransferError::InvalidManifest);
    }
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for (index, entry) in manifest.entries.iter().enumerate() {
        validate_relative(Path::new(&entry.relative_path))?;
        if !seen.insert(entry.thread_id) || entry.size_bytes > CODEX_ROLLOUT_MAX_FILE_BYTES {
            return Err(RolloutTransferError::InvalidManifest);
        }
        let expected_parent = index
            .checked_sub(1)
            .map(|parent_index| manifest.entries[parent_index].thread_id);
        if entry.parent_thread_id != expected_parent {
            return Err(RolloutTransferError::InvalidManifest);
        }
        total = total
            .checked_add(entry.size_bytes)
            .ok_or(RolloutTransferError::InvalidManifest)?;
    }
    if total > CODEX_ROLLOUT_MAX_LINEAGE_BYTES {
        return Err(RolloutTransferError::InvalidManifest);
    }
    Ok(())
}

fn manifest_digest(manifest: &CodexRolloutManifest) -> String {
    let mut hash = Sha256::new();
    hash.update(manifest.operation_id.as_bytes());
    hash.update(manifest.workspace_id.as_bytes());
    hash.update(manifest.source_execution_id.as_bytes());
    hash.update(manifest.source_worker_node_id.as_bytes());
    hash.update(manifest.target_worker_node_id.as_bytes());
    hash.update(manifest.leaf_thread_id.as_bytes());
    for entry in &manifest.entries {
        hash.update(entry.thread_id.as_bytes());
        if let Some(parent) = entry.parent_thread_id {
            hash.update(parent.as_bytes());
        }
        hash.update(entry.relative_path.as_bytes());
        hash.update(entry.size_bytes.to_be_bytes());
        hash.update(entry.sha256.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn sha256_file(path: &Path, limit: u64) -> Result<String, RolloutTransferError> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let copied = std::io::copy(
        &mut std::io::Read::by_ref(&mut file).take(limit + 1),
        &mut HashWriter(&mut hash),
    )?;
    if copied > limit {
        return Err(RolloutTransferError::LineageTooLarge);
    }
    Ok(format!("{:x}", hash.finalize()))
}

struct HashWriter<'a>(&'a mut Sha256);
impl Write for HashWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.update(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(unix)]
fn set_private_permissions(file: &File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))
}
#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}
#[cfg(not(unix))]
fn set_private_permissions(_file: &File) -> std::io::Result<()> {
    Ok(())
}
#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn rollout(root: &Path, id: Uuid, parent: Option<Uuid>, body: &str) -> PathBuf {
        let dir = root.join("sessions/2026/08/06");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("rollout-2026-08-06T00-00-00-{id}.jsonl"));
        let meta = serde_json::json!({"timestamp":"2026-08-06T00:00:00Z","type":"session_meta","payload":{"id":id,"parent_thread_id":parent}});
        fs::write(&path, format!("{meta}\n{body}\n")).unwrap();
        path
    }

    #[test]
    fn resolves_and_stages_ancestor_first_idempotently() {
        let source = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();
        let parent = Uuid::new_v4();
        let leaf = Uuid::new_v4();
        rollout(source.path(), parent, None, "{}");
        rollout(source.path(), leaf, Some(parent), "{}");
        let source_store = CodexRolloutStore::new(source.path()).unwrap();
        let target_store = CodexRolloutStore::new(target.path()).unwrap();
        let manifest = source_store
            .resolve_manifest(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                leaf,
            )
            .unwrap();
        assert_eq!(
            manifest
                .entries
                .iter()
                .map(|entry| entry.thread_id)
                .collect::<Vec<_>>(),
            vec![parent, leaf]
        );
        let first_destination = target
            .path()
            .join("sessions")
            .join(&manifest.entries[0].relative_path);
        fs::create_dir_all(first_destination.parent().unwrap()).unwrap();
        let stale_partial = first_destination.parent().unwrap().join(format!(
            ".vk-transfer-{}-{}.partial",
            manifest.operation_id, manifest.entries[0].thread_id
        ));
        fs::write(&stale_partial, "interrupted").unwrap();
        for entry in &manifest.entries {
            let artifact = source_store
                .read_artifact(&manifest, entry.thread_id)
                .unwrap();
            assert!(
                !target_store
                    .stage_artifact(&manifest, &artifact)
                    .unwrap()
                    .reused
            );
            assert!(
                target_store
                    .stage_artifact(&manifest, &artifact)
                    .unwrap()
                    .reused
            );
        }
        assert_eq!(
            target_store
                .verify_manifest(&manifest)
                .unwrap()
                .verified_thread_ids,
            vec![parent, leaf]
        );
        assert!(!stale_partial.exists());

        let abandoned_partial = first_destination
            .parent()
            .unwrap()
            .join(".vk-transfer-abandoned.partial");
        fs::write(&abandoned_partial, "partial").unwrap();
        let after_partial_retention =
            SystemTime::now() + PARTIAL_RETENTION + Duration::from_secs(1);
        let cleanup = target_store
            .cleanup_expired(after_partial_retention, false)
            .unwrap();
        assert_eq!(cleanup.partials_removed, 1);
        assert_eq!(cleanup.verified_removed, 0);

        let after_verified_retention =
            SystemTime::now() + VERIFIED_RETENTION + Duration::from_secs(1);
        assert_eq!(
            target_store
                .cleanup_expired(after_verified_retention, false)
                .unwrap()
                .verified_removed,
            0
        );
        assert_eq!(
            target_store
                .cleanup_expired(after_verified_retention, true)
                .unwrap()
                .verified_removed,
            2
        );
        assert!(!first_destination.exists());
    }

    #[test]
    fn rejects_traversal_symlink_and_conflicting_content() {
        let source = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();
        let leaf = Uuid::new_v4();
        rollout(source.path(), leaf, None, "{}");
        let source_store = CodexRolloutStore::new(source.path()).unwrap();
        let target_store = CodexRolloutStore::new(target.path()).unwrap();
        let manifest = source_store
            .resolve_manifest(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                leaf,
            )
            .unwrap();
        let artifact = source_store.read_artifact(&manifest, leaf).unwrap();
        let destination = target
            .path()
            .join("sessions")
            .join(&manifest.entries[0].relative_path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(&destination, "different").unwrap();
        assert!(matches!(
            target_store.stage_artifact(&manifest, &artifact),
            Err(RolloutTransferError::TargetConflict(_))
        ));

        let mut traversal = manifest.clone();
        traversal.entries[0].relative_path = "../escape".into();
        traversal.manifest_sha256 = manifest_digest(&traversal);
        assert!(matches!(
            target_store.verify_manifest(&traversal),
            Err(RolloutTransferError::UnsafePath)
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            fs::remove_file(&destination).unwrap();
            let escaped = target.path().join("escaped");
            fs::create_dir(&escaped).unwrap();
            let date_dir = target.path().join("sessions/2026");
            fs::remove_dir_all(&date_dir).unwrap();
            symlink(&escaped, &date_dir).unwrap();
            assert!(matches!(
                target_store.stage_artifact(&manifest, &artifact),
                Err(RolloutTransferError::UnsafePath)
            ));

            let contained_root = TempDir::new().unwrap();
            let contained_store = CodexRolloutStore::new(contained_root.path()).unwrap();
            let inside = contained_root.path().join("sessions/inside");
            fs::create_dir(&inside).unwrap();
            fs::write(inside.join("rollout.jsonl"), "{}").unwrap();
            symlink(&inside, contained_root.path().join("sessions/link")).unwrap();
            assert!(matches!(
                contained_store.safe_existing_path("link/rollout.jsonl"),
                Err(RolloutTransferError::UnsafePath)
            ));
        }
    }

    #[test]
    fn rejects_checksum_mismatch_cycle_and_oversized_source() {
        let source = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        rollout(source.path(), first, Some(second), "{}");
        rollout(source.path(), second, Some(first), "{}");
        let source_store = CodexRolloutStore::new(source.path()).unwrap();
        assert!(matches!(
            source_store.resolve_manifest(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                first,
            ),
            Err(RolloutTransferError::Cycle(_))
        ));

        let direct = Uuid::new_v4();
        let path = rollout(source.path(), direct, None, "{}");
        let manifest = source_store
            .resolve_manifest(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                direct,
            )
            .unwrap();
        let mut artifact = source_store.read_artifact(&manifest, direct).unwrap();
        artifact.data_base64 = BASE64_STANDARD.encode(b"tampered");
        let target_store = CodexRolloutStore::new(target.path()).unwrap();
        assert!(matches!(
            target_store.stage_artifact(&manifest, &artifact),
            Err(RolloutTransferError::ChecksumMismatch(_))
        ));

        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_len(CODEX_ROLLOUT_MAX_FILE_BYTES + 1)
            .unwrap();
        assert!(matches!(
            source_store.resolve_manifest(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                direct,
            ),
            Err(RolloutTransferError::FileTooLarge { .. })
        ));
    }
}
