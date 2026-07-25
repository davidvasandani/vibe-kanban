//! File-based task pipelines.
//!
//! Each `~/.vibe-kanban/pipelines/*.toml` file defines one selectable pipeline:
//! a `name`, an optional `description`, and an ordered list of `[[stage]]`
//! tables. The file stem is the pipeline `id` (e.g. `basic.toml` → `basic`).
//!
//! ```toml
//! name = "Basic"
//! description = "Classic dev flow."
//!
//! [[stage]]
//! id = "spec"
//! label = "Create spec"
//! default_enabled = true
//! prompt = "Write a technical spec ..."
//! ```
//!
//! The task-create dialog reads these (via `GET /api/pipelines`), the operator
//! picks one pipeline and ticks which stages apply, and vibe-kanban composes an
//! ordered `## Pipeline` block into the task description. Stages are ordered by
//! their position in the file. Bundled defaults are seeded to disk on first run.

use std::{
    collections::HashSet,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

/// Bundled default pipeline files, seeded to `pipelines_dir()` on first run and
/// used by the reset actions. Order here defines the display order of bundled
/// pipelines in the UI.
const BUNDLED: &[(&str, &str)] = &[
    (
        "basic.toml",
        include_str!("../../../../../assets/pipelines/basic.toml"),
    ),
    (
        "wikillm.toml",
        include_str!("../../../../../assets/pipelines/wikillm.toml"),
    ),
    (
        "speckit.toml",
        include_str!("../../../../../assets/pipelines/speckit.toml"),
    ),
    (
        "parallel-subagents.toml",
        include_str!("../../../../../assets/pipelines/parallel-subagents.toml"),
    ),
];

/// Private bookkeeping for incremental bundled-pipeline seeding. This is not a
/// TOML file so pipeline discovery ignores it.
const SEED_MANIFEST: &str = ".bundled-pipelines.json";
const SEED_MANIFEST_VERSION: u32 = 1;

/// Bundles shipped before seed manifests were introduced. A non-empty legacy
/// install is considered to have seen these names even when one was deleted.
/// `parallel-subagents.toml` is intentionally absent: it is the first bundle
/// that must be added incrementally.
const LEGACY_BUNDLED: &[&str] = &["basic.toml", "wikillm.toml", "speckit.toml"];

static SEED_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
/// `load_pipelines` and `load_pipeline_statuses` can run on different request
/// threads. Serialize their reconciliation transactions so one call never
/// rolls back a file after another has committed the manifest.
static SEED_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Serialize, Deserialize)]
struct SeedManifest {
    version: u32,
    bundled: Vec<String>,
}

/// A single per-task pipeline stage. Stages are defined in pipeline files
/// (`~/.vibe-kanban/pipelines/*.toml`, loaded by `services::services::pipelines`
/// into `Pipeline.stages`). The task-create "Pipeline" control lets the operator
/// pick a pipeline and tick which stages apply; the ticked `prompt_fragment`s
/// are composed, in order, into a `## Pipeline` block on the task description.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct PipelineStep {
    /// Stable slug, e.g. "spec".
    pub id: String,
    /// Shown next to the task-create checkbox.
    pub label: String,
    /// Appended as a bullet when the step is ticked.
    pub prompt_fragment: String,
    /// Whether the task checkbox starts ticked.
    #[serde(default)]
    pub default_enabled: bool,
    /// Whether this stage is marked "heavy" (resource-intensive); the UI
    /// renders a badge and it starts unticked by convention.
    #[serde(default)]
    pub heavy: bool,
}

/// A selectable task pipeline loaded from a `*.toml` file.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Pipeline {
    /// Stable slug = the file stem, e.g. "basic".
    pub id: String,
    /// Display name from the file's `name` field.
    pub name: String,
    /// Optional one-line description.
    pub description: Option<String>,
    /// Ordered stages; this order is authoritative for the composed block.
    pub stages: Vec<PipelineStep>,
}

/// A structured TOML parse/validation error, suitable for surfacing inline in
/// the Settings editor (message plus a best-effort 1-based line/column when
/// the underlying `toml` parser exposes a byte span).
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct PipelineParseError {
    pub message: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Result of validating a pipeline TOML draft (`POST /api/pipelines/validate`).
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct PipelineValidation {
    pub valid: bool,
    pub error: Option<PipelineParseError>,
}

/// Per-file status for every `~/.vibe-kanban/pipelines/*.toml` file, including
/// ones that currently fail to parse (and are therefore invisible to
/// `load_pipelines`/`GET /api/pipelines`).
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct PipelineFileStatus {
    pub id: String,
    pub name: String,
    pub stage_count: Option<u32>,
    pub valid: bool,
    pub error: Option<PipelineParseError>,
}

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("failed to parse pipeline TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid pipeline: {0}")]
    Invalid(String),
    #[error("pipeline not found")]
    NotFound,
    #[error("invalid pipeline id")]
    InvalidId,
}

#[derive(Debug, Deserialize)]
struct RawPipeline {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    stage: Vec<RawStage>,
}

#[derive(Debug, Deserialize)]
struct RawStage {
    id: String,
    label: String,
    prompt: String,
    #[serde(default)]
    default_enabled: bool,
    #[serde(default)]
    heavy: bool,
}

/// A slug is a non-empty run of ASCII alphanumerics, `-`, or `_`. Used for both
/// pipeline ids (file stems) and stage ids. Rejects path traversal (`/`, `\`,
/// `..`) and anything that would collide oddly in the UI.
fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Validate an untrusted pipeline id (e.g. a route path param).
pub fn validate_id(id: &str) -> Result<(), PipelineError> {
    if is_valid_slug(id) {
        Ok(())
    } else {
        Err(PipelineError::InvalidId)
    }
}

/// Parse and validate a single pipeline's TOML. `id` is the file stem.
pub fn parse_pipeline(id: &str, raw: &str) -> Result<Pipeline, PipelineError> {
    validate_id(id)?;
    let parsed: RawPipeline = toml::from_str(raw)?;
    if parsed.name.trim().is_empty() {
        return Err(PipelineError::Invalid(
            "pipeline name must not be empty".to_string(),
        ));
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut stages = Vec::with_capacity(parsed.stage.len());
    for st in parsed.stage {
        if !is_valid_slug(&st.id) {
            return Err(PipelineError::Invalid(format!(
                "invalid stage id: {:?}",
                st.id
            )));
        }
        if !seen.insert(st.id.clone()) {
            return Err(PipelineError::Invalid(format!(
                "duplicate stage id: {}",
                st.id
            )));
        }
        if st.label.trim().is_empty() {
            return Err(PipelineError::Invalid(format!(
                "stage {} label must not be empty",
                st.id
            )));
        }
        if st.prompt.trim().is_empty() {
            return Err(PipelineError::Invalid(format!(
                "stage {} prompt must not be empty",
                st.id
            )));
        }
        stages.push(PipelineStep {
            id: st.id,
            label: st.label,
            prompt_fragment: st.prompt,
            default_enabled: st.default_enabled,
            heavy: st.heavy,
        });
    }
    Ok(Pipeline {
        id: id.to_string(),
        name: parsed.name,
        description: parsed.description,
        stages,
    })
}

/// Convert a byte offset into 1-based (line, column) by scanning `content`.
/// `\n` bumps the line and resets the column; anything else advances the
/// column by one char. O(offset), fine for the small pipeline files here.
fn offset_to_line_col(content: &str, offset: usize) -> (u32, u32) {
    let mut line: u32 = 1;
    let mut col: u32 = 1;
    for (idx, ch) in content.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Build a structured error from a `PipelineError`, attaching line/column
/// when the error is a TOML syntax error with a byte span; message-only for
/// semantic errors (`Invalid`/`InvalidId`/`NotFound`/`Io`).
pub fn structured_error(e: &PipelineError, content: &str) -> PipelineParseError {
    match e {
        PipelineError::Parse(de) => {
            let message = {
                let m = de.message();
                if m.is_empty() {
                    e.to_string()
                } else {
                    m.to_string()
                }
            };
            let (line, column) = match de.span() {
                Some(span) => {
                    let (l, c) = offset_to_line_col(content, span.start);
                    (Some(l), Some(c))
                }
                None => (None, None),
            };
            PipelineParseError {
                message,
                line,
                column,
            }
        }
        other => PipelineParseError {
            message: other.to_string(),
            line: None,
            column: None,
        },
    }
}

/// Validate a pipeline TOML draft without touching disk.
pub fn validate(id: &str, content: &str) -> PipelineValidation {
    match parse_pipeline(id, content) {
        Ok(_) => PipelineValidation {
            valid: true,
            error: None,
        },
        Err(e) => PipelineValidation {
            valid: false,
            error: Some(structured_error(&e, content)),
        },
    }
}

/// Sort order shared by `load_pipelines` and `load_pipeline_statuses`:
/// bundled files first (in `BUNDLED` order), then alphabetical by id.
fn bundled_order(id: &str) -> Option<usize> {
    BUNDLED
        .iter()
        .position(|(n, _)| n.trim_end_matches(".toml") == id)
}

fn sort_by_bundled_order<T>(items: &mut [T], id_of: impl Fn(&T) -> &str) {
    items.sort_by(
        |a, b| match (bundled_order(id_of(a)), bundled_order(id_of(b))) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => id_of(a).cmp(id_of(b)),
        },
    );
}

/// Status for every `*.toml` file in `dir`, including malformed ones (which
/// `load_pipelines` silently skips). Seeds bundled defaults first, mirroring
/// `load_pipelines`.
pub fn load_pipeline_statuses(dir: &Path) -> Vec<PipelineFileStatus> {
    if let Err(e) = ensure_seeded(dir) {
        tracing::warn!("failed to seed pipelines dir {}: {}", dir.display(), e);
    }
    let mut out: Vec<PipelineFileStatus> = Vec::new();
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            tracing::warn!("failed to read pipelines dir {}: {}", dir.display(), e);
            return out;
        }
    };
    for entry in rd.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|x| x.to_str()) != Some("toml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let stem = stem.to_string();
        match std::fs::read_to_string(&path) {
            Ok(raw) => match parse_pipeline(&stem, &raw) {
                Ok(p) => out.push(PipelineFileStatus {
                    id: p.id,
                    name: p.name,
                    stage_count: Some(p.stages.len() as u32),
                    valid: true,
                    error: None,
                }),
                Err(e) => out.push(PipelineFileStatus {
                    id: stem.clone(),
                    name: stem,
                    stage_count: None,
                    valid: false,
                    error: Some(structured_error(&e, &raw)),
                }),
            },
            Err(e) => out.push(PipelineFileStatus {
                id: stem.clone(),
                name: stem,
                stage_count: None,
                valid: false,
                error: Some(PipelineParseError {
                    message: e.to_string(),
                    line: None,
                    column: None,
                }),
            }),
        }
    }
    sort_by_bundled_order(&mut out, |s| s.id.as_str());
    out
}

fn has_toml(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .map(|rd| {
            rd.filter_map(|e| e.ok()).any(|e| {
                let path = e.path();
                path.is_file() && path.extension().and_then(|x| x.to_str()) == Some("toml")
            })
        })
        .unwrap_or(false)
}

fn validate_manifest(manifest: SeedManifest) -> Result<HashSet<String>, PipelineError> {
    if manifest.version != SEED_MANIFEST_VERSION {
        return Err(PipelineError::Invalid(format!(
            "unsupported pipeline seed manifest version: {}",
            manifest.version
        )));
    }
    if manifest.bundled.iter().any(|name| {
        Path::new(name).file_name().and_then(|part| part.to_str()) != Some(name.as_str())
            || !name.ends_with(".toml")
    }) {
        return Err(PipelineError::Invalid(
            "pipeline seed manifest contains an invalid filename".to_string(),
        ));
    }
    Ok(manifest.bundled.into_iter().collect())
}

fn read_seed_manifest(dir: &Path) -> Result<Option<HashSet<String>>, PipelineError> {
    let path = dir.join(SEED_MANIFEST);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read(&path)?;
    let manifest: SeedManifest = serde_json::from_slice(&raw).map_err(|e| {
        PipelineError::Invalid(format!(
            "invalid pipeline seed manifest {}: {e}",
            path.display()
        ))
    })?;
    validate_manifest(manifest).map(Some)
}

fn current_seed_manifest() -> SeedManifest {
    SeedManifest {
        version: SEED_MANIFEST_VERSION,
        bundled: BUNDLED
            .iter()
            .map(|(name, _)| (*name).to_string())
            .collect(),
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    std::fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: Both paths are encoded as owned, NUL-terminated UTF-16 buffers
    // that remain alive for the duration of the call.
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn write_seed_manifest(dir: &Path) -> Result<(), PipelineError> {
    let manifest = current_seed_manifest();
    let mut content = serde_json::to_vec_pretty(&manifest).map_err(|e| {
        PipelineError::Invalid(format!("failed to serialize pipeline seed manifest: {e}"))
    })?;
    content.push(b'\n');

    let nonce = SEED_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_path = dir.join(format!(
        "{SEED_MANIFEST}.tmp-{}-{nonce}",
        std::process::id()
    ));
    let result = (|| -> Result<(), std::io::Error> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(&content)?;
        file.sync_all()?;
        replace_file(&temp_path, &dir.join(SEED_MANIFEST))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result.map_err(PipelineError::Io)
}

fn remove_created(paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        if let Err(e) = std::fs::remove_file(path) {
            tracing::warn!(
                "failed to roll back seeded pipeline {}: {}",
                path.display(),
                e
            );
        }
    }
}

/// Reconcile bundled defaults without overwriting existing files or resurrecting
/// known defaults that the user deleted. A private manifest records the bundle
/// names seen by the last successful reconciliation. If the operator deletes
/// every TOML, all defaults are re-seeded (the established empty-dir behavior).
pub fn ensure_seeded(dir: &Path) -> Result<(), PipelineError> {
    let _seed_guard = SEED_LOCK
        .lock()
        .map_err(|_| PipelineError::Invalid("pipeline seed lock is poisoned".to_string()))?;
    std::fs::create_dir_all(dir)?;

    // Validate existing bookkeeping even when the last TOML was deleted. A
    // corrupt manifest must not be guessed through before the empty-dir reseed.
    let recorded = read_seed_manifest(dir)?;
    let known: HashSet<String> = if !has_toml(dir) {
        HashSet::new()
    } else {
        recorded.unwrap_or_else(|| {
            LEGACY_BUNDLED
                .iter()
                .map(|name| (*name).to_string())
                .collect()
        })
    };

    let mut created = Vec::new();
    for (name, content) in BUNDLED {
        if known.contains(*name) {
            continue;
        }
        let path = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                created.push(path);
                if let Err(e) = file.write_all(content.as_bytes()) {
                    remove_created(&created);
                    return Err(PipelineError::Io(e));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && path.is_file() => {}
            Err(e) => {
                remove_created(&created);
                return Err(PipelineError::Io(e));
            }
        }
    }

    if let Err(e) = write_seed_manifest(dir) {
        remove_created(&created);
        return Err(e);
    }
    Ok(())
}

/// Load every valid pipeline from `dir`, seeding defaults first if empty.
/// Malformed files are skipped with a warning so a single bad user file never
/// breaks the endpoint. Sorted: bundled order first, then alphabetical.
pub fn load_pipelines(dir: &Path) -> Vec<Pipeline> {
    if let Err(e) = ensure_seeded(dir) {
        tracing::warn!("failed to seed pipelines dir {}: {}", dir.display(), e);
    }
    let mut out: Vec<Pipeline> = Vec::new();
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            tracing::warn!("failed to read pipelines dir {}: {}", dir.display(), e);
            return out;
        }
    };
    for entry in rd.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|x| x.to_str()) != Some("toml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("skip pipeline {}: {}", path.display(), e);
                continue;
            }
        };
        match parse_pipeline(stem, &raw) {
            Ok(p) => out.push(p),
            Err(e) => tracing::warn!("skip invalid pipeline {}: {}", path.display(), e),
        }
    }
    sort_by_bundled_order(&mut out, |p| p.id.as_str());
    out
}

/// Read the raw TOML of a single pipeline (for the Settings editor).
pub fn read_raw(dir: &Path, id: &str) -> Result<String, PipelineError> {
    validate_id(id)?;
    let path = dir.join(format!("{id}.toml"));
    if !path.exists() {
        return Err(PipelineError::NotFound);
    }
    Ok(std::fs::read_to_string(path)?)
}

/// Validate and write raw TOML for a pipeline. Rejects content that fails to
/// parse **before** touching disk, returning the parsed pipeline on success.
pub fn write_raw(dir: &Path, id: &str, content: &str) -> Result<Pipeline, PipelineError> {
    validate_id(id)?;
    let pipeline = parse_pipeline(id, content)?;
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(format!("{id}.toml")), content)?;
    Ok(pipeline)
}

/// Restore a single bundled pipeline to its shipped default.
pub fn reset_one(dir: &Path, id: &str) -> Result<Pipeline, PipelineError> {
    validate_id(id)?;
    let file = format!("{id}.toml");
    let Some((_, content)) = BUNDLED.iter().find(|(n, _)| *n == file) else {
        return Err(PipelineError::NotFound);
    };
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(&file), content)?;
    parse_pipeline(id, content)
}

/// Overwrite all bundled pipelines with their shipped defaults, propagating any
/// write error rather than returning a stale/partial list.
pub fn reset_all(dir: &Path) -> Result<Vec<Pipeline>, PipelineError> {
    std::fs::create_dir_all(dir)?;
    for (name, content) in BUNDLED {
        std::fs::write(dir.join(name), content)?;
    }
    Ok(load_pipelines(dir))
}

/// Delete a pipeline file. Stays deleted while other pipeline files remain.
pub fn delete_pipeline(dir: &Path, id: &str) -> Result<(), PipelineError> {
    validate_id(id)?;
    let path = dir.join(format!("{id}.toml"));
    if !path.exists() {
        return Err(PipelineError::NotFound);
    }
    std::fs::remove_file(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Unique temp dir per test invocation (avoids a tempfile dev-dependency).
    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let p = std::env::temp_dir().join(format!(
                "vk-pipelines-test-{}-{}",
                std::process::id(),
                n
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parses_valid_pipeline() {
        let raw = r#"
            name = "Demo"
            [[stage]]
            id = "spec"
            label = "Create spec"
            default_enabled = true
            prompt = "Write a spec."
        "#;
        let p = parse_pipeline("demo", raw).unwrap();
        assert_eq!(p.id, "demo");
        assert_eq!(p.name, "Demo");
        assert_eq!(p.stages.len(), 1);
        assert_eq!(p.stages[0].id, "spec");
        assert_eq!(p.stages[0].prompt_fragment, "Write a spec.");
        assert!(p.stages[0].default_enabled);
    }

    #[test]
    fn rejects_duplicate_stage_ids() {
        let raw = r#"
            name = "Dup"
            [[stage]]
            id = "spec"
            label = "A"
            prompt = "x"
            [[stage]]
            id = "spec"
            label = "B"
            prompt = "y"
        "#;
        assert!(matches!(
            parse_pipeline("dup", raw),
            Err(PipelineError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_empty_fields_and_bad_ids() {
        let empty_label = "name=\"X\"\n[[stage]]\nid=\"a\"\nlabel=\"\"\nprompt=\"p\"\n";
        assert!(matches!(
            parse_pipeline("x", empty_label),
            Err(PipelineError::Invalid(_))
        ));
        let bad_stage = "name=\"X\"\n[[stage]]\nid=\"a b\"\nlabel=\"l\"\nprompt=\"p\"\n";
        assert!(matches!(
            parse_pipeline("x", bad_stage),
            Err(PipelineError::Invalid(_))
        ));
    }

    #[test]
    fn validate_id_rejects_traversal() {
        assert!(validate_id("..").is_err());
        assert!(validate_id("a/b").is_err());
        assert!(validate_id("a\\b").is_err());
        assert!(validate_id("").is_err());
        assert!(validate_id("basic").is_ok());
        assert!(validate_id("my_pipeline-2").is_ok());
    }

    #[test]
    fn seeds_defaults_into_empty_dir() {
        let d = TmpDir::new();
        let pipelines = load_pipelines(d.path());
        let ids: Vec<_> = pipelines.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["basic", "wikillm", "speckit", "parallel-subagents"]
        );
        assert!(d.path().join(SEED_MANIFEST).is_file());
    }

    #[test]
    fn seeds_new_bundle_into_manifestless_existing_install() {
        let d = TmpDir::new();
        for (name, content) in BUNDLED
            .iter()
            .filter(|(name, _)| LEGACY_BUNDLED.contains(name))
        {
            std::fs::write(d.path().join(name), content).unwrap();
        }

        assert!(!d.path().join("parallel-subagents.toml").exists());
        ensure_seeded(d.path()).unwrap();

        assert!(d.path().join("parallel-subagents.toml").is_file());
        assert!(d.path().join(SEED_MANIFEST).is_file());
    }

    #[test]
    fn does_not_reseed_deleted_file_when_others_remain() {
        let d = TmpDir::new();
        ensure_seeded(d.path()).unwrap();
        delete_pipeline(d.path(), "basic").unwrap();
        let pipelines = load_pipelines(d.path());
        assert!(!pipelines.iter().any(|p| p.id == "basic"));
        assert!(pipelines.iter().any(|p| p.id == "wikillm"));
    }

    #[test]
    fn preserves_local_edits_while_reconciling() {
        let d = TmpDir::new();
        ensure_seeded(d.path()).unwrap();
        let edited = "name = \"My Basic\"\n# keep this local edit\n";
        std::fs::write(d.path().join("basic.toml"), edited).unwrap();

        ensure_seeded(d.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(d.path().join("basic.toml")).unwrap(),
            edited
        );
    }

    #[test]
    fn seed_reconciliation_is_idempotent() {
        let d = TmpDir::new();
        ensure_seeded(d.path()).unwrap();
        let before: Vec<_> = BUNDLED
            .iter()
            .map(|(name, _)| (*name, std::fs::read(d.path().join(name)).unwrap()))
            .collect();
        let manifest_before = std::fs::read(d.path().join(SEED_MANIFEST)).unwrap();

        ensure_seeded(d.path()).unwrap();

        for (name, content) in before {
            assert_eq!(std::fs::read(d.path().join(name)).unwrap(), content);
        }
        assert_eq!(
            std::fs::read(d.path().join(SEED_MANIFEST)).unwrap(),
            manifest_before
        );
    }

    #[test]
    fn rejects_invalid_seed_manifest_without_writing() {
        let d = TmpDir::new();
        std::fs::write(
            d.path().join("custom.toml"),
            "name = \"Custom\"\n[[stage]]\nid=\"a\"\nlabel=\"A\"\nprompt=\"p\"\n",
        )
        .unwrap();
        std::fs::write(d.path().join(SEED_MANIFEST), b"not json").unwrap();

        assert!(ensure_seeded(d.path()).is_err());
        assert!(!d.path().join("parallel-subagents.toml").exists());
        assert_eq!(
            std::fs::read(d.path().join(SEED_MANIFEST)).unwrap(),
            b"not json"
        );
    }

    #[test]
    fn rejects_invalid_seed_manifest_when_no_toml_remains() {
        let d = TmpDir::new();
        std::fs::write(d.path().join(SEED_MANIFEST), b"not json").unwrap();

        assert!(ensure_seeded(d.path()).is_err());

        assert!(!d.path().join("basic.toml").exists());
        assert_eq!(
            std::fs::read(d.path().join(SEED_MANIFEST)).unwrap(),
            b"not json"
        );
    }

    #[test]
    fn failed_seed_rolls_back_created_files_and_does_not_commit_manifest() {
        let d = TmpDir::new();
        std::fs::create_dir(d.path().join("wikillm.toml")).unwrap();

        assert!(ensure_seeded(d.path()).is_err());

        assert!(!d.path().join("basic.toml").exists());
        assert!(d.path().join("wikillm.toml").is_dir());
        assert!(!d.path().join(SEED_MANIFEST).exists());
    }

    #[test]
    fn skips_malformed_file_but_keeps_valid_ones() {
        let d = TmpDir::new();
        ensure_seeded(d.path()).unwrap();
        std::fs::write(d.path().join("broken.toml"), "this is = not [valid").unwrap();
        let pipelines = load_pipelines(d.path());
        assert!(pipelines.iter().any(|p| p.id == "basic"));
        assert!(!pipelines.iter().any(|p| p.id == "broken"));
    }

    #[test]
    fn write_raw_rejects_invalid_toml() {
        let d = TmpDir::new();
        assert!(write_raw(d.path(), "custom", "not valid = [").is_err());
        assert!(!d.path().join("custom.toml").exists());
    }

    #[test]
    fn reset_one_and_all_restore_bundled() {
        let d = TmpDir::new();
        ensure_seeded(d.path()).unwrap();
        std::fs::write(d.path().join("basic.toml"), "name=\"Hacked\"\n").unwrap();
        let restored = reset_one(d.path(), "basic").unwrap();
        assert_eq!(restored.name, "Basic");
        assert!(reset_one(d.path(), "not-bundled").is_err());
        let all = reset_all(d.path()).unwrap();
        assert!(all.iter().any(|p| p.id == "basic" && p.name == "Basic"));
    }

    #[test]
    fn validate_reports_valid_pipeline() {
        let raw = r#"
            name = "Demo"
            [[stage]]
            id = "spec"
            label = "Create spec"
            prompt = "Write a spec."
        "#;
        let v = validate("demo", raw);
        assert!(v.valid);
        assert!(v.error.is_none());
    }

    #[test]
    fn validate_reports_toml_syntax_error_with_line_col() {
        let raw = "name = ";
        let v = validate("broken", raw);
        assert!(!v.valid);
        let err = v.error.expect("expected a structured error");
        assert!(err.line.is_some());
        assert!(err.column.is_some());
    }

    #[test]
    fn validate_reports_semantic_error_without_line_col() {
        let raw = r#"
            name = "Dup"
            [[stage]]
            id = "spec"
            label = "A"
            prompt = "x"
            [[stage]]
            id = "spec"
            label = "B"
            prompt = "y"
        "#;
        let v = validate("dup", raw);
        assert!(!v.valid);
        let err = v.error.expect("expected a structured error");
        assert!(err.line.is_none());
        assert!(err.column.is_none());
    }

    #[test]
    fn load_pipeline_statuses_reports_good_and_broken_files() {
        let d = TmpDir::new();
        std::fs::write(
            d.path().join("good.toml"),
            "name = \"Good\"\n[[stage]]\nid = \"a\"\nlabel = \"A\"\nprompt = \"p\"\n",
        )
        .unwrap();
        std::fs::write(d.path().join("broken.toml"), "this is = not [valid").unwrap();
        let statuses = load_pipeline_statuses(d.path());
        let good = statuses.iter().find(|s| s.id == "good").unwrap();
        assert!(good.valid);
        assert_eq!(good.stage_count, Some(1));
        assert!(good.error.is_none());
        let broken = statuses.iter().find(|s| s.id == "broken").unwrap();
        assert!(!broken.valid);
        assert!(broken.error.is_some());
    }

    #[test]
    fn bundled_parallel_subagents_is_valid() {
        let d = TmpDir::new();
        let pipelines = load_pipelines(d.path());
        let p = pipelines
            .iter()
            .find(|p| p.id == "parallel-subagents")
            .expect("parallel-subagents pipeline seeded");
        let ids: Vec<_> = p.stages.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["fanout", "analyze", "iterate", "code-review"]);

        let fanout = p.stages.iter().find(|s| s.id == "fanout").unwrap();
        assert!(fanout.default_enabled);
        assert!(fanout.heavy);
        let prompt = fanout.prompt_fragment.to_lowercase();
        assert!(prompt.contains("parallel"));
        assert!(prompt.contains("claude"));
        assert!(prompt.contains("codex"));
        assert!(prompt.contains("grok"));

        // The loop stage caps iterations at N so it cannot run unbounded.
        let iterate = p.stages.iter().find(|s| s.id == "iterate").unwrap();
        assert!(iterate.prompt_fragment.contains('N'));
    }

    #[test]
    fn bundled_basic_spec_prompt_is_verbatim() {
        let d = TmpDir::new();
        let pipelines = load_pipelines(d.path());
        let basic = pipelines.iter().find(|p| p.id == "basic").unwrap();
        let spec = basic.stages.iter().find(|s| s.id == "spec").unwrap();
        assert_eq!(
            spec.prompt_fragment,
            "Write a technical spec for this task and save it to `SPEC.md` at the repo root before implementing."
        );
    }
}
