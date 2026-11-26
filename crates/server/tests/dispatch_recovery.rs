#![cfg(unix)]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Path as RoutePath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use cluster_protocol::{
    DispatchAccepted, EventBatch, ExecutionDispatch, ExecutionEvent, ExecutionEventPayload,
    JobState, JobSummary, TerminalEvidence, TerminalState,
};
use db::models::{
    execution_process::{
        CreateExecutionProcess, ExecutionProcess, ExecutionProcessRunReason as Reason,
        ExecutionProcessStatus as Status,
    },
    execution_worker_job::ExecutionWorkerJob,
    workspace::Workspace,
};
use deployment::Deployment;
use executors::{
    actions::{ExecutorAction, coding_agent_initial::CodingAgentInitialRequest},
    executors::BaseCodingAgent,
    profile::ExecutorConfig,
};
use serde_json::json;
use services::services::{config::Config, container::ContainerService};
use sqlx::SqlitePool;
use tokio::{
    process::{Child, Command},
    time::{sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const CASES: [&str; 9] = [
    "survivor",
    "completed",
    "safe",
    "uncertain",
    "transient",
    "exhausted",
    "stopped",
    "paused",
    "notice-fail",
];
const NOTICE: &str = "npm notice New major version of npm available! 10.9.2 -> 11.6.0";

fn execution_id(case: &str) -> Uuid {
    Uuid::from_u128(CASES.iter().position(|value| *value == case).unwrap() as u128 + 1)
}

fn worker_id() -> Uuid {
    Uuid::from_u128(100)
}

struct Job {
    summary: JobSummary,
    events: Vec<ExecutionEvent>,
}

#[derive(Default)]
struct Recorded {
    dispatches: HashMap<Uuid, Vec<ExecutionDispatch>>,
    probes: HashMap<Uuid, usize>,
    jobs: HashMap<Uuid, Job>,
    first_event_deliveries: HashMap<Uuid, usize>,
}

struct Worker {
    root: PathBuf,
    recorded: Mutex<Recorded>,
    boot: AtomicUsize,
    second_boot_inventory: AtomicUsize,
}

fn refusal() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"code": "admission_refused", "error": "fixture worker draining"})),
    )
        .into_response()
}

impl Worker {
    fn attempts(&self, case: &str) -> (usize, usize) {
        let recorded = self.recorded.lock().unwrap();
        let id = execution_id(case);
        (
            recorded.dispatches.get(&id).map_or(0, Vec::len),
            recorded.probes.get(&id).copied().unwrap_or(0),
        )
    }

    fn effects(&self, case: &str) -> usize {
        std::fs::read_to_string(self.root.join(format!("effect-{case}")))
            .unwrap_or_default()
            .lines()
            .count()
    }

    async fn run_effect(&self, case: &str) -> std::process::Output {
        Command::new("sh")
            .args([
                "-c",
                "printf 'effect\n' >> \"$1\"; printf '%s\n' \"$2\" >&2; exit \"$3\"",
                "fixture",
            ])
            .arg(self.root.join(format!("effect-{case}")))
            .arg(if matches!(case, "notice-fail" | "completed") {
                NOTICE
            } else {
                ""
            })
            .arg(if case == "notice-fail" { "17" } else { "0" })
            .output()
            .await
            .unwrap()
    }

    async fn finish(&self, case: &str) {
        let output = self.run_effect(case).await;
        let now = Utc::now();
        let (state, terminal_state) = if output.status.success() {
            (JobState::Completed, TerminalState::Completed)
        } else {
            (JobState::Failed, TerminalState::Failed)
        };
        let terminal = TerminalEvidence {
            state: terminal_state,
            exit_code: output.status.code(),
            signal: None,
            observed_at: now,
        };
        let id = execution_id(case);
        let mut recorded = self.recorded.lock().unwrap();
        let job = recorded.jobs.get_mut(&id).unwrap();
        job.events.extend([
            ExecutionEvent {
                execution_id: id,
                sequence: 2,
                worker_timestamp: now,
                payload: ExecutionEventPayload::Stderr {
                    data_base64: STANDARD.encode(output.stderr),
                },
            },
            ExecutionEvent {
                execution_id: id,
                sequence: 3,
                worker_timestamp: now,
                payload: if state == JobState::Completed {
                    ExecutionEventPayload::Completed(terminal.clone())
                } else {
                    ExecutionEventPayload::Failed(terminal.clone())
                },
            },
        ]);
        job.summary.state = state;
        job.summary.terminal = Some(terminal);
        job.summary.last_sequence = 3;
    }
}

async fn dispatch(
    State(worker): State<Arc<Worker>>,
    Json(request): Json<ExecutionDispatch>,
) -> Response {
    let id = request.execution_id;
    let case = CASES
        .iter()
        .copied()
        .find(|case| execution_id(case) == id)
        .unwrap();
    let attempt = {
        let mut recorded = worker.recorded.lock().unwrap();
        let dispatches = recorded.dispatches.entry(id).or_default();
        dispatches.push(request.clone());
        dispatches.len()
    };
    if attempt == 1
        && matches!(
            case,
            "safe" | "transient" | "exhausted" | "stopped" | "paused"
        )
    {
        return refusal();
    }
    if case == "uncertain" {
        worker.run_effect(case).await;
        return std::future::pending().await;
    }
    let worker_job_id = Uuid::new_v4();
    worker.recorded.lock().unwrap().jobs.insert(
        id,
        Job {
            summary: JobSummary {
                execution_id: id,
                worker_job_id,
                workspace_id: request.workspace_id,
                request_digest: request.request_digest.clone(),
                state: JobState::Running,
                last_sequence: 1,
                terminal: None,
            },
            events: vec![ExecutionEvent {
                execution_id: id,
                sequence: 1,
                worker_timestamp: Utc::now(),
                payload: ExecutionEventPayload::Accepted,
            }],
        },
    );
    if !matches!(case, "survivor" | "completed") {
        worker.finish(case).await;
    }
    Json(DispatchAccepted {
        execution_id: id,
        worker_job_id,
        request_digest: request.request_digest,
        state: JobState::Running,
        last_sequence: 1,
    })
    .into_response()
}

async fn readiness(State(worker): State<Arc<Worker>>, RoutePath(id): RoutePath<Uuid>) -> Response {
    let attempt = {
        let mut recorded = worker.recorded.lock().unwrap();
        let attempts = recorded.probes.entry(id).or_default();
        *attempts += 1;
        *attempts
    };
    std::fs::write(worker.root.join(format!("probe-{id}")), "").unwrap();
    if (id == execution_id("safe") && attempt == 1)
        || id == execution_id("stopped")
        || id == execution_id("paused")
    {
        return std::future::pending().await;
    }
    if id == execution_id("exhausted") || (id == execution_id("transient") && attempt == 1) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(
                json!({"code": "readiness_transient", "error": "fixture workspace read timed out"}),
            ),
        )
            .into_response();
    }
    Json(()).into_response()
}

async fn inventory(State(worker): State<Arc<Worker>>) -> Json<Vec<JobSummary>> {
    if worker.boot.load(SeqCst) == 2 && worker.second_boot_inventory.fetch_add(1, SeqCst) == 3 {
        return std::future::pending().await;
    }
    Json(
        worker
            .recorded
            .lock()
            .unwrap()
            .jobs
            .values()
            .map(|job| job.summary.clone())
            .collect(),
    )
}

async fn events(
    State(worker): State<Arc<Worker>>,
    RoutePath(id): RoutePath<Uuid>,
    Query(query): Query<HashMap<String, u64>>,
) -> Json<EventBatch> {
    let after = query.get("after").copied().unwrap_or_default();
    let mut recorded = worker.recorded.lock().unwrap();
    if after == 0 {
        *recorded.first_event_deliveries.entry(id).or_default() += 1;
    }
    let job = &recorded.jobs[&id];
    Json(EventBatch {
        execution_id: id,
        requested_after: after,
        earliest_available: 1,
        latest_available: job.summary.last_sequence,
        replay_gap: false,
        events: job
            .events
            .iter()
            .filter(|event| event.sequence > after)
            .cloned()
            .collect(),
    })
}

async fn accepted(pool: &SqlitePool, case: &str) -> bool {
    ExecutionWorkerJob::find_by_execution_id(pool, execution_id(case))
        .await
        .unwrap()
        .is_some_and(|job| job.accepted_at.is_some())
}

async fn status(pool: &SqlitePool, case: &str) -> Option<Status> {
    ExecutionProcess::find_by_id(pool, execution_id(case))
        .await
        .unwrap()
        .map(|process| process.status)
}

async fn wait_for<F: Future<Output = bool>>(mut condition: impl FnMut() -> F) {
    timeout(Duration::from_secs(25), async {
        while !condition().await {
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("fixture condition timed out");
}

#[tokio::test]
async fn recovers_across_two_isolated_coordinator_restarts() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    // Startup warms executor catalogs. Limit child PATH so those probes
    // cannot spawn the developer's installed agents or download packages.
    let fixture_bin = root.join("bin");
    std::fs::create_dir(&fixture_bin).unwrap();
    for program in ["sh", "git"] {
        let source = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|dir| dir.join(program))
            .find(|path| path.is_file())
            .unwrap();
        std::os::unix::fs::symlink(source, fixture_bin.join(program)).unwrap();
    }

    let worker = Arc::new(Worker {
        root: root.to_owned(),
        recorded: Mutex::default(),
        boot: AtomicUsize::new(1),
        second_boot_inventory: AtomicUsize::new(0),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"worker_node_id": worker_id()})) }),
        )
        .route("/v1/jobs", get(inventory))
        .route("/v1/executions/{id}", post(dispatch))
        .route("/v1/executions/{id}/readiness", post(readiness))
        .route("/v1/executions/{id}/events", get(events))
        .route(
            "/v1/executions/{id}/ack",
            post(|| async { Json(json!({})) }),
        )
        .with_state(worker.clone());
    let listener = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mut first = coordinator(root, &endpoint, true);
    wait_for(|| async root.join("seeded").exists()).await;
    let pool = SqlitePool::connect(&format!("sqlite://{}/data/db.v2.sqlite", root.display()))
        .await
        .unwrap();
    wait_for(|| async {
        worker.attempts("safe") == (1, 1)
            && worker.effects("uncertain") == 1
            && status(&pool, "stopped").await == Some(Status::Killed)
            && status(&pool, "paused").await == Some(Status::Interrupted)
            && status(&pool, "transient").await == Some(Status::Completed)
            && status(&pool, "exhausted").await == Some(Status::Failed)
            && status(&pool, "notice-fail").await == Some(Status::Failed)
            && accepted(&pool, "survivor").await
            && accepted(&pool, "completed").await
    })
    .await;
    first.kill().await.unwrap();
    // Simulate the crash window in which the worker served an event but
    // the coordinator's last durable cursor still precedes it.
    sqlx::query(
        "UPDATE execution_worker_jobs SET last_event_sequence = 0 WHERE execution_process_id = ?",
    )
    .bind(execution_id("survivor"))
    .execute(&pool)
    .await
    .unwrap();
    worker.finish("completed").await;
    worker.boot.store(2, SeqCst);
    let mut second = coordinator(root, &endpoint, false);
    wait_for(|| async worker.second_boot_inventory.load(SeqCst) >= 4).await;
    wait_for(|| async {
        worker
            .recorded
            .lock()
            .unwrap()
            .first_event_deliveries
            .get(&execution_id("survivor"))
            .copied()
            .unwrap_or_default()
            >= 2
    })
    .await;
    assert_eq!(worker.attempts("safe"), (1, 1));
    second.kill().await.unwrap();
    worker.boot.store(3, SeqCst);
    let mut third = coordinator(root, &endpoint, false);
    wait_for(|| async status(&pool, "safe").await == Some(Status::Completed)).await;
    worker.finish("survivor").await;
    wait_for(|| async {
        status(&pool, "survivor").await == Some(Status::Completed)
            && status(&pool, "completed").await == Some(Status::Completed)
            && status(&pool, "uncertain").await == Some(Status::Indeterminate)
    })
    .await;

    for (case, calls, probes, effects) in [
        ("survivor", 1, 0, 1),
        ("completed", 1, 0, 1),
        ("safe", 2, 2, 1),
        ("uncertain", 1, 0, 1),
        ("transient", 2, 2, 1),
        ("exhausted", 1, 2, 0),
        ("stopped", 1, 1, 0),
        ("paused", 1, 1, 0),
        ("notice-fail", 1, 0, 1),
    ] {
        let (dispatches, readiness_checks) = worker.attempts(case);
        let actual_effects = worker.effects(case);
        println!(
            "{case}: dispatches={dispatches}, readiness_checks={readiness_checks}, effects={actual_effects}"
        );
        assert_eq!(
            (dispatches, readiness_checks, actual_effects),
            (calls, probes, effects),
            "{case}"
        );
    }
    assert_eq!(status(&pool, "stopped").await, Some(Status::Killed));
    assert_eq!(status(&pool, "paused").await, Some(Status::Interrupted));
    let failed = ExecutionProcess::find_by_id(&pool, execution_id("notice-fail"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.exit_code, Some(17));
    {
        let recorded = worker.recorded.lock().unwrap();
        for case in ["safe", "transient"] {
            let requests = &recorded.dispatches[&execution_id(case)];
            assert_eq!(requests[0].execution_id, requests[1].execution_id);
            assert_eq!(requests[0].session_id, requests[1].session_id);
            assert_eq!(requests[0].workspace_id, requests[1].workspace_id);
            assert_eq!(requests[0].action, requests[1].action);
            assert_eq!(requests[0].request_digest, requests[1].request_digest);
            assert_ne!(requests[0].authority.nonce, requests[1].authority.nonce);
        }
    }
    for (case, message) in [
        ("safe", "Workspace readiness check 2 passed"),
        ("transient", "Workspace readiness check 1 failed"),
        ("exhausted", "submit the action manually"),
        (
            "uncertain",
            "Review worker and workspace effects before retrying",
        ),
        ("completed", NOTICE),
        ("notice-fail", NOTICE),
    ] {
        wait_for(|| async history(&pool, root, case).await.contains(message)).await;
    }
    third.kill().await.unwrap();
    listener.abort();
}

async fn history(pool: &SqlitePool, root: &Path, case: &str) -> String {
    let id = execution_id(case);
    let process = ExecutionProcess::find_by_id(pool, id)
        .await
        .unwrap()
        .unwrap();
    let path = utils::execution_logs::process_log_file_path_in_root(
        &root.join("data"),
        process.session_id,
        id,
    );
    std::fs::read_to_string(path).unwrap_or_default()
}

fn coordinator(root: &Path, endpoint: &str, seed: bool) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "coordinator_process", "--nocapture"])
        .env_clear()
        .env("PATH", root.join("bin"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("XDG_DATA_HOME", root.join("xdg-data"))
        .env("XDG_STATE_HOME", root.join("xdg-state"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", root.join("gitconfig"))
        .env("XDG_CONFIG_HOME", root.join("xdg"))
        .env("VK_RECOVERY_FIXTURE", root)
        .env("VK_RECOVERY_SEED", if seed { "1" } else { "0" })
        .env("VK_DATA_DIR", root.join("data"))
        .env("VK_CLUSTER_ENABLED", "true")
        .env(
            "VK_CLUSTER_COORDINATOR_ID",
            Uuid::from_u128(101).to_string(),
        )
        .env("VK_CLUSTER_SHARED_ROOT", root)
        .env("VK_CLUSTER_WORKER_ENDPOINTS", endpoint)
        .env("VK_SHARED_API_BASE", "http://127.0.0.1:1")
        .env("VK_SHARED_RELAY_API_BASE", "http://127.0.0.1:1")
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

#[tokio::test]
#[ignore = "spawned with isolated data and loopback worker by the restart test"]
async fn coordinator_process() {
    let root = PathBuf::from(std::env::var_os("VK_RECOVERY_FIXTURE").unwrap());
    rustls::crypto::aws_lc_rs::default_provider().install_default().unwrap();
    let seed = std::env::var("VK_RECOVERY_SEED").unwrap() == "1";
    if seed {
        let config = Config {
            onboarding_acknowledged: true,
            analytics_enabled: false,
            resume_interrupted_on_startup: true,
            workspace_dir: Some(root.join("worktrees").to_string_lossy().into_owned()),
            notifications: services::services::config::NotificationConfig {
                push_enabled: false,
                sound_enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        std::fs::write(
            utils::assets::config_path(),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
    }
    let server =
        server::startup::start_with_bind("127.0.0.1:0", "127.0.0.1:0", CancellationToken::new())
            .await
            .unwrap();
    let base_url = format!("http://127.0.0.1:{}", server.port);
    let deployment = server.deployment.clone();
    let serving = tokio::spawn(server.serve());
    if seed {
        let db = &deployment.db().pool;
        sqlx::query("INSERT INTO worker_nodes (id, hostname, status, worker_version, vibe_version, mount_status, lease_expires_at)
                     VALUES (?, 'fixture', 'online', 'fixture', 'fixture', 'healthy', datetime('now', '+15 minutes'))")
            .bind(worker_id()).execute(db).await.unwrap();
        for case in CASES {
            let workspace_id = Uuid::new_v4();
            let session_id = Uuid::new_v4();
            let workspace_dir = root.join(case);
            std::fs::create_dir(&workspace_dir).unwrap();
            sqlx::query("INSERT INTO workspaces (id, branch, container_ref, worker_node_id, placement_state)
                         VALUES (?, 'fixture', ?, ?, 'ready')")
                .bind(workspace_id).bind(workspace_dir.to_str().unwrap()).bind(worker_id()).execute(db).await.unwrap();
            sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES (?, ?)")
                .bind(session_id)
                .bind(workspace_id)
                .execute(db)
                .await
                .unwrap();
            let workspace = Workspace::find_by_id(db, workspace_id)
                .await
                .unwrap()
                .unwrap();
            let action = ExecutorAction::new(
                CodingAgentInitialRequest {
                    prompt: case.into(),
                    executor_config: ExecutorConfig::new(BaseCodingAgent::Opencode),
                    working_dir: None,
                }
                .into(),
                None,
            );
            let process = ExecutionProcess::create(
                db,
                &CreateExecutionProcess {
                    session_id,
                    executor_action: action.clone(),
                    run_reason: Reason::CodingAgent,
                },
                execution_id(case),
                &[],
            )
            .await
            .unwrap();
            let dispatch_deployment = deployment.clone();
            let execution = process.clone();
            tokio::spawn(async move {
                if dispatch_deployment
                    .container()
                    .dispatch_execution(&workspace, &execution, &action)
                    .await
                    .is_ok()
                {
                    dispatch_deployment
                        .container()
                        .start_execution_log_pipeline(&workspace, &execution, &action, true)
                        .await;
                }
            });
            if matches!(case, "stopped" | "paused") {
                let deployment = deployment.clone();
                let root = root.clone();
                let base_url = base_url.clone();
                tokio::spawn(async move {
                    wait_for(|| async {
                        ExecutionWorkerJob::recovery(&deployment.db().pool, process.id)
                            .await
                            .is_ok_and(|recovery| recovery.readiness_attempts == 1)
                            && root.join(format!("probe-{}", process.id)).exists()
                    })
                    .await;
                    let control = if case == "stopped" {
                        "stop"
                    } else {
                        "pause-recovery"
                    };
                    reqwest::Client::new()
                        .post(format!(
                            "{base_url}/api/execution-processes/{}/{control}",
                            process.id
                        ))
                        .send()
                        .await
                        .unwrap()
                        .error_for_status()
                        .unwrap();
                });
            }
        }
        std::fs::write(root.join("seeded"), "").unwrap();
    }
    serving.await.unwrap().unwrap();
}
