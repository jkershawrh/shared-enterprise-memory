use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::memory_event::Actor;

pub const CONTEXT_BUNDLE_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextBundleV1 {
    pub schema_version: String,
    pub tenant_id: String,
    pub subject_id: String,
    pub generated_at: DateTime<Utc>,
    pub effective_decisions: Vec<DecisionView>,
    pub observations: Vec<MemoryRecordView>,
    pub assertions: Vec<MemoryRecordView>,
    pub dissent: Vec<DissentView>,
    pub evidence: Vec<EvidenceLinkView>,
    pub projection_freshness: ProjectionFreshness,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionView {
    pub decision_id: Uuid,
    pub actor: Actor,
    pub content: serde_json::Value,
    pub effective_at: DateTime<Utc>,
    pub recorded_at: DateTime<Utc>,
    pub is_current: bool,
    pub correlation_id: Option<String>,
    pub has_dissent: bool,
    pub ledger_entry_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryRecordView {
    pub record_id: Uuid,
    pub event_type: String,
    pub actor: Actor,
    pub content: serde_json::Value,
    pub recorded_at: DateTime<Utc>,
    pub claim_state: String,
    pub has_dissent: bool,
    pub ledger_entry_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DissentView {
    pub dissent_id: Uuid,
    pub target_event_id: Uuid,
    pub actor: Actor,
    pub content: serde_json::Value,
    pub recorded_at: DateTime<Utc>,
    pub ledger_entry_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceLinkView {
    pub projected_table: String,
    pub projected_id: Uuid,
    pub ledger_entry_id: Uuid,
    pub entry_hash: String,
    pub chain_position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectionFreshness {
    pub last_processed_at: Option<DateTime<Utc>>,
    pub lag_entries: HashMap<String, i64>,
    pub is_caught_up: bool,
}

impl ContextBundleV1 {
    pub fn empty(tenant_id: String, subject_id: String) -> Self {
        Self {
            schema_version: CONTEXT_BUNDLE_SCHEMA_VERSION.to_string(),
            tenant_id,
            subject_id,
            generated_at: Utc::now(),
            effective_decisions: Vec::new(),
            observations: Vec::new(),
            assertions: Vec::new(),
            dissent: Vec::new(),
            evidence: Vec::new(),
            projection_freshness: ProjectionFreshness {
                last_processed_at: None,
                lag_entries: HashMap::new(),
                is_caught_up: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bundle_serializes() {
        let bundle = ContextBundleV1::empty("acme".to_string(), "incident-42".to_string());
        let json = serde_json::to_string(&bundle).unwrap();
        let restored: ContextBundleV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.schema_version, CONTEXT_BUNDLE_SCHEMA_VERSION);
        assert_eq!(restored.tenant_id, "acme");
        assert!(restored.effective_decisions.is_empty());
        assert!(restored.observations.is_empty());
        assert!(restored.dissent.is_empty());
    }

    #[test]
    fn bundle_with_decisions_round_trips() {
        let mut bundle = ContextBundleV1::empty("acme".to_string(), "inc-1".to_string());
        bundle.effective_decisions.push(DecisionView {
            decision_id: Uuid::new_v4(),
            actor: Actor {
                actor_id: "carol".to_string(),
                actor_type: "human".to_string(),
            },
            content: serde_json::json!({"decision": "apply fix"}),
            effective_at: Utc::now(),
            recorded_at: Utc::now(),
            is_current: true,
            correlation_id: None,
            has_dissent: false,
            ledger_entry_id: Uuid::new_v4(),
        });
        let bytes = serde_json::to_vec(&bundle).unwrap();
        let restored: ContextBundleV1 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.effective_decisions.len(), 1);
        assert!(restored.effective_decisions[0].is_current);
    }
}
