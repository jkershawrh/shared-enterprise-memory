use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use projections::auth::DevAuthProvider;
use projections::context::ContextService;
use projections::contract::memory_event::*;
use projections::ledger::{InMemoryLedgerReader, LedgerEntry};
use projections::projection::{InMemoryProjectionRepository, Projector};

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

fn make_event(event_type: &str, tenant: &str, subject: &str, actor_id: &str) -> MemoryEventV1 {
    MemoryEventV1 {
        event_id: Uuid::new_v4(),
        event_type: event_type.to_string(),
        schema_version: MEMORY_EVENT_SCHEMA_VERSION.to_string(),
        tenant_id: tenant.to_string(),
        subject_id: subject.to_string(),
        actor: projections::contract::Actor {
            actor_id: actor_id.to_string(),
            actor_type: "human".to_string(),
        },
        recorded_at: Utc::now(),
        effective_at: Some(Utc::now()),
        correlation_id: None,
        supersedes_event_id: None,
        classification: "internal".to_string(),
        claim_state: "asserted".to_string(),
        payload: serde_json::json!({}),
    }
}

struct TestStack {
    reader: Arc<InMemoryLedgerReader>,
    #[allow(dead_code)]
    repo: Arc<InMemoryProjectionRepository>,
    projector: Projector<InMemoryLedgerReader, InMemoryProjectionRepository>,
    context: ContextService<InMemoryProjectionRepository, DevAuthProvider>,
}

fn build_open_stack() -> TestStack {
    let reader = Arc::new(InMemoryLedgerReader::default());
    let repo = Arc::new(InMemoryProjectionRepository::default());
    let auth = Arc::new(DevAuthProvider::open());
    let projector = Projector::new(reader.clone(), repo.clone(), 100);
    let context = ContextService::new(repo.clone(), auth);
    TestStack {
        reader,
        repo,
        projector,
        context,
    }
}

fn build_restricted_stack() -> TestStack {
    let reader = Arc::new(InMemoryLedgerReader::default());
    let repo = Arc::new(InMemoryProjectionRepository::default());
    let mut config = HashMap::new();
    config.insert("acme".to_string(), vec!["ops-agent".to_string()]);
    let auth = Arc::new(DevAuthProvider::with_config(config));
    let projector = Projector::new(reader.clone(), repo.clone(), 100);
    let context = ContextService::new(repo.clone(), auth);
    TestStack {
        reader,
        repo,
        projector,
        context,
    }
}

// Scenario 1: Two people remember the same event differently — both preserved
#[tokio::test]
async fn divergent_observations_are_preserved() {
    let stack = build_open_stack();

    let mut alice_obs = make_event(OBSERVATION_RECORDED, "acme", "incident-42", "alice");
    alice_obs.payload = serde_json::json!({"observation": "server was overloaded"});
    insert_memory_event(&stack.reader, &alice_obs, 1).await;

    let mut bob_obs = make_event(OBSERVATION_RECORDED, "acme", "incident-42", "bob");
    bob_obs.payload = serde_json::json!({"observation": "network was congested"});
    insert_memory_event(&stack.reader, &bob_obs, 2).await;

    stack.projector.process_cycle().await.unwrap();

    let bundle = stack
        .context
        .get_context("agent-1", "acme", "incident-42")
        .await
        .unwrap();

    assert_eq!(bundle.observations.len(), 2);
    let actors: Vec<&str> = bundle
        .observations
        .iter()
        .map(|o| o.actor.actor_id.as_str())
        .collect();
    assert!(actors.contains(&"alice"));
    assert!(actors.contains(&"bob"));
    assert!(bundle.evidence.len() >= 2);
}

// Scenario 2: Dissenting interpretation is recorded
#[tokio::test]
async fn dissent_recorded_against_assertion() {
    let stack = build_open_stack();

    let mut assertion = make_event(ASSERTION_RECORDED, "acme", "incident-42", "alice");
    assertion.payload = serde_json::json!({"assertion": "root cause is memory leak"});
    insert_memory_event(&stack.reader, &assertion, 1).await;
    stack.projector.process_cycle().await.unwrap();

    let mut dissent = make_event(DISSENT_RECORDED, "acme", "incident-42", "bob");
    dissent.claim_state = "dissented".to_string();
    dissent.supersedes_event_id = Some(assertion.event_id);
    dissent.payload = serde_json::json!({"reason": "disagree, it's a config issue"});
    insert_memory_event(&stack.reader, &dissent, 1).await;
    stack.projector.process_cycle().await.unwrap();

    let bundle = stack
        .context
        .get_context("agent-1", "acme", "incident-42")
        .await
        .unwrap();

    assert_eq!(bundle.assertions.len(), 1);
    assert!(bundle.assertions[0].has_dissent);

    assert_eq!(bundle.dissent.len(), 1);
    assert_eq!(bundle.dissent[0].target_event_id, assertion.event_id);
    assert_eq!(bundle.dissent[0].actor.actor_id, "bob");
}

// Scenario 3: Decision approved despite dissent — dissent remains visible
#[tokio::test]
async fn decision_despite_dissent() {
    let stack = build_open_stack();

    let mut assertion = make_event(ASSERTION_RECORDED, "acme", "incident-42", "alice");
    assertion.payload = serde_json::json!({"assertion": "root cause is memory leak"});
    insert_memory_event(&stack.reader, &assertion, 1).await;
    stack.projector.process_cycle().await.unwrap();

    let mut dissent = make_event(DISSENT_RECORDED, "acme", "incident-42", "bob");
    dissent.claim_state = "dissented".to_string();
    dissent.supersedes_event_id = Some(assertion.event_id);
    dissent.payload = serde_json::json!({"reason": "disagree"});
    insert_memory_event(&stack.reader, &dissent, 1).await;
    stack.projector.process_cycle().await.unwrap();

    let mut decision = make_event(DECISION_RECORDED, "acme", "incident-42", "carol");
    decision.claim_state = "decided".to_string();
    decision.payload = serde_json::json!({"decision": "apply memory limit fix"});
    insert_memory_event(&stack.reader, &decision, 1).await;
    stack.projector.process_cycle().await.unwrap();

    let bundle = stack
        .context
        .get_context("agent-1", "acme", "incident-42")
        .await
        .unwrap();

    assert_eq!(bundle.effective_decisions.len(), 1);
    assert_eq!(bundle.effective_decisions[0].actor.actor_id, "carol");
    assert_eq!(
        bundle.dissent.len(),
        1,
        "dissent must NOT be erased by decision"
    );
    assert!(bundle.assertions[0].has_dissent);
}

// Scenario 4: Supersession preserves history
#[tokio::test]
async fn supersession_preserves_history() {
    let stack = build_open_stack();

    let mut original = make_event(DECISION_RECORDED, "acme", "incident-42", "carol");
    original.claim_state = "decided".to_string();
    original.payload = serde_json::json!({"decision": "apply memory limit fix"});
    insert_memory_event(&stack.reader, &original, 1).await;
    stack.projector.process_cycle().await.unwrap();

    let mut superseding = make_event(DECISION_SUPERSEDED, "acme", "incident-42", "carol");
    superseding.claim_state = "superseded".to_string();
    superseding.supersedes_event_id = Some(original.event_id);
    superseding.payload = serde_json::json!({"decision": "revert to config-based fix"});
    insert_memory_event(&stack.reader, &superseding, 1).await;
    stack.projector.process_cycle().await.unwrap();

    let bundle = stack
        .context
        .get_context("agent-1", "acme", "incident-42")
        .await
        .unwrap();

    assert_eq!(bundle.effective_decisions.len(), 1);
    assert_eq!(
        bundle.effective_decisions[0].content["decision"],
        "revert to config-based fix"
    );

    let history = stack
        .context
        .get_decision_history("agent-1", "acme", "incident-42")
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
}

// Scenario 5: Zero ledger calls on read path
#[tokio::test]
async fn zero_ledger_calls_on_read_path() {
    let stack = build_open_stack();

    let mut decision = make_event(DECISION_RECORDED, "acme", "incident-42", "carol");
    decision.claim_state = "decided".to_string();
    insert_memory_event(&stack.reader, &decision, 1).await;
    stack.projector.process_cycle().await.unwrap();

    stack.reader.reset_call_count();

    let _bundle = stack
        .context
        .get_context("agent-1", "acme", "incident-42")
        .await
        .unwrap();

    assert_eq!(
        stack.reader.call_count(),
        0,
        "context retrieval must make ZERO ledger calls"
    );
}

// Scenario 6: Unauthorized tenant denied
#[tokio::test]
async fn unauthorized_tenant_denied() {
    let stack = build_restricted_stack();

    let mut decision = make_event(DECISION_RECORDED, "acme", "incident-42", "carol");
    decision.claim_state = "decided".to_string();
    insert_memory_event(&stack.reader, &decision, 1).await;
    stack.projector.process_cycle().await.unwrap();

    let denied = stack
        .context
        .get_context("external-agent", "acme", "incident-42")
        .await;
    assert!(denied.is_err(), "external agent must be denied");

    let allowed = stack
        .context
        .get_context("ops-agent", "acme", "incident-42")
        .await;
    assert!(allowed.is_ok(), "authorized agent must succeed");
    assert_eq!(allowed.unwrap().effective_decisions.len(), 1);
}
