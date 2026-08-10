use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tracing::{info, warn};
use uuid::Uuid;

use crate::contract::memory_event::*;
use crate::ledger::{LedgerEntry, LedgerReader};
use crate::metrics;

use super::repository::{ProjectionError, ProjectionRepository};
use super::types::*;

#[derive(Debug)]
pub struct ProjectionCycleResult {
    pub entries_processed: usize,
    pub errors: usize,
}

pub struct Projector<L: LedgerReader, P: ProjectionRepository> {
    ledger: Arc<L>,
    projection: Arc<P>,
    batch_size: i64,
    watched_entry_types: Vec<String>,
}

impl<L: LedgerReader + 'static, P: ProjectionRepository + 'static> Projector<L, P> {
    pub fn new(ledger: Arc<L>, projection: Arc<P>, batch_size: i64) -> Self {
        let watched_entry_types = VALID_EVENT_TYPES
            .iter()
            .map(|et| format!("{MEMORY_ENTRY_TYPE_PREFIX}{et}"))
            .collect();
        Self {
            ledger,
            projection,
            batch_size,
            watched_entry_types,
        }
    }

    pub async fn process_cycle(&self) -> Result<ProjectionCycleResult, ProjectionError> {
        let mut total_processed = 0;
        let mut total_errors = 0;

        for entry_type in &self.watched_entry_types {
            match self.process_chain(entry_type).await {
                Ok((processed, errors)) => {
                    total_processed += processed;
                    total_errors += errors;
                }
                Err(e) => {
                    warn!(entry_type, error = %e, "failed to process chain");
                    total_errors += 1;
                }
            }
        }

        Ok(ProjectionCycleResult {
            entries_processed: total_processed,
            errors: total_errors,
        })
    }

    async fn process_chain(&self, entry_type: &str) -> Result<(usize, usize), ProjectionError> {
        let checkpoint = self.projection.get_checkpoint(entry_type).await?;
        let after_position = checkpoint.map(|c| c.last_position).unwrap_or(0);

        let entries = self
            .ledger
            .get_entries_by_type_paged(entry_type, after_position, self.batch_size)
            .await
            .map_err(|_| ProjectionError::Unavailable)?;

        let mut processed = 0;
        let mut errors = 0;

        for entry in &entries {
            match self.project_entry(entry).await {
                Ok(()) => {
                    metrics::PROJECTION_ENTRIES_PROCESSED.inc();
                    processed += 1;
                }
                Err(e) => {
                    warn!(
                        entry_id = %entry.entry_id,
                        entry_type = %entry.entry_type,
                        error = %e,
                        "failed to project entry, skipping"
                    );
                    metrics::PROJECTION_ERRORS_TOTAL
                        .with_label_values(&[&entry.entry_type])
                        .inc();
                    errors += 1;
                }
            }
            self.projection
                .update_checkpoint(entry_type, entry.chain_position, entry.entry_id)
                .await?;
        }

        Ok((processed, errors))
    }

    async fn project_entry(&self, entry: &LedgerEntry) -> Result<(), ProjectionError> {
        let event =
            MemoryEventV1::from_bytes(&entry.content).map_err(|_| ProjectionError::Unavailable)?;

        match event.event_type.as_str() {
            OBSERVATION_RECORDED | ASSERTION_RECORDED | ACTION_AUTHORIZED | ACTION_COMPLETED
            | OUTCOME_RECORDED => {
                self.project_memory_record(&event, entry).await?;
            }
            DISSENT_RECORDED => {
                self.project_dissent(&event, entry).await?;
            }
            DECISION_RECORDED => {
                self.project_decision(&event, entry).await?;
            }
            DECISION_SUPERSEDED => {
                self.project_supersession(&event, entry).await?;
            }
            _ => {
                warn!(
                    event_type = %event.event_type,
                    "unknown event type, skipping"
                );
            }
        }

        Ok(())
    }

    async fn project_memory_record(
        &self,
        event: &MemoryEventV1,
        entry: &LedgerEntry,
    ) -> Result<(), ProjectionError> {
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
            ledger_entry_id: entry.entry_id,
        };
        self.projection.upsert_memory_record(record).await?;
        self.link_evidence("memory_records", event.event_id, entry)
            .await
    }

    async fn project_dissent(
        &self,
        event: &MemoryEventV1,
        entry: &LedgerEntry,
    ) -> Result<(), ProjectionError> {
        let dissent = DissentRecordRow {
            dissent_id: event.event_id,
            tenant_id: event.tenant_id.clone(),
            subject_id: event.subject_id.clone(),
            target_event_id: event.supersedes_event_id.unwrap_or(Uuid::nil()),
            actor_id: event.actor.actor_id.clone(),
            actor_type: event.actor.actor_type.clone(),
            dissent_content: event.payload.clone(),
            recorded_at: event.recorded_at,
            ledger_entry_id: entry.entry_id,
            correlation_id: event.correlation_id.clone(),
        };
        self.projection.insert_dissent(dissent).await?;

        if let Some(target_id) = event.supersedes_event_id {
            self.projection.mark_record_dissented(target_id).await?;
        }

        self.link_evidence("dissent_records", event.event_id, entry)
            .await
    }

    async fn project_decision(
        &self,
        event: &MemoryEventV1,
        entry: &LedgerEntry,
    ) -> Result<(), ProjectionError> {
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
            ledger_entry_id: entry.entry_id,
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
            ledger_entry_id: entry.entry_id,
            correlation_id: event.correlation_id.clone(),
        };
        self.projection.insert_decision_history(history).await?;

        self.link_evidence("current_decisions", event.event_id, entry)
            .await
    }

    async fn project_supersession(
        &self,
        event: &MemoryEventV1,
        entry: &LedgerEntry,
    ) -> Result<(), ProjectionError> {
        if let Some(old_decision_id) = event.supersedes_event_id {
            self.projection
                .mark_decision_superseded(old_decision_id, event.event_id)
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
            ledger_entry_id: entry.entry_id,
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
            ledger_entry_id: entry.entry_id,
            correlation_id: event.correlation_id.clone(),
        };
        self.projection.insert_decision_history(history).await?;

        self.link_evidence("current_decisions", event.event_id, entry)
            .await
    }

    async fn link_evidence(
        &self,
        table: &str,
        projected_id: Uuid,
        entry: &LedgerEntry,
    ) -> Result<(), ProjectionError> {
        let link = EvidenceLinkRow {
            link_id: Uuid::new_v4(),
            projected_table: table.to_string(),
            projected_id,
            ledger_entry_id: entry.entry_id,
            entry_type: entry.entry_type.clone(),
            entry_hash: entry.entry_hash.clone(),
            chain_position: entry.chain_position,
        };
        self.projection.insert_evidence_link(link).await
    }

    pub fn start_background(
        self: Arc<Self>,
        poll_interval: Duration,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        tokio::spawn(async move {
            info!("projector background task started");
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        info!("projector shutting down");
                        break;
                    }
                    _ = tokio::time::sleep(poll_interval) => {
                        let timer = metrics::PROJECTION_CYCLE_DURATION.start_timer();
                        match self.process_cycle().await {
                            Ok(result) if result.entries_processed > 0 => {
                                info!(
                                    processed = result.entries_processed,
                                    errors = result.errors,
                                    "projection cycle complete"
                                );
                            }
                            Err(e) => {
                                warn!(error = %e, "projection cycle failed");
                            }
                            _ => {}
                        }
                        timer.observe_duration();
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::ledger::InMemoryLedgerReader;
    use crate::projection::InMemoryProjectionRepository;

    use super::*;

    fn make_event(event_type: &str, tenant: &str, subject: &str) -> MemoryEventV1 {
        MemoryEventV1 {
            event_id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            schema_version: MEMORY_EVENT_SCHEMA_VERSION.to_string(),
            tenant_id: tenant.to_string(),
            subject_id: subject.to_string(),
            actor: crate::contract::Actor {
                actor_id: "alice".to_string(),
                actor_type: "human".to_string(),
            },
            recorded_at: Utc::now(),
            effective_at: Some(Utc::now()),
            correlation_id: None,
            supersedes_event_id: None,
            classification: "internal".to_string(),
            claim_state: "asserted".to_string(),
            payload: serde_json::json!({"note": "test"}),
        }
    }

    async fn insert_event_async(
        reader: &InMemoryLedgerReader,
        event: &MemoryEventV1,
        position: i64,
    ) -> LedgerEntry {
        let entry = LedgerEntry {
            entry_id: Uuid::new_v4(),
            entry_type: event.ledger_entry_type(),
            agent_id: event.actor.actor_id.clone(),
            content: event.to_bytes(),
            content_type: "application/json".to_string(),
            source_id: "test".to_string(),
            correlation_id: event.correlation_id.clone(),
            entry_hash: format!("hash-{position}"),
            hash_version: "V3".to_string(),
            chain_position: position,
            written_ts: Utc::now(),
        };
        reader.insert_entry(entry.clone()).await;
        entry
    }

    fn build_projector(
        reader: Arc<InMemoryLedgerReader>,
        repo: Arc<InMemoryProjectionRepository>,
    ) -> Projector<InMemoryLedgerReader, InMemoryProjectionRepository> {
        Projector::new(reader, repo, 100)
    }

    #[tokio::test]
    async fn projects_observation() {
        let reader = Arc::new(InMemoryLedgerReader::default());
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let projector = build_projector(reader.clone(), repo.clone());

        let event = make_event(OBSERVATION_RECORDED, "acme", "inc-1");
        insert_event_async(&reader, &event, 1).await;

        projector.process_cycle().await.unwrap();

        let records = repo.get_memory_records("acme", "inc-1").await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event_type, OBSERVATION_RECORDED);
    }

    #[tokio::test]
    async fn projects_assertion() {
        let reader = Arc::new(InMemoryLedgerReader::default());
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let projector = build_projector(reader.clone(), repo.clone());

        let event = make_event(ASSERTION_RECORDED, "acme", "inc-1");
        insert_event_async(&reader, &event, 1).await;

        projector.process_cycle().await.unwrap();

        let records = repo.get_memory_records("acme", "inc-1").await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event_type, ASSERTION_RECORDED);
    }

    #[tokio::test]
    async fn projects_dissent_and_marks_target() {
        let reader = Arc::new(InMemoryLedgerReader::default());
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let projector = build_projector(reader.clone(), repo.clone());

        let assertion = make_event(ASSERTION_RECORDED, "acme", "inc-1");
        insert_event_async(&reader, &assertion, 1).await;
        projector.process_cycle().await.unwrap();

        let mut dissent_event = make_event(DISSENT_RECORDED, "acme", "inc-1");
        dissent_event.actor.actor_id = "bob".to_string();
        dissent_event.claim_state = "dissented".to_string();
        dissent_event.supersedes_event_id = Some(assertion.event_id);
        insert_event_async(&reader, &dissent_event, 1).await;
        projector.process_cycle().await.unwrap();

        let records = repo.get_memory_records("acme", "inc-1").await.unwrap();
        let target = records
            .iter()
            .find(|r| r.record_id == assertion.event_id)
            .unwrap();
        assert!(target.has_dissent);

        let dissents = repo
            .get_dissent_for_target(assertion.event_id)
            .await
            .unwrap();
        assert_eq!(dissents.len(), 1);
        assert_eq!(dissents[0].actor_id, "bob");
    }

    #[tokio::test]
    async fn projects_decision() {
        let reader = Arc::new(InMemoryLedgerReader::default());
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let projector = build_projector(reader.clone(), repo.clone());

        let mut event = make_event(DECISION_RECORDED, "acme", "inc-1");
        event.claim_state = "decided".to_string();
        event.actor.actor_id = "carol".to_string();
        insert_event_async(&reader, &event, 1).await;

        projector.process_cycle().await.unwrap();

        let decisions = repo.get_current_decisions("acme", "inc-1").await.unwrap();
        assert_eq!(decisions.len(), 1);
        assert!(decisions[0].is_current);

        let history = repo.get_decision_history("acme", "inc-1").await.unwrap();
        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn projects_supersession() {
        let reader = Arc::new(InMemoryLedgerReader::default());
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let projector = build_projector(reader.clone(), repo.clone());

        let mut original = make_event(DECISION_RECORDED, "acme", "inc-1");
        original.claim_state = "decided".to_string();
        insert_event_async(&reader, &original, 1).await;
        projector.process_cycle().await.unwrap();

        let mut superseding = make_event(DECISION_SUPERSEDED, "acme", "inc-1");
        superseding.claim_state = "superseded".to_string();
        superseding.supersedes_event_id = Some(original.event_id);
        superseding.payload = serde_json::json!({"decision": "new approach"});
        insert_event_async(&reader, &superseding, 1).await;
        projector.process_cycle().await.unwrap();

        let current = repo.get_current_decisions("acme", "inc-1").await.unwrap();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].decision_id, superseding.event_id);

        let history = repo.get_decision_history("acme", "inc-1").await.unwrap();
        assert_eq!(history.len(), 2);
    }

    #[tokio::test]
    async fn checkpoint_advances_after_batch() {
        let reader = Arc::new(InMemoryLedgerReader::default());
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let projector = build_projector(reader.clone(), repo.clone());

        let event = make_event(OBSERVATION_RECORDED, "acme", "inc-1");
        let entry = insert_event_async(&reader, &event, 1).await;

        projector.process_cycle().await.unwrap();

        let cp = repo
            .get_checkpoint("memory.observation.recorded")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cp.last_position, 1);
        assert_eq!(cp.last_entry_id, Some(entry.entry_id));
    }

    #[tokio::test]
    async fn skips_already_processed_entries() {
        let reader = Arc::new(InMemoryLedgerReader::default());
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let projector = build_projector(reader.clone(), repo.clone());

        let event = make_event(OBSERVATION_RECORDED, "acme", "inc-1");
        insert_event_async(&reader, &event, 1).await;

        projector.process_cycle().await.unwrap();
        let result = projector.process_cycle().await.unwrap();

        assert_eq!(result.entries_processed, 0);
        let records = repo.get_memory_records("acme", "inc-1").await.unwrap();
        assert_eq!(records.len(), 1);
    }

    #[tokio::test]
    async fn handles_malformed_content() {
        let reader = Arc::new(InMemoryLedgerReader::default());
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let projector = build_projector(reader.clone(), repo.clone());

        let bad_entry = LedgerEntry {
            entry_id: Uuid::new_v4(),
            entry_type: "memory.observation.recorded".to_string(),
            agent_id: "agent".to_string(),
            content: b"not valid json".to_vec(),
            content_type: "application/json".to_string(),
            source_id: "test".to_string(),
            correlation_id: None,
            entry_hash: "hash-1".to_string(),
            hash_version: "V3".to_string(),
            chain_position: 1,
            written_ts: Utc::now(),
        };
        reader.insert_entry(bad_entry).await;

        let good_event = make_event(OBSERVATION_RECORDED, "acme", "inc-1");
        insert_event_async(&reader, &good_event, 2).await;

        projector.process_cycle().await.unwrap();

        let cp = repo
            .get_checkpoint("memory.observation.recorded")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cp.last_position, 2);

        let records = repo.get_memory_records("acme", "inc-1").await.unwrap();
        assert_eq!(records.len(), 1);
    }

    #[tokio::test]
    async fn processes_multiple_chains() {
        let reader = Arc::new(InMemoryLedgerReader::default());
        let repo = Arc::new(InMemoryProjectionRepository::default());
        let projector = build_projector(reader.clone(), repo.clone());

        let obs = make_event(OBSERVATION_RECORDED, "acme", "inc-1");
        insert_event_async(&reader, &obs, 1).await;

        let mut dec = make_event(DECISION_RECORDED, "acme", "inc-1");
        dec.claim_state = "decided".to_string();
        insert_event_async(&reader, &dec, 1).await;

        let result = projector.process_cycle().await.unwrap();
        assert_eq!(result.entries_processed, 2);
    }

    #[tokio::test]
    async fn replay_produces_identical_state() {
        let reader = Arc::new(InMemoryLedgerReader::default());
        let repo = Arc::new(InMemoryProjectionRepository::default());

        let obs = make_event(OBSERVATION_RECORDED, "acme", "inc-1");
        insert_event_async(&reader, &obs, 1).await;

        let mut dec = make_event(DECISION_RECORDED, "acme", "inc-1");
        dec.claim_state = "decided".to_string();
        insert_event_async(&reader, &dec, 1).await;

        let projector = build_projector(reader.clone(), repo.clone());
        projector.process_cycle().await.unwrap();

        let decisions_1 = repo.get_current_decisions("acme", "inc-1").await.unwrap();
        let records_1 = repo.get_memory_records("acme", "inc-1").await.unwrap();

        // Reset checkpoints to replay
        let repo2 = Arc::new(InMemoryProjectionRepository::default());
        let projector2 = build_projector(reader.clone(), repo2.clone());
        projector2.process_cycle().await.unwrap();

        let decisions_2 = repo2.get_current_decisions("acme", "inc-1").await.unwrap();
        let records_2 = repo2.get_memory_records("acme", "inc-1").await.unwrap();

        assert_eq!(decisions_1.len(), decisions_2.len());
        assert_eq!(records_1.len(), records_2.len());
        assert_eq!(decisions_1[0].decision_id, decisions_2[0].decision_id);
        assert_eq!(records_1[0].record_id, records_2[0].record_id);
    }
}
