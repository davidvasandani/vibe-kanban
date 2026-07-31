use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use cluster_protocol::{DisconnectPolicy, ExecutionEventPayload, InteractionRequest};
use executors::approvals::{ExecutorApprovalError, ExecutorApprovalService};
use tokio::sync::{Mutex, oneshot};
use tokio_util::sync::CancellationToken;
use utils::approvals::{ApprovalOutcome, ApprovalStatus, QuestionStatus};
use uuid::Uuid;

use crate::execution::WorkerJob;

#[derive(Default)]
pub struct InteractionBroker {
    pending: Mutex<HashMap<Uuid, oneshot::Sender<ApprovalOutcome>>>,
    completed: Mutex<HashMap<Uuid, ApprovalOutcome>>,
}

impl InteractionBroker {
    async fn register(&self, interaction_id: Uuid) -> oneshot::Receiver<ApprovalOutcome> {
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(interaction_id, sender);
        receiver
    }

    pub async fn respond(&self, interaction_id: Uuid, outcome: ApprovalOutcome) -> bool {
        if self.completed.lock().await.contains_key(&interaction_id) {
            return true;
        }
        let Some(sender) = self.pending.lock().await.remove(&interaction_id) else {
            return false;
        };
        self.completed
            .lock()
            .await
            .insert(interaction_id, outcome.clone());
        let _ = sender.send(outcome);
        true
    }
}

pub struct WorkerApprovalService {
    job: Arc<WorkerJob>,
    broker: Arc<InteractionBroker>,
    receivers: Mutex<HashMap<Uuid, oneshot::Receiver<ApprovalOutcome>>>,
}

impl WorkerApprovalService {
    pub fn new(job: Arc<WorkerJob>, broker: Arc<InteractionBroker>) -> Arc<Self> {
        Arc::new(Self {
            job,
            broker,
            receivers: Mutex::new(HashMap::new()),
        })
    }

    async fn create(
        &self,
        kind: &str,
        prompt: String,
        policy: DisconnectPolicy,
    ) -> Result<String, ExecutorApprovalError> {
        let interaction_id = Uuid::new_v4();
        let receiver = self.broker.register(interaction_id).await;
        self.receivers.lock().await.insert(interaction_id, receiver);
        self.job
            .emit(ExecutionEventPayload::InteractionRequested(
                InteractionRequest {
                    interaction_id,
                    kind: kind.into(),
                    prompt,
                    expires_at: Some(Utc::now() + Duration::hours(10)),
                    disconnect_policy: policy,
                },
            ))
            .await;
        Ok(interaction_id.to_string())
    }

    async fn wait(
        &self,
        approval_id: &str,
        cancel: CancellationToken,
    ) -> Result<ApprovalOutcome, ExecutorApprovalError> {
        let interaction_id = approval_id
            .parse::<Uuid>()
            .map_err(ExecutorApprovalError::request_failed)?;
        let receiver = self
            .receivers
            .lock()
            .await
            .remove(&interaction_id)
            .ok_or(ExecutorApprovalError::SessionNotRegistered)?;
        tokio::select! {
            _ = cancel.cancelled() => Err(ExecutorApprovalError::Cancelled),
            outcome = receiver => outcome.map_err(|_| ExecutorApprovalError::ServiceUnavailable),
        }
    }
}

#[async_trait]
impl ExecutorApprovalService for WorkerApprovalService {
    async fn create_tool_approval(&self, tool_name: &str) -> Result<String, ExecutorApprovalError> {
        self.create("tool", tool_name.into(), DisconnectPolicy::FailClosed)
            .await
    }

    async fn create_question_approval(
        &self,
        tool_name: &str,
        question_count: usize,
    ) -> Result<String, ExecutorApprovalError> {
        self.create(
            "question",
            format!("{tool_name}:{question_count}"),
            DisconnectPolicy::Timeout,
        )
        .await
    }

    async fn wait_tool_approval(
        &self,
        approval_id: &str,
        cancel: CancellationToken,
    ) -> Result<ApprovalStatus, ExecutorApprovalError> {
        match self.wait(approval_id, cancel).await? {
            ApprovalOutcome::Approved => Ok(ApprovalStatus::Approved),
            ApprovalOutcome::Denied { reason } => Ok(ApprovalStatus::Denied { reason }),
            ApprovalOutcome::TimedOut => Ok(ApprovalStatus::TimedOut),
            ApprovalOutcome::Answered { .. } => Err(ExecutorApprovalError::request_failed(
                "question answer returned for tool approval",
            )),
        }
    }

    async fn wait_question_answer(
        &self,
        approval_id: &str,
        cancel: CancellationToken,
    ) -> Result<QuestionStatus, ExecutorApprovalError> {
        match self.wait(approval_id, cancel).await? {
            ApprovalOutcome::Answered { answers } => Ok(QuestionStatus::Answered { answers }),
            ApprovalOutcome::TimedOut => Ok(QuestionStatus::TimedOut),
            _ => Err(ExecutorApprovalError::request_failed(
                "tool decision returned for question",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn responses_are_correlated_and_idempotent() {
        let broker = InteractionBroker::default();
        let interaction_id = Uuid::new_v4();
        let receiver = broker.register(interaction_id).await;
        let outcome = ApprovalOutcome::Denied {
            reason: Some("not allowed".into()),
        };

        assert!(broker.respond(interaction_id, outcome.clone()).await);
        assert!(matches!(
            receiver.await.unwrap(),
            ApprovalOutcome::Denied { reason: Some(reason) } if reason == "not allowed"
        ));
        assert!(
            broker
                .respond(interaction_id, ApprovalOutcome::Approved)
                .await
        );
        assert!(
            !broker
                .respond(Uuid::new_v4(), ApprovalOutcome::Approved)
                .await
        );
    }
}
