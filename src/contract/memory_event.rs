use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const MEMORY_EVENT_SCHEMA_VERSION: &str = "1.0.0";
pub const MEMORY_ENTRY_TYPE_PREFIX: &str = "memory.";

pub const OBSERVATION_RECORDED: &str = "observation.recorded";
pub const ASSERTION_RECORDED: &str = "assertion.recorded";
pub const DISSENT_RECORDED: &str = "dissent.recorded";
pub const DECISION_RECORDED: &str = "decision.recorded";
pub const DECISION_SUPERSEDED: &str = "decision.superseded";
pub const ACTION_AUTHORIZED: &str = "action.authorized";
pub const ACTION_COMPLETED: &str = "action.completed";
pub const OUTCOME_RECORDED: &str = "outcome.recorded";

pub const VALID_EVENT_TYPES: &[&str] = &[
    OBSERVATION_RECORDED,
    ASSERTION_RECORDED,
    DISSENT_RECORDED,
    DECISION_RECORDED,
    DECISION_SUPERSEDED,
    ACTION_AUTHORIZED,
    ACTION_COMPLETED,
    OUTCOME_RECORDED,
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Actor {
    pub actor_id: String,
    pub actor_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryEventV1 {
    pub event_id: Uuid,
    pub event_type: String,
    pub schema_version: String,
    pub tenant_id: String,
    pub subject_id: String,
    pub actor: Actor,
    pub recorded_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes_event_id: Option<Uuid>,
    pub classification: String,
    pub claim_state: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("validation failed: {0}")]
    ValidationFailed(String),
}

impl MemoryEventV1 {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ContractError> {
        let event: Self = serde_json::from_slice(bytes)?;
        event.validate()?;
        Ok(event)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("MemoryEventV1 is always serializable")
    }

    pub fn ledger_entry_type(&self) -> String {
        format!("{MEMORY_ENTRY_TYPE_PREFIX}{}", self.event_type)
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if self.tenant_id.is_empty() {
            return Err(ContractError::ValidationFailed(
                "tenant_id must not be empty".to_string(),
            ));
        }
        if self.subject_id.is_empty() {
            return Err(ContractError::ValidationFailed(
                "subject_id must not be empty".to_string(),
            ));
        }
        if self.actor.actor_id.is_empty() {
            return Err(ContractError::ValidationFailed(
                "actor.actor_id must not be empty".to_string(),
            ));
        }
        if !VALID_EVENT_TYPES.contains(&self.event_type.as_str()) {
            return Err(ContractError::ValidationFailed(format!(
                "unknown event_type: {}",
                self.event_type
            )));
        }
        if self.schema_version != MEMORY_EVENT_SCHEMA_VERSION {
            return Err(ContractError::ValidationFailed(format!(
                "unsupported schema_version: {}",
                self.schema_version
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_event() -> MemoryEventV1 {
        MemoryEventV1 {
            event_id: Uuid::new_v4(),
            event_type: OBSERVATION_RECORDED.to_string(),
            schema_version: MEMORY_EVENT_SCHEMA_VERSION.to_string(),
            tenant_id: "acme".to_string(),
            subject_id: "incident-42".to_string(),
            actor: Actor {
                actor_id: "alice".to_string(),
                actor_type: "human".to_string(),
            },
            recorded_at: Utc::now(),
            effective_at: None,
            correlation_id: None,
            supersedes_event_id: None,
            classification: "internal".to_string(),
            claim_state: "asserted".to_string(),
            payload: serde_json::json!({"observation": "server was overloaded"}),
        }
    }

    #[test]
    fn json_round_trip() {
        let event = test_event();
        let bytes = event.to_bytes();
        let restored = MemoryEventV1::from_bytes(&bytes).unwrap();
        assert_eq!(event, restored);
    }

    #[test]
    fn ledger_entry_type_has_prefix() {
        let event = test_event();
        assert_eq!(event.ledger_entry_type(), "memory.observation.recorded");
    }

    #[test]
    fn validates_empty_tenant_id() {
        let mut event = test_event();
        event.tenant_id = String::new();
        let err = event.validate().unwrap_err();
        assert!(err.to_string().contains("tenant_id"));
    }

    #[test]
    fn validates_empty_subject_id() {
        let mut event = test_event();
        event.subject_id = String::new();
        let err = event.validate().unwrap_err();
        assert!(err.to_string().contains("subject_id"));
    }

    #[test]
    fn validates_empty_actor_id() {
        let mut event = test_event();
        event.actor.actor_id = String::new();
        let err = event.validate().unwrap_err();
        assert!(err.to_string().contains("actor_id"));
    }

    #[test]
    fn validates_unknown_event_type() {
        let mut event = test_event();
        event.event_type = "unknown.type".to_string();
        let err = event.validate().unwrap_err();
        assert!(err.to_string().contains("unknown event_type"));
    }

    #[test]
    fn validates_unsupported_schema_version() {
        let mut event = test_event();
        event.schema_version = "99.0.0".to_string();
        let err = event.validate().unwrap_err();
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn from_bytes_rejects_non_json() {
        let result = MemoryEventV1::from_bytes(b"not json");
        assert!(result.is_err());
    }

    #[test]
    fn from_bytes_rejects_missing_fields() {
        let json = r#"{"event_id": "00000000-0000-0000-0000-000000000000"}"#;
        let result = MemoryEventV1::from_bytes(json.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn all_event_types_produce_valid_entry_types() {
        for et in VALID_EVENT_TYPES {
            let mut event = test_event();
            event.event_type = et.to_string();
            let entry_type = event.ledger_entry_type();
            assert!(entry_type.starts_with("memory."));
            assert!(entry_type.len() <= 50);
        }
    }
}
