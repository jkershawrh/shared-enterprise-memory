use async_trait::async_trait;
use uuid::Uuid;

use super::repository::{ProjectionError, ProjectionRepository};
use super::types::*;

pub struct PostgresProjectionRepository {
    pool: deadpool_postgres::Pool,
}

impl PostgresProjectionRepository {
    pub fn new(pool: deadpool_postgres::Pool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProjectionRepository for PostgresProjectionRepository {
    async fn get_checkpoint(
        &self,
        entry_type: &str,
    ) -> Result<Option<ProjectionCheckpoint>, ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        let row = client
            .query_opt(
                "SELECT entry_type, last_position, last_entry_id, updated_at \
                 FROM are_memory.projection_checkpoints WHERE entry_type = $1",
                &[&entry_type],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;

        Ok(row.map(|r| ProjectionCheckpoint {
            entry_type: r.get("entry_type"),
            last_position: r.get("last_position"),
            last_entry_id: r.get("last_entry_id"),
            updated_at: r.get("updated_at"),
        }))
    }

    async fn update_checkpoint(
        &self,
        entry_type: &str,
        position: i64,
        entry_id: Uuid,
    ) -> Result<(), ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        client
            .execute(
                "INSERT INTO are_memory.projection_checkpoints (entry_type, last_position, last_entry_id, updated_at) \
                 VALUES ($1, $2, $3, NOW()) \
                 ON CONFLICT (entry_type) DO UPDATE SET last_position = $2, last_entry_id = $3, updated_at = NOW()",
                &[&entry_type, &position, &entry_id],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        Ok(())
    }

    async fn upsert_memory_record(&self, record: MemoryRecordRow) -> Result<(), ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        client
            .execute(
                "INSERT INTO are_memory.memory_records \
                 (record_id, tenant_id, subject_id, event_type, actor_id, actor_type, \
                  record_content, effective_at, recorded_at, claim_state, has_dissent, \
                  correlation_id, classification, ledger_entry_id) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
                 ON CONFLICT (record_id) DO UPDATE SET \
                  record_content = EXCLUDED.record_content, \
                  claim_state = EXCLUDED.claim_state, \
                  has_dissent = EXCLUDED.has_dissent",
                &[
                    &record.record_id,
                    &record.tenant_id,
                    &record.subject_id,
                    &record.event_type,
                    &record.actor_id,
                    &record.actor_type,
                    &record.record_content,
                    &record.effective_at,
                    &record.recorded_at,
                    &record.claim_state,
                    &record.has_dissent,
                    &record.correlation_id,
                    &record.classification,
                    &record.ledger_entry_id,
                ],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        Ok(())
    }

    async fn get_memory_records(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<MemoryRecordRow>, ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        let rows = client
            .query(
                "SELECT record_id, tenant_id, subject_id, event_type, actor_id, actor_type, \
                        record_content, effective_at, recorded_at, claim_state, has_dissent, \
                        correlation_id, classification, ledger_entry_id \
                 FROM are_memory.memory_records \
                 WHERE tenant_id = $1 AND subject_id = $2 \
                 ORDER BY recorded_at",
                &[&tenant_id, &subject_id],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;

        Ok(rows
            .iter()
            .map(|r| MemoryRecordRow {
                record_id: r.get("record_id"),
                tenant_id: r.get("tenant_id"),
                subject_id: r.get("subject_id"),
                event_type: r.get("event_type"),
                actor_id: r.get("actor_id"),
                actor_type: r.get("actor_type"),
                record_content: r.get("record_content"),
                effective_at: r.get("effective_at"),
                recorded_at: r.get("recorded_at"),
                claim_state: r.get("claim_state"),
                has_dissent: r.get("has_dissent"),
                correlation_id: r.get("correlation_id"),
                classification: r.get("classification"),
                ledger_entry_id: r.get("ledger_entry_id"),
            })
            .collect())
    }

    async fn mark_record_dissented(&self, record_id: Uuid) -> Result<(), ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        client
            .execute(
                "UPDATE are_memory.memory_records SET has_dissent = TRUE WHERE record_id = $1",
                &[&record_id],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        Ok(())
    }

    async fn upsert_decision(&self, d: CurrentDecisionRow) -> Result<(), ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        client
            .execute(
                "INSERT INTO are_memory.current_decisions \
                 (decision_id, tenant_id, subject_id, actor_id, actor_type, \
                  decision_content, effective_at, recorded_at, superseded_by, \
                  is_current, ledger_entry_id, correlation_id, classification) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) \
                 ON CONFLICT (decision_id) DO UPDATE SET \
                  decision_content = EXCLUDED.decision_content, \
                  is_current = EXCLUDED.is_current, \
                  superseded_by = EXCLUDED.superseded_by",
                &[
                    &d.decision_id,
                    &d.tenant_id,
                    &d.subject_id,
                    &d.actor_id,
                    &d.actor_type,
                    &d.decision_content,
                    &d.effective_at,
                    &d.recorded_at,
                    &d.superseded_by,
                    &d.is_current,
                    &d.ledger_entry_id,
                    &d.correlation_id,
                    &d.classification,
                ],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        Ok(())
    }

    async fn get_current_decisions(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<CurrentDecisionRow>, ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        let rows = client
            .query(
                "SELECT decision_id, tenant_id, subject_id, actor_id, actor_type, \
                        decision_content, effective_at, recorded_at, superseded_by, \
                        is_current, ledger_entry_id, correlation_id, classification \
                 FROM are_memory.current_decisions \
                 WHERE tenant_id = $1 AND subject_id = $2 AND is_current = TRUE \
                 ORDER BY effective_at",
                &[&tenant_id, &subject_id],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;

        Ok(rows
            .iter()
            .map(|r| CurrentDecisionRow {
                decision_id: r.get("decision_id"),
                tenant_id: r.get("tenant_id"),
                subject_id: r.get("subject_id"),
                actor_id: r.get("actor_id"),
                actor_type: r.get("actor_type"),
                decision_content: r.get("decision_content"),
                effective_at: r.get("effective_at"),
                recorded_at: r.get("recorded_at"),
                superseded_by: r.get("superseded_by"),
                is_current: r.get("is_current"),
                ledger_entry_id: r.get("ledger_entry_id"),
                correlation_id: r.get("correlation_id"),
                classification: r.get("classification"),
            })
            .collect())
    }

    async fn mark_decision_superseded(
        &self,
        decision_id: Uuid,
        superseded_by: Uuid,
    ) -> Result<(), ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        client
            .execute(
                "UPDATE are_memory.current_decisions \
                 SET is_current = FALSE, superseded_by = $2 \
                 WHERE decision_id = $1",
                &[&decision_id, &superseded_by],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        Ok(())
    }

    async fn insert_decision_history(&self, h: DecisionHistoryRow) -> Result<(), ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        client
            .execute(
                "INSERT INTO are_memory.decision_history \
                 (history_id, decision_id, tenant_id, subject_id, actor_id, actor_type, \
                  decision_content, effective_at, recorded_at, supersedes_event_id, \
                  claim_state, ledger_entry_id, correlation_id) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) \
                 ON CONFLICT (history_id) DO NOTHING",
                &[
                    &h.history_id,
                    &h.decision_id,
                    &h.tenant_id,
                    &h.subject_id,
                    &h.actor_id,
                    &h.actor_type,
                    &h.decision_content,
                    &h.effective_at,
                    &h.recorded_at,
                    &h.supersedes_event_id,
                    &h.claim_state,
                    &h.ledger_entry_id,
                    &h.correlation_id,
                ],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        Ok(())
    }

    async fn get_decision_history(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<DecisionHistoryRow>, ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        let rows = client
            .query(
                "SELECT history_id, decision_id, tenant_id, subject_id, actor_id, actor_type, \
                        decision_content, effective_at, recorded_at, supersedes_event_id, \
                        claim_state, ledger_entry_id, correlation_id \
                 FROM are_memory.decision_history \
                 WHERE tenant_id = $1 AND subject_id = $2 \
                 ORDER BY recorded_at",
                &[&tenant_id, &subject_id],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;

        Ok(rows
            .iter()
            .map(|r| DecisionHistoryRow {
                history_id: r.get("history_id"),
                decision_id: r.get("decision_id"),
                tenant_id: r.get("tenant_id"),
                subject_id: r.get("subject_id"),
                actor_id: r.get("actor_id"),
                actor_type: r.get("actor_type"),
                decision_content: r.get("decision_content"),
                effective_at: r.get("effective_at"),
                recorded_at: r.get("recorded_at"),
                supersedes_event_id: r.get("supersedes_event_id"),
                claim_state: r.get("claim_state"),
                ledger_entry_id: r.get("ledger_entry_id"),
                correlation_id: r.get("correlation_id"),
            })
            .collect())
    }

    async fn insert_dissent(&self, d: DissentRecordRow) -> Result<(), ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        client
            .execute(
                "INSERT INTO are_memory.dissent_records \
                 (dissent_id, tenant_id, subject_id, target_event_id, actor_id, actor_type, \
                  dissent_content, recorded_at, ledger_entry_id, correlation_id) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
                 ON CONFLICT (dissent_id) DO NOTHING",
                &[
                    &d.dissent_id,
                    &d.tenant_id,
                    &d.subject_id,
                    &d.target_event_id,
                    &d.actor_id,
                    &d.actor_type,
                    &d.dissent_content,
                    &d.recorded_at,
                    &d.ledger_entry_id,
                    &d.correlation_id,
                ],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        Ok(())
    }

    async fn get_dissent_for_subject(
        &self,
        tenant_id: &str,
        subject_id: &str,
    ) -> Result<Vec<DissentRecordRow>, ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        let rows = client
            .query(
                "SELECT dissent_id, tenant_id, subject_id, target_event_id, actor_id, actor_type, \
                        dissent_content, recorded_at, ledger_entry_id, correlation_id \
                 FROM are_memory.dissent_records \
                 WHERE tenant_id = $1 AND subject_id = $2 \
                 ORDER BY recorded_at",
                &[&tenant_id, &subject_id],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;

        Ok(rows.iter().map(map_dissent_row).collect())
    }

    async fn get_dissent_for_target(
        &self,
        target_event_id: Uuid,
    ) -> Result<Vec<DissentRecordRow>, ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        let rows = client
            .query(
                "SELECT dissent_id, tenant_id, subject_id, target_event_id, actor_id, actor_type, \
                        dissent_content, recorded_at, ledger_entry_id, correlation_id \
                 FROM are_memory.dissent_records \
                 WHERE target_event_id = $1 \
                 ORDER BY recorded_at",
                &[&target_event_id],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;

        Ok(rows.iter().map(map_dissent_row).collect())
    }

    async fn insert_evidence_link(&self, link: EvidenceLinkRow) -> Result<(), ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        client
            .execute(
                "INSERT INTO are_memory.evidence_links \
                 (link_id, projected_table, projected_id, ledger_entry_id, \
                  entry_type, entry_hash, chain_position) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7) \
                 ON CONFLICT (projected_table, projected_id, ledger_entry_id) DO NOTHING",
                &[
                    &link.link_id,
                    &link.projected_table,
                    &link.projected_id,
                    &link.ledger_entry_id,
                    &link.entry_type,
                    &link.entry_hash,
                    &link.chain_position,
                ],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        Ok(())
    }

    async fn get_evidence_links(
        &self,
        projected_table: &str,
        projected_id: Uuid,
    ) -> Result<Vec<EvidenceLinkRow>, ProjectionError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProjectionError::Unavailable)?;
        let rows = client
            .query(
                "SELECT link_id, projected_table, projected_id, ledger_entry_id, \
                        entry_type, entry_hash, chain_position \
                 FROM are_memory.evidence_links \
                 WHERE projected_table = $1 AND projected_id = $2",
                &[&projected_table, &projected_id],
            )
            .await
            .map_err(|_| ProjectionError::Unavailable)?;

        Ok(rows
            .iter()
            .map(|r| EvidenceLinkRow {
                link_id: r.get("link_id"),
                projected_table: r.get("projected_table"),
                projected_id: r.get("projected_id"),
                ledger_entry_id: r.get("ledger_entry_id"),
                entry_type: r.get("entry_type"),
                entry_hash: r.get("entry_hash"),
                chain_position: r.get("chain_position"),
            })
            .collect())
    }
}

fn map_dissent_row(r: &tokio_postgres::Row) -> DissentRecordRow {
    DissentRecordRow {
        dissent_id: r.get("dissent_id"),
        tenant_id: r.get("tenant_id"),
        subject_id: r.get("subject_id"),
        target_event_id: r.get("target_event_id"),
        actor_id: r.get("actor_id"),
        actor_type: r.get("actor_type"),
        dissent_content: r.get("dissent_content"),
        recorded_at: r.get("recorded_at"),
        ledger_entry_id: r.get("ledger_entry_id"),
        correlation_id: r.get("correlation_id"),
    }
}
