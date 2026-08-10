use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub entry_id: Uuid,
    pub entry_type: String,
    pub agent_id: String,
    pub content: Vec<u8>,
    pub content_type: String,
    pub source_id: String,
    pub correlation_id: Option<String>,
    pub entry_hash: String,
    pub hash_version: String,
    pub chain_position: i64,
    pub written_ts: DateTime<Utc>,
}
