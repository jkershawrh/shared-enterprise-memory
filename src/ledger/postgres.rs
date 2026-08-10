use async_trait::async_trait;
use uuid::Uuid;

use super::{LedgerEntry, LedgerReadError, LedgerReader};

pub struct PostgresLedgerReader {
    pool: deadpool_postgres::Pool,
}

impl PostgresLedgerReader {
    pub fn new(pool: deadpool_postgres::Pool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LedgerReader for PostgresLedgerReader {
    async fn get_entries_by_type_paged(
        &self,
        entry_type: &str,
        after_position: i64,
        limit: i64,
    ) -> Result<Vec<LedgerEntry>, LedgerReadError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| LedgerReadError::Unavailable)?;
        let rows = client
            .query(
                "SELECT entry_id, entry_type, agent_id, content, content_type, \
                        source_id, correlation_id, entry_hash, hash_version, \
                        chain_position, written_ts \
                 FROM are_ledger.ledger_entries \
                 WHERE entry_type = $1 AND chain_position > $2 \
                 ORDER BY chain_position ASC \
                 LIMIT $3",
                &[&entry_type, &after_position, &limit],
            )
            .await
            .map_err(|_| LedgerReadError::Unavailable)?;

        Ok(rows.iter().map(map_row).collect())
    }

    async fn get_entry(&self, entry_id: Uuid) -> Result<LedgerEntry, LedgerReadError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| LedgerReadError::Unavailable)?;
        let row = client
            .query_opt(
                "SELECT entry_id, entry_type, agent_id, content, content_type, \
                        source_id, correlation_id, entry_hash, hash_version, \
                        chain_position, written_ts \
                 FROM are_ledger.ledger_entries \
                 WHERE entry_id = $1",
                &[&entry_id],
            )
            .await
            .map_err(|_| LedgerReadError::Unavailable)?
            .ok_or(LedgerReadError::NotFound(entry_id))?;

        Ok(map_row(&row))
    }
}

fn map_row(row: &tokio_postgres::Row) -> LedgerEntry {
    LedgerEntry {
        entry_id: row.get("entry_id"),
        entry_type: row.get("entry_type"),
        agent_id: row.get("agent_id"),
        content: row.get("content"),
        content_type: row.get("content_type"),
        source_id: row.get("source_id"),
        correlation_id: row.get("correlation_id"),
        entry_hash: row.get("entry_hash"),
        hash_version: row.get("hash_version"),
        chain_position: row.get("chain_position"),
        written_ts: row.get("written_ts"),
    }
}
