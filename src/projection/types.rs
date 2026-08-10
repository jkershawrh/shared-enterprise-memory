use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionCheckpoint {
    pub entry_type: String,
    pub last_position: i64,
    pub last_entry_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CurrentDecisionRow {
    pub decision_id: Uuid,
    pub tenant_id: String,
    pub subject_id: String,
    pub actor_id: String,
    pub actor_type: String,
    pub decision_content: serde_json::Value,
    pub effective_at: DateTime<Utc>,
    pub recorded_at: DateTime<Utc>,
    pub superseded_by: Option<Uuid>,
    pub is_current: bool,
    pub ledger_entry_id: Uuid,
    pub correlation_id: Option<String>,
    pub classification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionHistoryRow {
    pub history_id: Uuid,
    pub decision_id: Uuid,
    pub tenant_id: String,
    pub subject_id: String,
    pub actor_id: String,
    pub actor_type: String,
    pub decision_content: serde_json::Value,
    pub effective_at: DateTime<Utc>,
    pub recorded_at: DateTime<Utc>,
    pub supersedes_event_id: Option<Uuid>,
    pub claim_state: String,
    pub ledger_entry_id: Uuid,
    pub correlation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryRecordRow {
    pub record_id: Uuid,
    pub tenant_id: String,
    pub subject_id: String,
    pub event_type: String,
    pub actor_id: String,
    pub actor_type: String,
    pub record_content: serde_json::Value,
    pub effective_at: Option<DateTime<Utc>>,
    pub recorded_at: DateTime<Utc>,
    pub claim_state: String,
    pub has_dissent: bool,
    pub correlation_id: Option<String>,
    pub classification: String,
    pub ledger_entry_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DissentRecordRow {
    pub dissent_id: Uuid,
    pub tenant_id: String,
    pub subject_id: String,
    pub target_event_id: Uuid,
    pub actor_id: String,
    pub actor_type: String,
    pub dissent_content: serde_json::Value,
    pub recorded_at: DateTime<Utc>,
    pub ledger_entry_id: Uuid,
    pub correlation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceLinkRow {
    pub link_id: Uuid,
    pub projected_table: String,
    pub projected_id: Uuid,
    pub ledger_entry_id: Uuid,
    pub entry_type: String,
    pub entry_hash: String,
    pub chain_position: i64,
}
