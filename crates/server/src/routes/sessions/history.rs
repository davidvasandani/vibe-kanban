//! `GET /api/sessions/{id}/history`: a session's chat log across every
//! coding-agent turn, filtered by time window, keyword, role and turn, then
//! paged. `/messages` only ever reads one execution; this is the
//! session-wide counterpart the `search_session_messages` MCP tool calls.

use std::{
    collections::HashSet,
    future::Future,
    time::{Duration, Instant},
};

use axum::{
    Extension,
    extract::{Query, State},
    response::Json as ResponseJson,
};
use chrono::{DateTime, Utc};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessRunReason},
    session::Session,
};
use deployment::Deployment;
use executors::logs::NormalizedEntry;
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    routes::execution_processes::{contains_folded, match_snippet, parse_roles, project_entry},
};

const DEFAULT_HISTORY_LIMIT: usize = 50;
const MAX_HISTORY_LIMIT: usize = 200;
/// Request-wide budget, checked before each turn is loaded. The turn already
/// loading when it passes still finishes, so its normalized sidecar is
/// written and a retry is cheaper.
const HISTORY_BUDGET: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum HistoryOrder {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Default, Deserialize)]
pub struct SessionHistoryQuery {
    /// Inclusive RFC 3339 lower bound on each message's effective time.
    pub since: Option<String>,
    /// Inclusive RFC 3339 upper bound on each message's effective time.
    pub until: Option<String>,
    /// Case-insensitive substring to match against untruncated text.
    pub query: Option<String>,
    /// Comma-separated subset of `user`, `assistant`, `system`, `tool`.
    pub roles: Option<String>,
    /// Restrict to one coding-agent turn of this session.
    pub execution_id: Option<Uuid>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    #[serde(default)]
    pub order: HistoryOrder,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct SessionHistoryMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    /// The entry's own timestamp as the executor reported it, if any.
    pub created_at: Option<String>,
    /// Start of the owning turn; the effective time when `created_at` is
    /// absent or unparseable.
    pub turn_started_at: String,
    pub execution_id: String,
    /// 0-based position among the session's coding-agent turns.
    pub turn_index: usize,
    /// Context around the first keyword hit, when `query` was given.
    pub match_snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct SessionHistoryResponse {
    pub session_id: String,
    pub turn_count: usize,
    pub turns_scanned: usize,
    pub total_matches: usize,
    pub offset: usize,
    pub limit: usize,
    pub order: HistoryOrder,
    pub has_more: bool,
    /// False when any candidate turn went unsearched: the budget stopped
    /// the scan (`unscanned_execution_ids`, worth a retry) or its log could
    /// not be read (`unreadable_execution_ids`).
    pub complete: bool,
    pub unscanned_execution_ids: Vec<String>,
    pub unreadable_execution_ids: Vec<String>,
    pub messages: Vec<SessionHistoryMessage>,
}

/// The execution metadata the scan needs for one coding-agent turn.
#[derive(Debug, Clone)]
struct Turn {
    execution_id: Uuid,
    turn_index: usize,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Default)]
struct HistoryFilter {
    since: Option<DateTime<Utc>>,
    until: Option<DateTime<Utc>>,
    needle: Option<String>,
    roles: Option<HashSet<String>>,
}

impl HistoryFilter {
    /// Validation errors are plain messages; the handler maps them to 400.
    fn from_query(query: &SessionHistoryQuery) -> Result<Self, String> {
        let since = parse_bound("since", query.since.as_deref())?;
        let until = parse_bound("until", query.until.as_deref())?;
        if let (Some(since), Some(until)) = (since, until)
            && since > until
        {
            return Err("`since` must not be later than `until`".to_string());
        }
        let needle = query
            .query
            .as_deref()
            .map(str::trim)
            .filter(|needle| !needle.is_empty())
            .map(str::to_string);
        Ok(Self {
            since,
            until,
            needle,
            roles: parse_roles(query.roles.as_deref()),
        })
    }

    /// Whether a turn spanning `[started_at, completed_at or now]` can hold
    /// a message inside the window, judged from execution metadata alone.
    fn turn_overlaps(&self, turn: &Turn, now: DateTime<Utc>) -> bool {
        let end = turn.completed_at.unwrap_or(now).max(turn.started_at);
        self.since.is_none_or(|since| end >= since)
            && self.until.is_none_or(|until| turn.started_at <= until)
    }

    fn time_matches(&self, at: DateTime<Utc>) -> bool {
        self.since.is_none_or(|since| at >= since) && self.until.is_none_or(|until| at <= until)
    }

    /// Projects and filters one turn's entries, oldest first.
    fn project_turn(&self, turn: &Turn, entries: &[NormalizedEntry]) -> Vec<SessionHistoryMessage> {
        let turn_started_at = turn.started_at.to_rfc3339();
        entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let (message, untruncated) =
                    project_entry(index, entry, turn.execution_id, self.roles.as_ref())?;
                let at = effective_time(message.created_at.as_deref(), turn.started_at);
                if !self.time_matches(at) {
                    return None;
                }
                let match_snippet = match &self.needle {
                    Some(needle) => {
                        if !contains_folded(untruncated, needle) {
                            return None;
                        }
                        match_snippet(untruncated, needle)
                    }
                    None => None,
                };
                Some(SessionHistoryMessage {
                    id: message.id,
                    role: message.role,
                    text: message.text,
                    created_at: message.created_at,
                    turn_started_at: turn_started_at.clone(),
                    execution_id: message.execution_id,
                    turn_index: turn.turn_index,
                    match_snippet,
                })
            })
            .collect()
    }
}

fn parse_bound(name: &str, value: Option<&str>) -> Result<Option<DateTime<Utc>>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| Some(parsed.with_timezone(&Utc)))
        .map_err(|error| {
            format!("`{name}` must be an RFC 3339 timestamp (e.g. 2026-10-11T09:30:00Z): {error}")
        })
}

/// An entry's own RFC 3339 timestamp, else its turn's start. Never "now".
fn effective_time(entry_timestamp: Option<&str>, turn_started_at: DateTime<Utc>) -> DateTime<Utc> {
    entry_timestamp
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|parsed| parsed.with_timezone(&Utc))
        .unwrap_or(turn_started_at)
}

fn clamp_history_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_HISTORY_LIMIT)
        .clamp(1, MAX_HISTORY_LIMIT)
}

/// Orders oldest-first matches and returns one page plus `has_more`.
fn paginate<T>(
    mut items: Vec<T>,
    order: HistoryOrder,
    offset: usize,
    limit: usize,
) -> (Vec<T>, bool) {
    if order == HistoryOrder::Desc {
        items.reverse();
    }
    let total = items.len();
    let page: Vec<T> = items.into_iter().skip(offset).take(limit).collect();
    let has_more = offset.saturating_add(page.len()) < total;
    (page, has_more)
}

#[derive(Debug, Default)]
struct ScanResult {
    matches: Vec<SessionHistoryMessage>,
    turns_scanned: usize,
    unscanned: Vec<Uuid>,
    unreadable: Vec<Uuid>,
}

/// Loads candidate turns one at a time: a cold legacy turn competes for the
/// shared historical-normalization permit, so fanning out would only queue.
/// Once `budget_spent` reports true, no further turn is loaded and the rest
/// are reported as unscanned. A turn whose log could not be read (`None`)
/// is reported as unreadable rather than counted as an empty, searched turn.
async fn scan_turns<Load, Fut>(
    turns: &[Turn],
    filter: &HistoryFilter,
    budget_spent: impl Fn() -> bool,
    mut load: Load,
) -> ScanResult
where
    Load: FnMut(Uuid) -> Fut,
    Fut: Future<Output = Option<Vec<NormalizedEntry>>>,
{
    let mut result = ScanResult::default();
    for (position, turn) in turns.iter().enumerate() {
        if budget_spent() {
            result.unscanned = turns[position..]
                .iter()
                .map(|turn| turn.execution_id)
                .collect();
            break;
        }
        let Some(entries) = load(turn.execution_id).await else {
            result.unreadable.push(turn.execution_id);
            continue;
        };
        result.matches.extend(filter.project_turn(turn, &entries));
        result.turns_scanned += 1;
    }
    result
}

pub async fn get_session_history(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<SessionHistoryQuery>,
) -> Result<ResponseJson<ApiResponse<SessionHistoryResponse>>, ApiError> {
    let filter = HistoryFilter::from_query(&query).map_err(ApiError::BadRequest)?;
    let pool = &deployment.db().pool;
    let turns: Vec<Turn> = ExecutionProcess::find_by_session_id(pool, session.id, false)
        .await?
        .into_iter()
        .filter(|process| process.run_reason == ExecutionProcessRunReason::CodingAgent)
        .enumerate()
        .map(|(turn_index, process)| Turn {
            execution_id: process.id,
            turn_index,
            started_at: process.started_at,
            completed_at: process.completed_at,
        })
        .collect();
    let turn_count = turns.len();

    let mut candidates: Vec<Turn> = match query.execution_id {
        Some(execution_id) => {
            let turn = turns
                .into_iter()
                .find(|turn| turn.execution_id == execution_id)
                .ok_or_else(|| {
                    ApiError::BadRequest(format!(
                        "execution {execution_id} is not a coding-agent turn of session {}",
                        session.id
                    ))
                })?;
            vec![turn]
        }
        None => turns,
    };
    let now = Utc::now();
    candidates.retain(|turn| filter.turn_overlaps(turn, now));

    let deadline = Instant::now() + HISTORY_BUDGET;
    let container = deployment.container();
    let scan = scan_turns(
        &candidates,
        &filter,
        || Instant::now() >= deadline,
        |execution_id| async move { container.normalized_entries(&execution_id).await },
    )
    .await;

    let offset = query.offset.unwrap_or(0);
    let limit = clamp_history_limit(query.limit);
    let total_matches = scan.matches.len();
    let (messages, has_more) = paginate(scan.matches, query.order, offset, limit);

    Ok(ResponseJson(ApiResponse::success(SessionHistoryResponse {
        session_id: session.id.to_string(),
        turn_count,
        turns_scanned: scan.turns_scanned,
        total_matches,
        offset,
        limit,
        order: query.order,
        has_more,
        complete: scan.unscanned.is_empty() && scan.unreadable.is_empty(),
        unscanned_execution_ids: scan.unscanned.iter().map(Uuid::to_string).collect(),
        unreadable_execution_ids: scan.unreadable.iter().map(Uuid::to_string).collect(),
        messages,
    })))
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use chrono::TimeZone;
    use executors::logs::NormalizedEntryType;

    use super::*;

    fn at(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 11, hour, 0, 0).unwrap()
    }

    fn turn(turn_index: usize, started: u32, completed: Option<u32>) -> Turn {
        Turn {
            execution_id: Uuid::new_v4(),
            turn_index,
            started_at: at(started),
            completed_at: completed.map(at),
        }
    }

    fn entry(entry_type: NormalizedEntryType, content: &str) -> NormalizedEntry {
        NormalizedEntry {
            timestamp: None,
            entry_type,
            content: content.to_string(),
            metadata: None,
        }
    }

    fn query() -> SessionHistoryQuery {
        SessionHistoryQuery::default()
    }

    #[test]
    fn bounds_must_parse_and_be_ordered() {
        let filter = HistoryFilter::from_query(&SessionHistoryQuery {
            since: Some("2026-10-11T09:00:00+02:00".to_string()),
            until: Some(" ".to_string()),
            ..query()
        })
        .unwrap();
        assert_eq!(filter.since, Some(at(7)));
        assert_eq!(filter.until, None);

        assert!(matches!(
            HistoryFilter::from_query(&SessionHistoryQuery {
                since: Some("yesterday".to_string()),
                ..query()
            }),
            Err(message) if message.contains("since")
        ));
        assert!(
            HistoryFilter::from_query(&SessionHistoryQuery {
                since: Some(at(10).to_rfc3339()),
                until: Some(at(9).to_rfc3339()),
                ..query()
            })
            .is_err()
        );
    }

    #[test]
    fn blank_query_is_no_filter() {
        let filter = HistoryFilter::from_query(&SessionHistoryQuery {
            query: Some("   ".to_string()),
            ..query()
        })
        .unwrap();
        assert!(filter.needle.is_none());
    }

    #[test]
    fn turn_overlap_uses_completion_or_now() {
        let filter = HistoryFilter {
            since: Some(at(10)),
            until: Some(at(12)),
            ..Default::default()
        };
        assert!(!filter.turn_overlaps(&turn(0, 8, Some(9)), at(23)));
        assert!(filter.turn_overlaps(&turn(0, 8, Some(10)), at(23)));
        assert!(filter.turn_overlaps(&turn(0, 11, Some(11)), at(23)));
        assert!(!filter.turn_overlaps(&turn(0, 13, Some(14)), at(23)));
        // A running turn extends to now.
        assert!(filter.turn_overlaps(&turn(0, 8, None), at(11)));
        assert!(!filter.turn_overlaps(&turn(0, 8, None), at(9)));
        assert!(HistoryFilter::default().turn_overlaps(&turn(0, 1, None), at(2)));
    }

    #[test]
    fn effective_time_falls_back_to_turn_start() {
        assert_eq!(effective_time(None, at(3)), at(3));
        assert_eq!(effective_time(Some("garbage"), at(3)), at(3));
        assert_eq!(effective_time(Some("2026-10-11T05:00:00Z"), at(3)), at(5));
    }

    #[test]
    fn project_turn_filters_keyword_beyond_truncation_roles_and_window() {
        let current = turn(4, 10, Some(11));
        let long_tool_output = format!("{}DeployFailed", "x".repeat(5000));
        let mut timed = entry(NormalizedEntryType::AssistantMessage, "deployfailed later");
        timed.timestamp = Some(at(15).to_rfc3339());
        let entries = vec![
            entry(
                NormalizedEntryType::UserMessage,
                "why did deployFAILED happen?",
            ),
            entry(
                NormalizedEntryType::ToolUse {
                    tool_name: "bash".to_string(),
                    action_type: executors::logs::ActionType::Other {
                        description: "run".to_string(),
                    },
                    status: executors::logs::ToolStatus::Success,
                },
                &long_tool_output,
            ),
            entry(NormalizedEntryType::AssistantMessage, "unrelated"),
            timed,
        ];

        let filter = HistoryFilter {
            needle: Some("deployfailed".to_string()),
            until: Some(at(12)),
            ..Default::default()
        };
        let messages = filter.project_turn(&current, &entries);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "tool");
        assert!(messages[1].text.ends_with("… [truncated]"));
        assert!(
            messages[1]
                .match_snippet
                .as_deref()
                .is_some_and(|snippet| snippet.ends_with("DeployFailed"))
        );
        assert!(messages.iter().all(|message| message.turn_index == 4));
        assert_eq!(messages[0].turn_started_at, at(10).to_rfc3339());

        let filter = HistoryFilter {
            roles: parse_roles(Some("assistant")),
            ..Default::default()
        };
        let messages = filter.project_turn(&current, &entries);
        assert_eq!(messages.len(), 2);
        assert!(
            messages
                .iter()
                .all(|message| message.match_snippet.is_none())
        );
        assert_eq!(messages[1].created_at, Some(at(15).to_rfc3339()));
    }

    #[test]
    fn paginate_walks_without_gaps_in_either_order() {
        let items: Vec<usize> = (0..7).collect();
        assert_eq!(
            paginate(items.clone(), HistoryOrder::Asc, 0, 3),
            (vec![0, 1, 2], true)
        );
        assert_eq!(
            paginate(items.clone(), HistoryOrder::Asc, 6, 3),
            (vec![6], false)
        );
        assert_eq!(
            paginate(items.clone(), HistoryOrder::Asc, 4, 3),
            (vec![4, 5, 6], false)
        );
        assert_eq!(
            paginate(items.clone(), HistoryOrder::Desc, 0, 3),
            (vec![6, 5, 4], true)
        );
        assert_eq!(
            paginate(items, HistoryOrder::Asc, 99, 3),
            (Vec::<usize>::new(), false)
        );
    }

    #[test]
    fn limit_is_clamped() {
        assert_eq!(clamp_history_limit(None), DEFAULT_HISTORY_LIMIT);
        assert_eq!(clamp_history_limit(Some(0)), 1);
        assert_eq!(clamp_history_limit(Some(10_000)), MAX_HISTORY_LIMIT);
    }

    #[test]
    fn order_parses_lowercase() {
        let parsed: SessionHistoryQuery =
            serde_json::from_value(serde_json::json!({ "order": "desc" })).unwrap();
        assert_eq!(parsed.order, HistoryOrder::Desc);
        assert_eq!(query().order, HistoryOrder::Asc);
    }

    #[tokio::test]
    async fn scan_reads_every_turn_in_order() {
        let turns = vec![turn(0, 1, Some(2)), turn(1, 3, Some(4)), turn(2, 5, None)];
        let loaded = RefCell::new(Vec::new());
        let result = scan_turns(
            &turns,
            &HistoryFilter::default(),
            || false,
            |id| {
                loaded.borrow_mut().push(id);
                async move {
                    Some(vec![entry(
                        NormalizedEntryType::AssistantMessage,
                        &id.to_string(),
                    )])
                }
            },
        )
        .await;

        assert_eq!(result.turns_scanned, 3);
        assert!(result.unscanned.is_empty());
        let order: Vec<usize> = result.matches.iter().map(|m| m.turn_index).collect();
        assert_eq!(order, vec![0, 1, 2]);
        assert_eq!(
            *loaded.borrow(),
            turns.iter().map(|t| t.execution_id).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn scan_stops_loading_once_the_budget_is_spent() {
        let turns = vec![turn(0, 1, Some(2)), turn(1, 3, Some(4)), turn(2, 5, None)];
        let loads = Cell::new(0);
        let result = scan_turns(
            &turns,
            &HistoryFilter::default(),
            || loads.get() >= 1,
            |_| {
                loads.set(loads.get() + 1);
                async { Some(vec![entry(NormalizedEntryType::AssistantMessage, "hi")]) }
            },
        )
        .await;

        assert_eq!(
            loads.get(),
            1,
            "no turn is loaded after the budget is spent"
        );
        assert_eq!(result.turns_scanned, 1);
        assert_eq!(
            result.unscanned,
            vec![turns[1].execution_id, turns[2].execution_id]
        );
        assert_eq!(result.matches.len(), 1);
    }

    #[tokio::test]
    async fn an_unreadable_turn_is_reported_not_counted_as_searched() {
        let turns = vec![turn(0, 1, Some(2)), turn(1, 3, Some(4))];
        let unreadable = turns[0].execution_id;
        let result = scan_turns(
            &turns,
            &HistoryFilter::default(),
            || false,
            |id| async move {
                (id != unreadable).then(|| vec![entry(NormalizedEntryType::AssistantMessage, "hi")])
            },
        )
        .await;

        assert_eq!(result.unreadable, vec![unreadable]);
        assert!(result.unscanned.is_empty());
        assert_eq!(result.turns_scanned, 1);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].turn_index, 1);
    }
}
