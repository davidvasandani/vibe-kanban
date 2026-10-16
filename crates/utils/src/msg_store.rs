use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, RwLock},
};

use futures::{StreamExt, future};
use tokio::{sync::broadcast, task::JoinHandle};
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};

use crate::{log_msg::LogMsg, stream_lines::LinesStreamExt};

// 100 MB Limit
const HISTORY_BYTES: usize = 100000 * 1024;

#[derive(Clone)]
struct StoredMsg {
    msg: LogMsg,
    bytes: usize,
}

struct Inner {
    history: VecDeque<StoredMsg>,
    total_bytes: usize,
    entry_timestamps: Option<EntryTimestamps>,
}

// Bound metadata independently of the byte-capped message history. Once full,
// new entries keep unknown times rather than destabilizing existing timestamps.
const MAX_ENTRY_TIMESTAMPS: usize = 100_000;

#[derive(Default)]
struct EntryTimestamps(BTreeMap<usize, String>);

fn entry_index(path: &str) -> Option<usize> {
    path.strip_prefix("/entries/")?.parse().ok()
}

impl EntryTimestamps {
    fn enrich(&mut self, patch: &mut json_patch::Patch, now: &str) {
        for operation in &mut patch.0 {
            let (path, value, insert) = match operation {
                json_patch::PatchOperation::Add(op) => (op.path.as_str(), &mut op.value, true),
                json_patch::PatchOperation::Replace(op) => (op.path.as_str(), &mut op.value, false),
                json_patch::PatchOperation::Remove(op) => {
                    if op.path.as_str() == "/entries" {
                        self.0.clear();
                    } else if let Some(index) = entry_index(op.path.as_str()) {
                        self.0.remove(&index);
                        let tail = self.0.split_off(&index);
                        self.0
                            .extend(tail.into_iter().map(|(key, time)| (key - 1, time)));
                    }
                    continue;
                }
                _ => continue,
            };
            if path == "/entries" {
                self.0.clear();
                continue;
            }
            let Some(index) = entry_index(path) else {
                continue;
            };
            if insert {
                // JSON Patch add inserts into the array, even at an occupied index.
                let tail = self.0.split_off(&index);
                self.0.extend(
                    tail.into_iter()
                        .filter_map(|(key, time)| key.checked_add(1).map(|key| (key, time))),
                );
            }
            if value.get("type").and_then(serde_json::Value::as_str) != Some("NORMALIZED_ENTRY") {
                self.0.remove(&index);
                continue;
            }
            let Some(content) = value
                .get_mut("content")
                .and_then(serde_json::Value::as_object_mut)
            else {
                continue;
            };
            let source = content
                .get("timestamp")
                .and_then(serde_json::Value::as_str)
                .filter(|time| chrono::DateTime::parse_from_rfc3339(time).is_ok());
            let timestamp = source
                .map(str::to_owned)
                .or_else(|| self.0.get(&index).cloned())
                .or_else(|| (self.0.len() < MAX_ENTRY_TIMESTAMPS).then(|| now.to_owned()));
            if let Some(timestamp) = timestamp {
                if self.0.contains_key(&index) || self.0.len() < MAX_ENTRY_TIMESTAMPS {
                    self.0.insert(index, timestamp.clone());
                }
                content.insert("timestamp".into(), serde_json::Value::String(timestamp));
            }
        }
    }
}

pub struct MsgStore {
    inner: RwLock<Inner>,
    sender: broadcast::Sender<LogMsg>,
}

impl Default for MsgStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MsgStore {
    pub fn new() -> Self {
        Self::with_capacity(100000)
    }

    /// Record first observation times for a newly started execution only.
    /// Replayed/adopted executions must use `new` so old output is not dated now.
    pub fn live() -> Self {
        let mut store = Self::new();
        store.inner.get_mut().unwrap().entry_timestamps = Some(EntryTimestamps::default());
        store
    }

    fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            inner: RwLock::new(Inner {
                history: VecDeque::with_capacity(32),
                total_bytes: 0,
                entry_timestamps: None,
            }),
            sender,
        }
    }

    pub fn push(&self, mut msg: LogMsg) {
        let mut inner = self.inner.write().unwrap();
        if let (Some(timestamps), LogMsg::JsonPatch(patch)) =
            (&mut inner.entry_timestamps, &mut msg)
        {
            timestamps.enrich(patch, &chrono::Utc::now().to_rfc3339());
        }
        let bytes = msg.approx_bytes();
        while inner.total_bytes.saturating_add(bytes) > HISTORY_BYTES {
            if let Some(front) = inner.history.pop_front() {
                inner.total_bytes = inner.total_bytes.saturating_sub(front.bytes);
            } else {
                break;
            }
        }
        inner.history.push_back(StoredMsg {
            msg: msg.clone(),
            bytes,
        });
        inner.total_bytes = inner.total_bytes.saturating_add(bytes);
        // Publish while holding the same lock used by the history handoff.
        // A new consumer therefore sees a message in exactly one side of its
        // history/live boundary (existing consumers still receive it live).
        let _ = self.sender.send(msg);
    }

    // Convenience
    pub fn push_stdout<S: Into<String>>(&self, s: S) {
        self.push(LogMsg::Stdout(s.into()));
    }

    pub fn push_patch(&self, patch: json_patch::Patch) {
        self.push(LogMsg::JsonPatch(patch));
    }

    pub fn push_session_id(&self, session_id: String) {
        self.push(LogMsg::SessionId(session_id));
    }

    pub fn push_message_id(&self, id: String) {
        self.push(LogMsg::MessageId(id));
    }

    pub fn push_finished(&self) {
        self.push(LogMsg::Finished);
    }

    pub fn get_receiver(&self) -> broadcast::Receiver<LogMsg> {
        self.sender.subscribe()
    }

    pub fn get_history(&self) -> Vec<LogMsg> {
        self.select_history(|msg| Some(msg.clone()))
    }

    /// Retained history, cloning only what `select` keeps.
    ///
    /// Callers that want one variant — normalized patches, say — would
    /// otherwise pay [`Self::get_history`]'s full deep copy of a history that
    /// is mostly raw stdout, while holding the same lock `push` needs. That
    /// cost lands on the log forwarder, so keep the selection inside the
    /// read guard rather than filtering a complete clone afterwards.
    pub fn select_history<T>(&self, select: impl FnMut(&LogMsg) -> Option<T>) -> Vec<T> {
        let mut select = select;
        self.inner
            .read()
            .unwrap()
            .history
            .iter()
            .filter_map(|stored| select(&stored.msg))
            .collect()
    }

    /// [`Self::history_plus_stream`], plus how many retained history messages
    /// match `count`, both taken under the same lock so the count describes
    /// exactly the history the stream replays.
    pub fn history_plus_stream_counting(
        &self,
        count: impl Fn(&LogMsg) -> bool,
    ) -> (
        usize,
        futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>,
    ) {
        let inner = self.inner.read().unwrap();
        let rx = self.sender.subscribe();
        let history = inner
            .history
            .iter()
            .map(|s| s.msg.clone())
            .collect::<Vec<_>>();
        drop(inner);
        let matched = history.iter().filter(|msg| count(msg)).count();
        (matched, Self::chain_history_and_live(history, rx))
    }

    fn chain_history_and_live(
        history: Vec<LogMsg>,
        rx: broadcast::Receiver<LogMsg>,
    ) -> futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>> {
        let hist = futures::stream::iter(history.into_iter().map(Ok::<_, std::io::Error>));
        let live = BroadcastStream::new(rx).map(|res| match res {
            Ok(msg) => Ok(msg),
            Err(BroadcastStreamRecvError::Lagged(n)) => Err(std::io::Error::other(format!(
                "MsgStore history/live stream lagged by {n} messages; resubscribe required"
            ))),
        });
        Box::pin(hist.chain(live))
    }

    /// History then live, as `LogMsg`.
    pub fn history_plus_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>> {
        // Subscribe while history is read-locked. `push` publishes under the
        // write lock, closing both the history→subscribe loss window and the
        // subscribe→history duplication window.
        let inner = self.inner.read().unwrap();
        let rx = self.sender.subscribe();
        let history = inner
            .history
            .iter()
            .map(|s| s.msg.clone())
            .collect::<Vec<_>>();
        drop(inner);

        Self::chain_history_and_live(history, rx)
    }

    pub fn stdout_chunked_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<String, std::io::Error>> {
        self.history_plus_stream()
            .take_while(|res| future::ready(!matches!(res, Ok(LogMsg::Finished))))
            .filter_map(|res| async move {
                match res {
                    Ok(LogMsg::Stdout(s)) => Some(Ok(s)),
                    Err(error) => Some(Err(error)),
                    _ => None,
                }
            })
            .boxed()
    }

    pub fn stdout_lines_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, std::io::Result<String>> {
        self.stdout_chunked_stream().lines()
    }

    pub fn stderr_chunked_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<String, std::io::Error>> {
        self.history_plus_stream()
            .take_while(|res| future::ready(!matches!(res, Ok(LogMsg::Finished))))
            .filter_map(|res| async move {
                match res {
                    Ok(LogMsg::Stderr(s)) => Some(Ok(s)),
                    Err(error) => Some(Err(error)),
                    _ => None,
                }
            })
            .boxed()
    }

    /// Forward a stream of typed log messages into this store.
    pub fn spawn_forwarder<S, E>(self: Arc<Self>, stream: S) -> JoinHandle<()>
    where
        S: futures::Stream<Item = Result<LogMsg, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        tokio::spawn(async move {
            tokio::pin!(stream);

            while let Some(next) = stream.next().await {
                match next {
                    Ok(msg) => self.push(msg),
                    Err(e) => self.push(LogMsg::Stderr(format!("stream error: {e}"))),
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[tokio::test]
    async fn history_handoff_delivers_each_message_once() {
        let store = MsgStore::with_capacity(8);
        store.push_stdout("history");
        let mut stream = store.history_plus_stream();
        store.push_stdout("live");

        assert!(
            matches!(stream.next().await, Some(Ok(LogMsg::Stdout(value))) if value == "history")
        );
        assert!(matches!(stream.next().await, Some(Ok(LogMsg::Stdout(value))) if value == "live"));
    }

    #[tokio::test]
    async fn broadcast_lag_is_a_stream_error() {
        let store = MsgStore::with_capacity(2);
        let mut stream = store.history_plus_stream();
        for value in 0..3 {
            store.push_stdout(value.to_string());
        }

        let error = stream
            .next()
            .await
            .expect("lag result")
            .expect_err("lag must invalidate stream authority");
        assert!(error.to_string().contains("lagged by 1 messages"));
        assert!(error.to_string().contains("resubscribe required"));
    }
}

#[cfg(test)]
mod timestamp_tests {
    use serde_json::json;

    use super::*;

    const FIRST: &str = "2026-10-16T09:19:00Z";
    const LATER: &str = "2026-10-16T10:34:00Z";

    fn entry(op: &str, index: usize, timestamp: Option<&str>) -> json_patch::Patch {
        serde_json::from_value(json!([{
            "op": op, "path": format!("/entries/{index}"),
            "value": { "type": "NORMALIZED_ENTRY", "content": {
                "timestamp": timestamp, "content": "step",
                "entry_type": { "type": "assistant_message" }
            }}
        }]))
        .unwrap()
    }

    fn time(patch: &json_patch::Patch) -> serde_json::Value {
        serde_json::to_value(patch).unwrap()[0]["value"]["content"]["timestamp"].clone()
    }

    #[test]
    fn distinct_steps_and_replacements_keep_their_observed_times() {
        let mut times = EntryTimestamps::default();
        let mut first = entry("add", 0, None);
        let mut later = entry("add", 1, None);
        times.enrich(&mut first, FIRST);
        times.enrich(&mut later, LATER);
        assert_eq!(time(&first), FIRST);
        assert_eq!(time(&later), LATER);
        let mut update = entry("replace", 0, None);
        times.enrich(&mut update, LATER);
        assert_eq!(time(&update), FIRST);
    }

    #[test]
    fn source_time_wins_and_survives_a_missing_or_invalid_update() {
        let mut times = EntryTimestamps::default();
        let mut first = entry("add", 0, None);
        times.enrich(&mut first, FIRST);
        let mut source = entry("replace", 0, Some(LATER));
        times.enrich(&mut source, FIRST);
        assert_eq!(time(&source), LATER);
        for source in [None, Some("invalid")] {
            let mut update = entry("replace", 0, source);
            times.enrich(&mut update, FIRST);
            assert_eq!(time(&update), LATER);
        }
    }

    #[test]
    fn removal_allows_a_new_identity_at_the_same_path() {
        let mut times = EntryTimestamps::default();
        times.enrich(&mut entry("add", 0, None), FIRST);
        let mut remove =
            serde_json::from_value(json!([{"op":"remove", "path":"/entries/0"}])).unwrap();
        times.enrich(&mut remove, FIRST);
        let mut next = entry("add", 0, None);
        times.enrich(&mut next, LATER);
        assert_eq!(time(&next), LATER);
    }

    #[test]
    fn array_insertions_and_amp_resume_clear_preserve_identity() {
        let mut times = EntryTimestamps::default();
        times.enrich(&mut entry("add", 0, None), FIRST);
        times.enrich(&mut entry("add", 1, None), FIRST);
        let mut inserted = entry("add", 0, None);
        times.enrich(&mut inserted, LATER);
        assert_eq!(time(&inserted), LATER);
        let mut shifted = entry("replace", 1, None);
        times.enrich(&mut shifted, LATER);
        assert_eq!(time(&shifted), FIRST);
        // AmpResume removes index zero repeatedly before resetting its provider.
        for _ in 0..3 {
            let mut remove = serde_json::from_value(json!([
                {"op":"remove", "path":"/entries/0"}
            ]))
            .unwrap();
            times.enrich(&mut remove, LATER);
        }
        assert!(times.0.is_empty());
        for index in 0..3 {
            let mut new = entry("add", index, None);
            times.enrich(&mut new, LATER);
            assert_eq!(time(&new), LATER);
        }
    }

    #[test]
    fn metadata_limit_keeps_existing_identity_and_leaves_new_time_unknown() {
        let mut times = EntryTimestamps(
            (0..MAX_ENTRY_TIMESTAMPS)
                .map(|index| (index, FIRST.to_owned()))
                .collect(),
        );
        let mut next = entry("add", MAX_ENTRY_TIMESTAMPS, None);
        times.enrich(&mut next, LATER);
        assert!(time(&next).is_null());
        let mut update = entry("replace", 0, None);
        times.enrich(&mut update, LATER);
        assert_eq!(time(&update), FIRST);
        assert_eq!(times.0.len(), MAX_ENTRY_TIMESTAMPS);
    }

    #[test]
    fn live_publication_and_serialized_replay_preserve_the_same_time() {
        let live = MsgStore::live();
        let mut receiver = live.get_receiver();
        live.push_patch(entry("add", 0, None));
        let message = receiver.try_recv().unwrap();
        let LogMsg::JsonPatch(patch) = &message else {
            panic!("expected patch")
        };
        let captured = time(patch);
        assert!(chrono::DateTime::parse_from_rfc3339(captured.as_str().unwrap()).is_ok());
        let serialized = serde_json::to_string(&message).unwrap();
        let replay = MsgStore::new();
        replay.push(serde_json::from_str(&serialized).unwrap());
        let LogMsg::JsonPatch(patch) = &replay.get_history()[0] else {
            panic!("expected patch")
        };
        assert_eq!(time(patch), captured);
        replay.push_patch(entry("replace", 0, None));
        let LogMsg::JsonPatch(patch) = &replay.get_history()[1] else {
            panic!("expected patch")
        };
        assert!(
            time(patch).is_null(),
            "historical normalization must not stamp now"
        );
    }
}
