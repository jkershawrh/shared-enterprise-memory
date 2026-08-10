use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::types::*;

#[derive(Debug, Error, Clone)]
pub enum ProjectionError {
    #[error("projection store unavailable")]
    Unavailable,
    #[error("not found")]
    NotFound,
}

#[async_trait]
pub trait ProjectionRepository: Send + Sync {
    // Checkpoints
    async fn get_checkpoint(
        &self,
        entry_type: &str,
    ) -> Result<Option<ProjectionCheckpoint>, ProjectionError>;
    async fn update_checkpoint(
        &self,
        entry_type: &str,
        position: i64,
        entry_id: Uuid,
    ) -> Result<(), ProjectionError>;

    // Memory records
    async fn upsert_memory_record(&self, record: MemoryRecordRow) -> Result<(), ProjectionError>;
    async fn get_memory_records(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<MemoryRecordRow>, ProjectionError>;
    async fn mark_record_dissented(&self, record_id: Uuid) -> Result<(), ProjectionError>;

    // Decisions
    async fn upsert_decision(&self, decision: CurrentDecisionRow) -> Result<(), ProjectionError>;
    async fn get_current_decisions(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<CurrentDecisionRow>, ProjectionError>;
    async fn mark_decision_superseded(
        &self,
        decision_id: Uuid,
        superseded_by: Uuid,
    ) -> Result<(), ProjectionError>;
    async fn insert_decision_history(
        &self,
        history: DecisionHistoryRow,
    ) -> Result<(), ProjectionError>;
    async fn get_decision_history(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<DecisionHistoryRow>, ProjectionError>;

    // Dissent
    async fn insert_dissent(&self, dissent: DissentRecordRow) -> Result<(), ProjectionError>;
    async fn get_dissent_for_subject(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<DissentRecordRow>, ProjectionError>;
    async fn get_dissent_for_target(
        &self,
        target_event_id: Uuid,
    ) -> Result<Vec<DissentRecordRow>, ProjectionError>;

    // Evidence links
    async fn insert_evidence_link(&self, link: EvidenceLinkRow) -> Result<(), ProjectionError>;
    async fn get_evidence_links(
        &self,
        projected_table: &str,
        projected_id: Uuid,
    ) -> Result<Vec<EvidenceLinkRow>, ProjectionError>;
}

#[derive(Default)]
struct Inner {
    checkpoints: HashMap<String, ProjectionCheckpoint>,
    memory_records: HashMap<Uuid, MemoryRecordRow>,
    decisions: HashMap<Uuid, CurrentDecisionRow>,
    decision_history: Vec<DecisionHistoryRow>,
    dissent_records: Vec<DissentRecordRow>,
    evidence_links: Vec<EvidenceLinkRow>,
}

#[derive(Default, Clone)]
pub struct InMemoryProjectionRepository {
    inner: Arc<RwLock<Inner>>,
}

#[async_trait]
impl ProjectionRepository for InMemoryProjectionRepository {
    async fn get_checkpoint(
        &self,
        entry_type: &str,
    ) -> Result<Option<ProjectionCheckpoint>, ProjectionError> {
        let guard = self.inner.read().await;
        Ok(guard.checkpoints.get(entry_type).cloned())
    }

    async fn update_checkpoint(
        &self,
        entry_type: &str,
        position: i64,
        entry_id: Uuid,
    ) -> Result<(), ProjectionError> {
        let mut guard = self.inner.write().await;
        guard.checkpoints.insert(
            entry_type.to_string(),
            ProjectionCheckpoint {
                entry_type: entry_type.to_string(),
                last_position: position,
                last_entry_id: Some(entry_id),
                updated_at: chrono::Utc::now(),
            },
        );
        Ok(())
    }

    async fn upsert_memory_record(&self, record: MemoryRecordRow) -> Result<(), ProjectionError> {
        let mut guard = self.inner.write().await;
        guard.memory_records.insert(record.record_id, record);
        Ok(())
    }

    async fn get_memory_records(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<MemoryRecordRow>, ProjectionError> {
        let guard = self.inner.read().await;
        Ok(guard
            .memory_records
            .values()
            .filter(|r| r.tenant_id == tenant_id && r.subject_id == subject_id)
            .cloned()
            .collect())
    }

    async fn mark_record_dissented(&self, record_id: Uuid) -> Result<(), ProjectionError> {
        let mut guard = self.inner.write().await;
        if let Some(record) = guard.memory_records.get_mut(&record_id) {
            record.has_dissent = true;
            Ok(())
        } else {
            Ok(())
        }
    }

    async fn upsert_decision(&self, decision: CurrentDecisionRow) -> Result<(), ProjectionError> {
        let mut guard = self.inner.write().await;
        guard.decisions.insert(decision.decision_id, decision);
        Ok(())
    }

    async fn get_current_decisions(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<CurrentDecisionRow>, ProjectionError> {
        let guard = self.inner.read().await;
        Ok(guard
            .decisions
            .values()
            .filter(|d| d.tenant_id == tenant_id && d.subject_id == subject_id && d.is_current)
            .cloned()
            .collect())
    }

    async fn mark_decision_superseded(
        &self,
        decision_id: Uuid,
        superseded_by: Uuid,
    ) -> Result<(), ProjectionError> {
        let mut guard = self.inner.write().await;
        if let Some(decision) = guard.decisions.get_mut(&decision_id) {
            decision.is_current = false;
            decision.superseded_by = Some(superseded_by);
        }
        Ok(())
    }

    async fn insert_decision_history(
        &self,
        history: DecisionHistoryRow,
    ) -> Result<(), ProjectionError> {
        let mut guard = self.inner.write().await;
        if !guard
            .decision_history
            .iter()
            .any(|h| h.history_id == history.history_id)
        {
            guard.decision_history.push(history);
        }
        Ok(())
    }

    async fn get_decision_history(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<DecisionHistoryRow>, ProjectionError> {
        let guard = self.inner.read().await;
        Ok(guard
            .decision_history
            .iter()
            .filter(|h| h.tenant_id == tenant_id && h.subject_id == subject_id)
            .cloned()
            .collect())
    }

    async fn insert_dissent(&self, dissent: DissentRecordRow) -> Result<(), ProjectionError> {
        let mut guard = self.inner.write().await;
        if !guard
            .dissent_records
            .iter()
            .any(|d| d.dissent_id == dissent.dissent_id)
        {
            guard.dissent_records.push(dissent);
        }
        Ok(())
    }

    async fn get_dissent_for_subject(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<DissentRecordRow>, ProjectionError> {
        let guard = self.inner.read().await;
        Ok(guard
            .dissent_records
            .iter()
            .filter(|d| d.tenant_id == tenant_id && d.subject_id == subject_id)
            .cloned()
            .collect())
    }

    async fn get_dissent_for_target(
        &self,
        target_event_id: Uuid,
    ) -> Result<Vec<DissentRecordRow>, ProjectionError> {
        let guard = self.inner.read().await;
        Ok(guard
            .dissent_records
            .iter()
            .filter(|d| d.target_event_id == target_event_id)
            .cloned()
            .collect())
    }

    async fn insert_evidence_link(&self, link: EvidenceLinkRow) -> Result<(), ProjectionError> {
        let mut guard = self.inner.write().await;
        let exists = guard.evidence_links.iter().any(|l| {
            l.projected_table == link.projected_table
                && l.projected_id == link.projected_id
                && l.ledger_entry_id == link.ledger_entry_id
        });
        if !exists {
            guard.evidence_links.push(link);
        }
        Ok(())
    }

    async fn get_evidence_links(
        &self,
        projected_table: &str,
        projected_id: Uuid,
    ) -> Result<Vec<EvidenceLinkRow>, ProjectionError> {
        let guard = self.inner.read().await;
        Ok(guard
            .evidence_links
            .iter()
            .filter(|l| l.projected_table == projected_table && l.projected_id == projected_id)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn test_memory_record(id: Uuid, tenant: &str, subject: &str) -> MemoryRecordRow {
        MemoryRecordRow {
            record_id: id,
            tenant_id: tenant.to_string(),
            subject_id: subject.to_string(),
            event_type: "observation.recorded".to_string(),
            actor_id: "alice".to_string(),
            actor_type: "human".to_string(),
            record_content: serde_json::json!({"note": "test"}),
            effective_at: None,
            recorded_at: Utc::now(),
            claim_state: "asserted".to_string(),
            has_dissent: false,
            correlation_id: None,
            classification: "internal".to_string(),
            ledger_entry_id: Uuid::new_v4(),
        }
    }

    fn test_decision(id: Uuid, tenant: &str, subject: &str) -> CurrentDecisionRow {
        CurrentDecisionRow {
            decision_id: id,
            tenant_id: tenant.to_string(),
            subject_id: subject.to_string(),
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
        }
    }

    #[tokio::test]
    async fn memory_record_crud() {
        let repo = InMemoryProjectionRepository::default();
        let id = Uuid::new_v4();
        let record = test_memory_record(id, "acme", "inc-1");
        repo.upsert_memory_record(record).await.unwrap();

        let records = repo.get_memory_records("acme", "inc-1").await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record_id, id);
    }

    #[tokio::test]
    async fn memory_record_upsert_is_idempotent() {
        let repo = InMemoryProjectionRepository::default();
        let id = Uuid::new_v4();
        let record = test_memory_record(id, "acme", "inc-1");
        repo.upsert_memory_record(record.clone()).await.unwrap();
        repo.upsert_memory_record(record).await.unwrap();

        let records = repo.get_memory_records("acme", "inc-1").await.unwrap();
        assert_eq!(records.len(), 1);
    }

    #[tokio::test]
    async fn mark_record_dissented() {
        let repo = InMemoryProjectionRepository::default();
        let id = Uuid::new_v4();
        let record = test_memory_record(id, "acme", "inc-1");
        repo.upsert_memory_record(record).await.unwrap();
        repo.mark_record_dissented(id).await.unwrap();

        let records = repo.get_memory_records("acme", "inc-1").await.unwrap();
        assert!(records[0].has_dissent);
    }

    #[tokio::test]
    async fn decision_current_only() {
        let repo = InMemoryProjectionRepository::default();
        let d1 = Uuid::new_v4();
        let d2 = Uuid::new_v4();

        let mut decision1 = test_decision(d1, "acme", "inc-1");
        repo.upsert_decision(decision1.clone()).await.unwrap();

        let decision2 = test_decision(d2, "acme", "inc-1");
        repo.upsert_decision(decision2).await.unwrap();

        decision1.is_current = false;
        decision1.superseded_by = Some(d2);
        repo.upsert_decision(decision1).await.unwrap();

        let current = repo.get_current_decisions("acme", "inc-1").await.unwrap();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].decision_id, d2);
    }

    #[tokio::test]
    async fn mark_decision_superseded() {
        let repo = InMemoryProjectionRepository::default();
        let d1 = Uuid::new_v4();
        let d2 = Uuid::new_v4();

        repo.upsert_decision(test_decision(d1, "acme", "inc-1"))
            .await
            .unwrap();
        repo.mark_decision_superseded(d1, d2).await.unwrap();

        let current = repo.get_current_decisions("acme", "inc-1").await.unwrap();
        assert!(current.is_empty());
    }

    #[tokio::test]
    async fn checkpoint_starts_empty() {
        let repo = InMemoryProjectionRepository::default();
        let cp = repo
            .get_checkpoint("memory.observation.recorded")
            .await
            .unwrap();
        assert!(cp.is_none());
    }

    #[tokio::test]
    async fn checkpoint_advances() {
        let repo = InMemoryProjectionRepository::default();
        let id = Uuid::new_v4();
        repo.update_checkpoint("memory.observation.recorded", 5, id)
            .await
            .unwrap();

        let cp = repo
            .get_checkpoint("memory.observation.recorded")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cp.last_position, 5);
        assert_eq!(cp.last_entry_id, Some(id));
    }

    #[tokio::test]
    async fn dissent_insert_is_idempotent() {
        let repo = InMemoryProjectionRepository::default();
        let dissent = DissentRecordRow {
            dissent_id: Uuid::new_v4(),
            tenant_id: "acme".to_string(),
            subject_id: "inc-1".to_string(),
            target_event_id: Uuid::new_v4(),
            actor_id: "bob".to_string(),
            actor_type: "human".to_string(),
            dissent_content: serde_json::json!({"reason": "disagree"}),
            recorded_at: Utc::now(),
            ledger_entry_id: Uuid::new_v4(),
            correlation_id: None,
        };
        repo.insert_dissent(dissent.clone()).await.unwrap();
        repo.insert_dissent(dissent).await.unwrap();

        let records = repo.get_dissent_for_subject("acme", "inc-1").await.unwrap();
        assert_eq!(records.len(), 1);
    }

    #[tokio::test]
    async fn evidence_link_uniqueness() {
        let repo = InMemoryProjectionRepository::default();
        let link = EvidenceLinkRow {
            link_id: Uuid::new_v4(),
            projected_table: "memory_records".to_string(),
            projected_id: Uuid::new_v4(),
            ledger_entry_id: Uuid::new_v4(),
            entry_type: "memory.observation.recorded".to_string(),
            entry_hash: "hash-1".to_string(),
            chain_position: 1,
        };
        repo.insert_evidence_link(link.clone()).await.unwrap();
        repo.insert_evidence_link(link.clone()).await.unwrap();

        let links = repo
            .get_evidence_links("memory_records", link.projected_id)
            .await
            .unwrap();
        assert_eq!(links.len(), 1);
    }
}
