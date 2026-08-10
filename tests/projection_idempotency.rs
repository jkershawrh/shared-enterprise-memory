use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use projections::contract::memory_event::*;
use projections::ledger::{InMemoryLedgerReader, LedgerEntry};
use projections::projection::{InMemoryProjectionRepository, ProjectionRepository, Projector};

async fn insert_memory_event(reader: &InMemoryLedgerReader, event: &MemoryEventV1, position: i64) {
    let entry = LedgerEntry {
        entry_id: Uuid::new_v4(),
        entry_type: event.ledger_entry_type(),
        agent_id: event.actor.actor_id.clone(),
        content: event.to_bytes(),
        content_type: "application/json".to_string(),
        source_id: "test".to_string(),
        correlation_id: event.correlation_id.clone(),
        entry_hash: format!("hash-{}-{position}", event.event_type),
        hash_version: "V3".to_string(),
        chain_position: position,
        written_ts: Utc::now(),
    };
    reader.insert_entry(entry).await;
}

fn make_event(event_type: &str) -> MemoryEventV1 {
    MemoryEventV1 {
        event_id: Uuid::new_v4(),
        event_type: event_type.to_string(),
        schema_version: MEMORY_EVENT_SCHEMA_VERSION.to_string(),
        tenant_id: "acme".to_string(),
        subject_id: "inc-1".to_string(),
        actor: projections::contract::Actor {
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

#[tokio::test]
async fn replay_produces_identical_projection_state() {
    let reader = Arc::new(InMemoryLedgerReader::default());

    let obs = make_event(OBSERVATION_RECORDED);
    insert_memory_event(&reader, &obs, 1).await;

    let mut dec = make_event(DECISION_RECORDED);
    dec.claim_state = "decided".to_string();
    insert_memory_event(&reader, &dec, 1).await;

    // First run
    let repo1 = Arc::new(InMemoryProjectionRepository::default());
    let p1 = Projector::new(reader.clone(), repo1.clone(), 100);
    p1.process_cycle().await.unwrap();

    let records_1 = repo1.get_memory_records("acme", "inc-1").await.unwrap();
    let decisions_1 = repo1.get_current_decisions("acme", "inc-1").await.unwrap();

    // Second run (fresh repo = full replay)
    let repo2 = Arc::new(InMemoryProjectionRepository::default());
    let p2 = Projector::new(reader.clone(), repo2.clone(), 100);
    p2.process_cycle().await.unwrap();

    let records_2 = repo2.get_memory_records("acme", "inc-1").await.unwrap();
    let decisions_2 = repo2.get_current_decisions("acme", "inc-1").await.unwrap();

    assert_eq!(records_1.len(), records_2.len());
    assert_eq!(decisions_1.len(), decisions_2.len());
    assert_eq!(records_1[0].record_id, records_2[0].record_id);
    assert_eq!(decisions_1[0].decision_id, decisions_2[0].decision_id);
}

#[tokio::test]
async fn duplicate_delivery_does_not_create_duplicates() {
    let reader = Arc::new(InMemoryLedgerReader::default());
    let repo = Arc::new(InMemoryProjectionRepository::default());

    let obs = make_event(OBSERVATION_RECORDED);
    insert_memory_event(&reader, &obs, 1).await;
    // Insert same event content at a different chain position (simulating duplicate delivery)
    insert_memory_event(&reader, &obs, 2).await;

    let projector = Projector::new(reader, repo.clone(), 100);
    projector.process_cycle().await.unwrap();

    let records = repo.get_memory_records("acme", "inc-1").await.unwrap();
    assert_eq!(
        records.len(),
        1,
        "duplicate event must not create duplicate row"
    );
}

#[tokio::test]
async fn malformed_content_is_skipped_projector_continues() {
    let reader = Arc::new(InMemoryLedgerReader::default());
    let repo = Arc::new(InMemoryProjectionRepository::default());

    // Insert malformed entry
    let bad_entry = LedgerEntry {
        entry_id: Uuid::new_v4(),
        entry_type: "memory.observation.recorded".to_string(),
        agent_id: "agent".to_string(),
        content: b"{{{{not json".to_vec(),
        content_type: "application/json".to_string(),
        source_id: "test".to_string(),
        correlation_id: None,
        entry_hash: "bad-hash".to_string(),
        hash_version: "V3".to_string(),
        chain_position: 1,
        written_ts: Utc::now(),
    };
    reader.insert_entry(bad_entry).await;

    // Insert valid entry after
    let good = make_event(OBSERVATION_RECORDED);
    insert_memory_event(&reader, &good, 2).await;

    let projector = Projector::new(reader, repo.clone(), 100);
    projector.process_cycle().await.unwrap();

    let records = repo.get_memory_records("acme", "inc-1").await.unwrap();
    assert_eq!(
        records.len(),
        1,
        "valid event should be projected despite earlier malformed entry"
    );
}
