//! A durable materialized view of a finished process's normalized log.
//!
//! Reading a completed process's conversation used to rerun the vendor
//! normalizer over its entire raw log, every time, because only raw stdout is
//! persisted. That cost is paid on an interactive request — it is what made
//! opening a long conversation take minutes — and it is paid again on every
//! refresh, because nothing kept the result.
//!
//! This module keeps the result. A finished process's *final* normalized
//! entries are written once, beside the raw log they were derived from, and
//! later reads replay them directly.
//!
//! # Why entries and not patches
//!
//! The stored form is the settled entry list, not the patch frames that
//! produced it. Patch frames cannot be sliced or counted:
//! `replace /entries/4` depends on the `add` before it, `remove` invalidates
//! later indexes, and frame count is not entry count. So the frames are
//! *applied* — by `json_patch`, against the same `{"entries": []}` document
//! the frontend builds — and the settled array is what gets stored. Replaying
//! it as one `add` per entry reproduces a stream the existing reader already
//! understands, which is why this needs no API or frontend change.
//!
//! # Why a sidecar rather than a table
//!
//! Raw logs already left SQLite for `sessions/<session>/processes/<id>.jsonl`.
//! Putting the materialized view beside them keeps one storage story, avoids a
//! schema migration, and — the part that matters operationally — inherits the
//! existing session-directory cleanup, so this cache cannot outlive the logs it
//! describes or strand rows nothing will collect.
//!
//! # Correctness boundary
//!
//! Only *finished* processes are cached: a running one still has entries to
//! come, and a cache of a moving target is a stale read waiting to happen.
//! Writes are atomic (temp file, then rename), so a crash mid-write leaves the
//! previous state rather than a truncated file that would read as a short
//! conversation. A cache whose header does not match [`CACHE_VERSION`] is
//! ignored and rewritten, which is what makes a normalizer change safe to ship:
//! bump the constant and every cache is re-derived on next read.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use json_patch::{Patch, PatchOperation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Bump when the normalizers change what they emit, or when the file layout
/// below changes. Any cache written by a different version is discarded rather
/// than trusted, so a stale entry can never outlive the code that wrote it.
pub const CACHE_VERSION: u32 = 1;

/// First line of the file. Kept separate from the entries so a truncated or
/// half-written file fails to parse as a header rather than as a short
/// conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheHeader {
    pub version: u32,
    pub entry_count: usize,
    /// Whether the normalization this was derived from had to drop the
    /// beginning of the log to stay bounded. Recorded so a reader knows it is
    /// looking at a partial conversation, and so a later full materialization
    /// can be preferred over a truncated one.
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("applying patch {index}: {source}")]
    Patch {
        index: usize,
        #[source]
        source: json_patch::PatchError,
    },
    #[error("normalized log document lost its entries array")]
    MalformedDocument,
}

/// Apply a finished process's patch frames and return the entries they settle
/// on.
///
/// The document starts as `{"entries": []}` — the same shape
/// `streamJsonPatchEntries` builds in the browser — so the two agree on what a
/// patch means by construction rather than by a second implementation of the
/// same rules.
pub fn materialize_entries(patches: &[Patch]) -> Result<Vec<Value>, CacheError> {
    let mut document = json!({ "entries": [] });
    for (index, patch) in patches.iter().enumerate() {
        json_patch::patch(&mut document, patch)
            .map_err(|source| CacheError::Patch { index, source })?;
    }

    match document.get_mut("entries").map(Value::take) {
        Some(Value::Array(entries)) => Ok(entries),
        _ => Err(CacheError::MalformedDocument),
    }
}

/// [`materialize_entries`] for a patch sequence whose beginning is missing,
/// re-basing entry indices onto the entries that survive and reporting how
/// many operations were dropped.
///
/// Only for a live store's retained history, which is byte-capped and evicts
/// from the front. Strict application cannot recover from that at all: once
/// `add /entries/0..k` are gone, every surviving `add /entries/N` is out of
/// bounds against the shorter array, so applying leniently in place would
/// skip *everything* and report an empty conversation — the failure this is
/// here to avoid. Appending each surviving `add` and remapping later
/// `replace`/`remove` against its new position keeps the entries that are
/// still fully described.
///
/// The stored-sidecar path deliberately keeps the strict form: there, a patch
/// that does not apply means a corrupt artifact that must be re-derived, not
/// a window that slid.
pub fn materialize_entries_rebased(patches: &[Patch]) -> (Vec<Value>, usize) {
    let mut entries: Vec<Value> = Vec::new();
    // Original entry index -> its position in `entries`.
    let mut positions: HashMap<usize, usize> = HashMap::new();
    let mut dropped = 0;

    for operation in patches.iter().flat_map(|patch| patch.0.iter()) {
        let index = entry_index(operation.path());
        match (operation, index) {
            (PatchOperation::Add(add), Some(index)) => {
                positions.insert(index, entries.len());
                entries.push(add.value.clone());
            }
            (PatchOperation::Replace(replace), Some(index)) => {
                match positions.get(&index) {
                    Some(&position) => entries[position] = replace.value.clone(),
                    // Its `add` was evicted, so there is nothing to revise.
                    None => dropped += 1,
                }
            }
            (PatchOperation::Remove(_), Some(index)) => match positions.remove(&index) {
                Some(position) => {
                    entries.remove(position);
                    for mapped in positions.values_mut() {
                        if *mapped > position {
                            *mapped -= 1;
                        }
                    }
                }
                None => dropped += 1,
            },
            _ => dropped += 1,
        }
    }

    (entries, dropped)
}

/// The `<index>` of an `/entries/<index>` pointer, if it is one.
///
/// `/entries/<repo>/<file>` is a repo diff, not a conversation entry, and
/// fails the `parse` because of the remaining segment.
fn entry_index(path: &str) -> Option<usize> {
    path.strip_prefix("/entries/")?.parse().ok()
}

/// Rebuild the patch stream a reader expects from stored entries.
///
/// One `add` per entry, in order: a settled conversation has no history of
/// replacements worth replaying, and the reader's document ends up identical
/// either way.
pub fn entries_as_patches(entries: &[Value]) -> Result<Vec<Patch>, CacheError> {
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            serde_json::from_value(json!([{
                "op": "add",
                "path": format!("/entries/{index}"),
                "value": entry,
            }]))
            .map_err(CacheError::from)
        })
        .collect()
}

fn serialize(header: &CacheHeader, entries: &[Value]) -> Result<String, CacheError> {
    let mut out = serde_json::to_string(header)?;
    out.push('\n');
    for entry in entries {
        out.push_str(&serde_json::to_string(entry)?);
        out.push('\n');
    }
    Ok(out)
}

fn deserialize(contents: &str) -> Option<(CacheHeader, Vec<Value>)> {
    let mut lines = contents.lines();
    let header: CacheHeader = serde_json::from_str(lines.next()?).ok()?;
    if header.version != CACHE_VERSION {
        return None;
    }

    let entries = lines
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;

    // A file whose body disagrees with its own header was cut short — by a
    // crash, a full disk, or a partial read. Re-deriving is cheap relative to
    // showing someone a conversation that silently stops early.
    (entries.len() == header.entry_count).then_some((header, entries))
}

/// Read a cached conversation, or `None` when there is nothing usable here.
///
/// Every failure — absent, unreadable, wrong version, inconsistent — answers
/// `None`, because every one of them has the same correct response: derive it
/// again. This cache is never the only copy; the raw log remains the source of
/// truth.
pub async fn read(path: &Path) -> Option<(CacheHeader, Vec<Value>)> {
    let contents = tokio::fs::read_to_string(path).await.ok()?;
    deserialize(&contents)
}

/// Write a cached conversation atomically.
///
/// Temp file then rename, so a reader either sees the previous cache or the new
/// one. A half-written file that parsed would be worse than no cache at all: it
/// would read as a complete but shorter conversation.
pub async fn write(path: &Path, header: CacheHeader, entries: &[Value]) -> Result<(), CacheError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let body = serialize(&header, entries)?;

    let temp_path: PathBuf = path.with_extension("jsonl.tmp");
    tokio::fs::write(&temp_path, body).await?;
    match tokio::fs::rename(&temp_path, path).await {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            Err(e.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(index: usize, value: Value) -> Patch {
        serde_json::from_value(json!([{
            "op": "add",
            "path": format!("/entries/{index}"),
            "value": value,
        }]))
        .unwrap()
    }

    fn replace(index: usize, value: Value) -> Patch {
        serde_json::from_value(json!([{
            "op": "replace",
            "path": format!("/entries/{index}"),
            "value": value,
        }]))
        .unwrap()
    }

    fn remove(index: usize) -> Patch {
        serde_json::from_value(json!([{
            "op": "remove",
            "path": format!("/entries/{index}"),
        }]))
        .unwrap()
    }

    /// The reason the stored form is entries rather than frames: a `replace`
    /// only means anything relative to the `add` before it, so the settled
    /// value is the one worth keeping.
    #[test]
    fn a_replaced_entry_materializes_to_its_final_value() {
        let entries = materialize_entries(&[
            add(0, json!("first")),
            add(1, json!("provisional")),
            replace(1, json!("settled")),
        ])
        .unwrap();

        assert_eq!(entries, vec![json!("first"), json!("settled")]);
    }

    /// And the reason frame count is not entry count.
    #[test]
    fn a_removed_entry_does_not_survive_materialization() {
        let entries =
            materialize_entries(&[add(0, json!("kept")), add(1, json!("dropped")), remove(1)])
                .unwrap();

        assert_eq!(entries, vec![json!("kept")]);
    }

    #[test]
    fn replaying_entries_reproduces_them() {
        let original = vec![json!({"type": "NORMALIZED_ENTRY"}), json!("stdout line")];

        let replayed = materialize_entries(&entries_as_patches(&original).unwrap()).unwrap();

        assert_eq!(replayed, original);
    }

    #[test]
    fn a_round_trip_through_the_file_format_preserves_entries() {
        let entries = vec![json!({"a": 1}), json!("two")];
        let header = CacheHeader {
            version: CACHE_VERSION,
            entry_count: entries.len(),
            truncated: false,
        };

        let (read_header, read_entries) =
            deserialize(&serialize(&header, &entries).unwrap()).unwrap();

        assert_eq!(read_header, header);
        assert_eq!(read_entries, entries);
    }

    /// A normalizer change must not serve conversations derived by the old one.
    #[test]
    fn a_cache_from_another_version_is_refused() {
        let entries = vec![json!("entry")];
        let stale = serialize(
            &CacheHeader {
                version: CACHE_VERSION + 1,
                entry_count: 1,
                truncated: false,
            },
            &entries,
        )
        .unwrap();

        assert!(deserialize(&stale).is_none());
    }

    /// The failure mode this format exists to avoid: a short file reading as a
    /// complete, shorter conversation.
    #[test]
    fn a_truncated_file_is_refused_rather_than_read_as_a_short_conversation() {
        let entries = vec![json!("one"), json!("two"), json!("three")];
        let header = CacheHeader {
            version: CACHE_VERSION,
            entry_count: entries.len(),
            truncated: false,
        };
        let full = serialize(&header, &entries).unwrap();
        let cut = full.lines().take(3).collect::<Vec<_>>().join("\n");

        assert!(deserialize(&cut).is_none());
    }

    #[tokio::test]
    async fn writing_then_reading_returns_what_was_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("proc.normalized.jsonl");
        let entries = vec![json!({"entry": "one"})];
        let header = CacheHeader {
            version: CACHE_VERSION,
            entry_count: 1,
            truncated: true,
        };

        write(&path, header.clone(), &entries).await.unwrap();

        assert_eq!(read(&path).await, Some((header, entries)));
        // The temp file must not be left behind for a reader to trip over.
        assert!(!path.with_extension("jsonl.tmp").exists());
    }

    #[tokio::test]
    async fn a_missing_cache_is_simply_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(&dir.path().join("absent.jsonl")).await.is_none());
    }
    /// The case the re-basing pass exists for: front eviction leaves a
    /// patch run that starts mid-conversation, which strict application
    /// cannot apply at all.
    #[test]
    fn front_eviction_rebases_surviving_entries_onto_a_fresh_array() {
        let patches = vec![add(2, json!("third")), add(3, json!("fourth"))];

        assert!(
            materialize_entries(&patches).is_err(),
            "strict application must fail, or the fallback would be pointless"
        );

        let (entries, dropped) = materialize_entries_rebased(&patches);
        assert_eq!(entries, vec![json!("third"), json!("fourth")]);
        assert_eq!(dropped, 0);
    }

    #[test]
    fn a_replace_follows_its_entry_to_the_rebased_position() {
        let patches = vec![
            add(2, json!("third")),
            add(3, json!("fourth")),
            replace(3, json!("fourth, revised")),
        ];

        let (entries, dropped) = materialize_entries_rebased(&patches);
        assert_eq!(entries, vec![json!("third"), json!("fourth, revised")]);
        assert_eq!(dropped, 0);
    }

    #[test]
    fn operations_on_evicted_entries_are_counted_not_applied() {
        let patches = vec![
            replace(0, json!("its add was evicted")),
            remove(1),
            add(2, json!("third")),
        ];

        let (entries, dropped) = materialize_entries_rebased(&patches);
        assert_eq!(entries, vec![json!("third")]);
        assert_eq!(dropped, 2);
    }

    /// A remove shifts the entries after it, so later indices must still
    /// resolve to the right position.
    #[test]
    fn a_remove_rebases_the_entries_after_it() {
        let patches = vec![
            add(2, json!("third")),
            add(3, json!("fourth")),
            add(4, json!("fifth")),
            remove(3),
            replace(4, json!("fifth, revised")),
        ];

        let (entries, dropped) = materialize_entries_rebased(&patches);
        assert_eq!(entries, vec![json!("third"), json!("fifth, revised")]);
        assert_eq!(dropped, 0);
    }

    /// An intact history must materialize identically either way, so the
    /// fallback cannot change what a normal read returns.
    #[test]
    fn an_intact_history_rebases_to_the_same_entries_strict_application_gives() {
        let patches = vec![
            add(0, json!("first")),
            add(1, json!("second")),
            replace(1, json!("second, revised")),
        ];

        let (rebased, dropped) = materialize_entries_rebased(&patches);
        assert_eq!(rebased, materialize_entries(&patches).unwrap());
        assert_eq!(dropped, 0);
    }

    /// Repo diffs target `/entries/<repo>/<file>`, not a conversation entry.
    #[test]
    fn repo_diff_paths_are_not_mistaken_for_entry_indices() {
        assert_eq!(entry_index("/entries/3"), Some(3));
        assert_eq!(entry_index("/entries/repo/src~1main.rs"), None);
        assert_eq!(entry_index("/entries"), None);
        assert_eq!(entry_index("/other/3"), None);
    }
}
