use std::sync::Arc;

use chrono::Utc;
use thiserror::Error;

use uuid::Uuid;

use crate::auth::{AuthAction, AuthResult, AuthorizationProvider};
use crate::contract::context_bundle::*;
use crate::contract::memory_event::*;
use crate::projection::repository::{ProjectionError, ProjectionRepository};
use crate::projection::types::*;

#[derive(Debug, Clone, serde::Serialize)]
pub struct WriteEventResponse {
    pub event_id: Uuid,
    pub event_type: String,
    pub projected: bool,
}

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("projection unavailable")]
    Unavailable,
    #[error("invalid event: {0}")]
    InvalidEvent(String),
}

impl From<ProjectionError> for ContextError {
    fn from(e: ProjectionError) -> Self {
        match e {
            ProjectionError::Unavailable => ContextError::Unavailable,
            ProjectionError::NotFound => ContextError::Unavailable,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectionStatus {
    pub checkpoints: Vec<CheckpointInfo>,
    pub is_healthy: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckpointInfo {
    pub entry_type: String,
    pub last_position: i64,
    pub updated_at: Option<chrono::DateTime<Utc>>,
}

pub struct ContextService<P: ProjectionRepository, A: AuthorizationProvider> {
    projection: Arc<P>,
    auth: Arc<A>,
}

impl<P: ProjectionRepository, A: AuthorizationProvider> ContextService<P, A> {
    pub fn new(projection: Arc<P>, auth: Arc<A>) -> Self {
        Self { projection, auth }
    }

    pub async fn get_context(
        &self,
        agent_id: &str,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<ContextBundleV1, ContextError> {
        self.check_auth(agent_id, tenant_id, AuthAction::ReadContext)
            .await?;

        let decisions = self
            .projection
            .get_current_decisions(tenant_id, subject_id)
            .await?;
        let records = self
            .projection
            .get_memory_records(tenant_id, subject_id)
            .await?;
        let dissent_records = self
            .projection
            .get_dissent_for_subject(tenant_id, subject_id)
            .await?;

        let mut all_evidence = Vec::new();

        let effective_decisions: Vec<DecisionView> = decisions
            .into_iter()
            .map(|d| {
                let has_dissent = dissent_records
                    .iter()
                    .any(|ds| ds.target_event_id == d.decision_id);
                DecisionView {
                    decision_id: d.decision_id,
                    actor: Actor {
                        actor_id: d.actor_id,
                        actor_type: d.actor_type,
                    },
                    content: d.decision_content,
                    effective_at: d.effective_at,
                    recorded_at: d.recorded_at,
                    is_current: d.is_current,
                    correlation_id: d.correlation_id,
                    has_dissent,
                    ledger_entry_id: d.ledger_entry_id,
                }
            })
            .collect();

        for d in &effective_decisions {
            if let Ok(links) = self
                .projection
                .get_evidence_links("current_decisions", d.decision_id)
                .await
            {
                all_evidence.extend(links.into_iter().map(to_evidence_view));
            }
        }

        let observations: Vec<MemoryRecordView> = records
            .iter()
            .filter(|r| r.event_type == OBSERVATION_RECORDED)
            .map(to_memory_record_view)
            .collect();

        let assertions: Vec<MemoryRecordView> = records
            .iter()
            .filter(|r| r.event_type == ASSERTION_RECORDED)
            .map(to_memory_record_view)
            .collect();

        for r in records.iter() {
            if let Ok(links) = self
                .projection
                .get_evidence_links("memory_records", r.record_id)
                .await
            {
                all_evidence.extend(links.into_iter().map(to_evidence_view));
            }
        }

        let dissent: Vec<DissentView> = dissent_records
            .iter()
            .map(|d| DissentView {
                dissent_id: d.dissent_id,
                target_event_id: d.target_event_id,
                actor: Actor {
                    actor_id: d.actor_id.clone(),
                    actor_type: d.actor_type.clone(),
                },
                content: d.dissent_content.clone(),
                recorded_at: d.recorded_at,
                ledger_entry_id: d.ledger_entry_id,
            })
            .collect();

        for d in &dissent {
            if let Ok(links) = self
                .projection
                .get_evidence_links("dissent_records", d.dissent_id)
                .await
            {
                all_evidence.extend(links.into_iter().map(to_evidence_view));
            }
        }

        Ok(ContextBundleV1 {
            schema_version: CONTEXT_BUNDLE_SCHEMA_VERSION.to_string(),
            tenant_id: tenant_id.to_string(),
            subject_id: subject_id.to_string(),
            generated_at: Utc::now(),
            effective_decisions,
            observations,
            assertions,
            dissent,
            evidence: all_evidence,
            projection_freshness: ProjectionFreshness {
                last_processed_at: None,
                lag_entries: std::collections::HashMap::new(),
                is_caught_up: true,
            },
        })
    }

    pub async fn write_event(
        &self,
        agent_id: &str,
        event: MemoryEventV1,
    ) -> Result<WriteEventResponse, ContextError> {
        self.check_auth(agent_id, &event.tenant_id, AuthAction::ReadContext)
            .await?;

        event
            .validate()
            .map_err(|e| ContextError::InvalidEvent(e.to_string()))?;

        let synthetic_ledger_id = Uuid::new_v4();
        let event_type = event.event_type.clone();

        match event_type.as_str() {
            OBSERVATION_RECORDED | ASSERTION_RECORDED | ACTION_AUTHORIZED | ACTION_COMPLETED
            | OUTCOME_RECORDED => {
                let record = MemoryRecordRow {
                    record_id: event.event_id,
                    tenant_id: event.tenant_id.clone(),
                    subject_id: event.subject_id.clone(),
                    event_type: event.event_type.clone(),
                    actor_id: event.actor.actor_id.clone(),
                    actor_type: event.actor.actor_type.clone(),
                    record_content: event.payload.clone(),
                    effective_at: event.effective_at,
                    recorded_at: event.recorded_at,
                    claim_state: event.claim_state.clone(),
                    has_dissent: false,
                    correlation_id: event.correlation_id.clone(),
                    classification: event.classification.clone(),
                    ledger_entry_id: synthetic_ledger_id,
                };
                self.projection.upsert_memory_record(record).await?;
            }
            DISSENT_RECORDED => {
                let dissent = DissentRecordRow {
                    dissent_id: event.event_id,
                    tenant_id: event.tenant_id.clone(),
                    subject_id: event.subject_id.clone(),
                    target_event_id: event.supersedes_event_id.unwrap_or(Uuid::nil()),
                    actor_id: event.actor.actor_id.clone(),
                    actor_type: event.actor.actor_type.clone(),
                    dissent_content: event.payload.clone(),
                    recorded_at: event.recorded_at,
                    ledger_entry_id: synthetic_ledger_id,
                    correlation_id: event.correlation_id.clone(),
                };
                self.projection.insert_dissent(dissent).await?;
                if let Some(target_id) = event.supersedes_event_id {
                    self.projection.mark_record_dissented(target_id).await?;
                }
            }
            DECISION_RECORDED => {
                let decision = CurrentDecisionRow {
                    decision_id: event.event_id,
                    tenant_id: event.tenant_id.clone(),
                    subject_id: event.subject_id.clone(),
                    actor_id: event.actor.actor_id.clone(),
                    actor_type: event.actor.actor_type.clone(),
                    decision_content: event.payload.clone(),
                    effective_at: event.effective_at.unwrap_or(event.recorded_at),
                    recorded_at: event.recorded_at,
                    superseded_by: None,
                    is_current: true,
                    ledger_entry_id: synthetic_ledger_id,
                    correlation_id: event.correlation_id.clone(),
                    classification: event.classification.clone(),
                };
                self.projection.upsert_decision(decision).await?;
                let history = DecisionHistoryRow {
                    history_id: Uuid::new_v4(),
                    decision_id: event.event_id,
                    tenant_id: event.tenant_id.clone(),
                    subject_id: event.subject_id.clone(),
                    actor_id: event.actor.actor_id.clone(),
                    actor_type: event.actor.actor_type.clone(),
                    decision_content: event.payload.clone(),
                    effective_at: event.effective_at.unwrap_or(event.recorded_at),
                    recorded_at: event.recorded_at,
                    supersedes_event_id: None,
                    claim_state: "decided".to_string(),
                    ledger_entry_id: synthetic_ledger_id,
                    correlation_id: event.correlation_id.clone(),
                };
                self.projection.insert_decision_history(history).await?;
            }
            DECISION_SUPERSEDED => {
                if let Some(old_id) = event.supersedes_event_id {
                    self.projection
                        .mark_decision_superseded(old_id, event.event_id)
                        .await?;
                }
                let decision = CurrentDecisionRow {
                    decision_id: event.event_id,
                    tenant_id: event.tenant_id.clone(),
                    subject_id: event.subject_id.clone(),
                    actor_id: event.actor.actor_id.clone(),
                    actor_type: event.actor.actor_type.clone(),
                    decision_content: event.payload.clone(),
                    effective_at: event.effective_at.unwrap_or(event.recorded_at),
                    recorded_at: event.recorded_at,
                    superseded_by: None,
                    is_current: true,
                    ledger_entry_id: synthetic_ledger_id,
                    correlation_id: event.correlation_id.clone(),
                    classification: event.classification.clone(),
                };
                self.projection.upsert_decision(decision).await?;
                let history = DecisionHistoryRow {
                    history_id: Uuid::new_v4(),
                    decision_id: event.event_id,
                    tenant_id: event.tenant_id.clone(),
                    subject_id: event.subject_id.clone(),
                    actor_id: event.actor.actor_id.clone(),
                    actor_type: event.actor.actor_type.clone(),
                    decision_content: event.payload.clone(),
                    effective_at: event.effective_at.unwrap_or(event.recorded_at),
                    recorded_at: event.recorded_at,
                    supersedes_event_id: event.supersedes_event_id,
                    claim_state: "superseded".to_string(),
                    ledger_entry_id: synthetic_ledger_id,
                    correlation_id: event.correlation_id.clone(),
                };
                self.projection.insert_decision_history(history).await?;
            }
            _ => {
                return Err(ContextError::InvalidEvent(format!(
                    "unsupported event type: {}",
                    event_type
                )));
            }
        }

        let link = EvidenceLinkRow {
            link_id: Uuid::new_v4(),
            projected_table: "direct_write".to_string(),
            projected_id: event.event_id,
            ledger_entry_id: synthetic_ledger_id,
            entry_type: format!("memory.{}", event_type),
            entry_hash: "direct-write-no-ledger".to_string(),
            chain_position: 0,
        };
        self.projection.insert_evidence_link(link).await?;

        Ok(WriteEventResponse {
            event_id: event.event_id,
            event_type,
            projected: true,
        })
    }

    pub async fn get_decision_history(
        &self,
        agent_id: &str,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<DecisionHistoryRow>, ContextError> {
        self.check_auth(agent_id, tenant_id, AuthAction::ReadHistory)
            .await?;
        Ok(self
            .projection
            .get_decision_history(tenant_id, subject_id)
            .await?)
    }

    pub async fn get_projection_status(&self) -> Result<ProjectionStatus, ContextError> {
        let entry_types = [
            "memory.observation.recorded",
            "memory.assertion.recorded",
            "memory.dissent.recorded",
            "memory.decision.recorded",
            "memory.decision.superseded",
            "memory.action.authorized",
            "memory.action.completed",
            "memory.outcome.recorded",
        ];

        let mut checkpoints = Vec::new();
        for et in &entry_types {
            let cp = self.projection.get_checkpoint(et).await?;
            checkpoints.push(CheckpointInfo {
                entry_type: et.to_string(),
                last_position: cp.as_ref().map(|c| c.last_position).unwrap_or(0),
                updated_at: cp.map(|c| c.updated_at),
            });
        }

        Ok(ProjectionStatus {
            checkpoints,
            is_healthy: true,
        })
    }

    async fn check_auth(
        &self,
        agent_id: &str,
        tenant_id: &str,
        action: AuthAction,
    ) -> Result<(), ContextError> {
        match self.auth.authorize(agent_id, tenant_id, action).await {
            AuthResult::Allowed => Ok(()),
            AuthResult::Denied(reason) => Err(ContextError::Forbidden(reason)),
        }
    }
}

fn to_memory_record_view(r: &MemoryRecordRow) -> MemoryRecordView {
    MemoryRecordView {
        record_id: r.record_id,
        event_type: r.event_type.clone(),
        actor: Actor {
            actor_id: r.actor_id.clone(),
            actor_type: r.actor_type.clone(),
        },
        content: r.record_content.clone(),
        recorded_at: r.recorded_at,
        claim_state: r.claim_state.clone(),
        has_dissent: r.has_dissent,
        ledger_entry_id: r.ledger_entry_id,
    }
}

fn to_evidence_view(l: crate::projection::types::EvidenceLinkRow) -> EvidenceLinkView {
    EvidenceLinkView {
        projected_table: l.projected_table,
        projected_id: l.projected_id,
        ledger_entry_id: l.ledger_entry_id,
        entry_hash: l.entry_hash,
        chain_position: l.chain_position,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use uuid::Uuid;

    use crate::auth::DevAuthProvider;
    use crate::projection::InMemoryProjectionRepository;

    use super::*;

    async fn setup_with_data() -> (
        Arc<InMemoryProjectionRepository>,
        ContextService<InMemoryProjectionRepository, DevAuthProvider>,
    ) {
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let auth = Arc::new(DevAuthProvider::open());
        let service = ContextService::new(repo.clone(), auth);

        let decision = CurrentDecisionRow {
            decision_id: Uuid::new_v4(),
            tenant_id: "acme".to_string(),
            subject_id: "inc-1".to_string(),
            actor_id: "carol".to_string(),
            actor_type: "human".to_string(),
            decision_content: serde_json::json!({"decision": "apply fix"}),
            effective_at: Utc::now(),
            recorded_at: Utc::now(),
            superseded_by: None,
            is_current: true,
            ledger_entry_id: Uuid::new_v4(),
            correlation_id: None,
            classification: "internal".to_string(),
        };
        repo.upsert_decision(decision).await.unwrap();

        let record = MemoryRecordRow {
            record_id: Uuid::new_v4(),
            tenant_id: "acme".to_string(),
            subject_id: "inc-1".to_string(),
            event_type: OBSERVATION_RECORDED.to_string(),
            actor_id: "alice".to_string(),
            actor_type: "human".to_string(),
            record_content: serde_json::json!({"observation": "server overloaded"}),
            effective_at: None,
            recorded_at: Utc::now(),
            claim_state: "asserted".to_string(),
            has_dissent: false,
            correlation_id: None,
            classification: "internal".to_string(),
            ledger_entry_id: Uuid::new_v4(),
        };
        repo.upsert_memory_record(record).await.unwrap();

        (repo, service)
    }

    #[tokio::test]
    async fn assembles_complete_bundle() {
        let (_repo, service) = setup_with_data().await;
        let bundle = service.get_context("agent", "acme", "inc-1").await.unwrap();

        assert_eq!(bundle.tenant_id, "acme");
        assert_eq!(bundle.subject_id, "inc-1");
        assert_eq!(bundle.effective_decisions.len(), 1);
        assert_eq!(bundle.observations.len(), 1);
        assert!(bundle.effective_decisions[0].is_current);
    }

    #[tokio::test]
    async fn returns_empty_for_unknown_subject() {
        let (_repo, service) = setup_with_data().await;
        let bundle = service
            .get_context("agent", "acme", "unknown")
            .await
            .unwrap();

        assert!(bundle.effective_decisions.is_empty());
        assert!(bundle.observations.is_empty());
    }

    #[tokio::test]
    async fn forbidden_for_unauthorized_agent() {
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let mut config = HashMap::new();
        config.insert("acme".to_string(), vec!["ops-agent".to_string()]);
        let auth = Arc::new(DevAuthProvider::with_config(config));
        let service = ContextService::new(repo, auth);

        let result = service.get_context("external-agent", "acme", "inc-1").await;
        assert!(matches!(result, Err(ContextError::Forbidden(_))));
    }

    #[tokio::test]
    async fn projection_status_returns_checkpoints() {
        let (_repo, service) = setup_with_data().await;
        let status = service.get_projection_status().await.unwrap();
        assert!(status.is_healthy);
        assert!(!status.checkpoints.is_empty());
    }
}
