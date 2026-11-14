use std::sync::Arc;

use projections::auth::DevAuthProvider;
use projections::context::ContextService;
use projections::contract::memory_event::OBSERVATION_RECORDED;
use projections::ocsf::adapter::{OcsfAdapter, OcsfAdapterConfig};
use projections::ocsf::admission::AdmissionPolicy;
use projections::ocsf::types::*;
use projections::projection::repository::ProjectionRepository;
use projections::projection::InMemoryProjectionRepository;

fn load_fixture(name: &str) -> OcsfEvent {
    let path = format!(
        "{}/tests/fixtures/ocsf/{}.json",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let data = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {}", path, e));
    serde_json::from_str(&data).unwrap()
}

fn fixture_adapter() -> OcsfAdapter {
    OcsfAdapter::new(
        AdmissionPolicy::default(),
        OcsfAdapterConfig {
            tenant_id: "openshell".to_string(),
            source_label: "openshell-supervisor".to_string(),
            is_fixture: true,
        },
    )
}

fn live_adapter() -> OcsfAdapter {
    OcsfAdapter::new(
        AdmissionPolicy::default(),
        OcsfAdapterConfig {
            tenant_id: "openshell".to_string(),
            source_label: "openshell-supervisor".to_string(),
            is_fixture: false,
        },
    )
}

async fn build_service() -> (
    Arc<InMemoryProjectionRepository>,
    ContextService<InMemoryProjectionRepository, DevAuthProvider>,
) {
    let repo = Arc::new(InMemoryProjectionRepository::default());
    let auth = Arc::new(DevAuthProvider::open());
    let service = ContextService::new(repo.clone(), auth);
    (repo, service)
}

// OS-01: Routine events dropped
#[test]
fn routine_events_are_dropped() {
    let adapter = fixture_adapter();

    let routines = [
        "network_activity_allowed",
        "http_activity_routine",
        "process_activity_start",
        "app_lifecycle_heartbeat",
    ];

    for name in routines {
        let event = load_fixture(name);
        let result = adapter.process(&event).unwrap();
        assert!(
            result.memory_event.is_none(),
            "expected {} to be dropped, got promoted",
            name
        );
    }
}

// OS-01: Consequential denials promoted
#[tokio::test]
async fn consequential_denials_are_promoted() {
    let adapter = fixture_adapter();
    let (repo, service) = build_service().await;

    let denials = ["network_activity_denied", "ssh_activity_denied"];

    for name in denials {
        let event = load_fixture(name);
        let result = adapter.process(&event).unwrap();
        assert!(
            result.memory_event.is_some(),
            "expected {} to be promoted",
            name
        );

        let mem = result.memory_event.unwrap();
        assert_eq!(mem.event_type, OBSERVATION_RECORDED);
        service.write_event("test-agent", mem).await.unwrap();
    }

    let records = repo
        .get_memory_records("openshell", "openshell.sb-suspect-002")
        .await
        .unwrap();
    assert!(
        !records.is_empty(),
        "denied network event should be projected"
    );
}

// OS-01: Detection finding always promoted regardless of severity
#[test]
fn detection_finding_always_promoted() {
    let adapter = fixture_adapter();
    let event = load_fixture("detection_finding");
    assert_eq!(event.severity_id, 1);

    let result = adapter.process(&event).unwrap();
    assert!(
        result.memory_event.is_some(),
        "detection_finding should be promoted even at low severity"
    );
}

// OS-02: Correlation fields preserved through write path
#[tokio::test]
async fn correlation_fields_preserved() {
    let adapter = fixture_adapter();
    let (_repo, service) = build_service().await;

    let event = load_fixture("policy_denial_high_severity");
    let result = adapter.process(&event).unwrap();
    let mem = result.memory_event.unwrap();

    assert_eq!(mem.correlation_id.as_deref(), Some("req-full-corr-001"));

    service
        .write_event("test-agent", mem.clone())
        .await
        .unwrap();

    let bundle = service
        .get_context("test-agent", "openshell", &mem.subject_id)
        .await
        .unwrap();

    assert_eq!(bundle.observations.len(), 1);
    let obs = &bundle.observations[0];
    let content = &obs.content;
    assert_eq!(content["sandbox_id"].as_str(), Some("sb-ml-pipeline-007"));
    assert_eq!(content["cluster"].as_str(), Some("prod-east-gpu"));
    assert_eq!(content["namespace"].as_str(), Some("ml-training"));
    assert_eq!(content["policy_id"].as_str(), Some("pol-egress-strict"));
    assert_eq!(content["policy_revision"].as_str(), Some("42"));
    assert_eq!(content["policy_hash"].as_str(), Some("sha256:42deadbeef42"));
    assert_eq!(content["disposition"].as_str(), Some("denied"));
}

// Duplicate OCSF delivery is idempotent
#[tokio::test]
async fn duplicate_ocsf_delivery_is_idempotent() {
    let adapter = fixture_adapter();
    let (repo, service) = build_service().await;

    let event = load_fixture("network_activity_denied");

    let r1 = adapter.process(&event).unwrap();
    let r2 = adapter.process(&event).unwrap();

    let mem1 = r1.memory_event.unwrap();
    let mem2 = r2.memory_event.unwrap();
    assert_eq!(mem1.event_id, mem2.event_id);

    service.write_event("test-agent", mem1).await.unwrap();
    service.write_event("test-agent", mem2).await.unwrap();

    let records = repo
        .get_memory_records("openshell", "openshell.sb-suspect-002")
        .await
        .unwrap();
    assert_eq!(
        records.len(),
        1,
        "duplicate delivery should result in single record"
    );
}

// OS-03: Fixture evidence labeled separately from live evidence
#[test]
fn fixture_vs_live_labeling() {
    let fixture_adapter = fixture_adapter();
    let live = live_adapter();

    let event = load_fixture("detection_finding");

    let fixture_result = fixture_adapter.process(&event).unwrap();
    let live_result = live.process(&event).unwrap();

    assert_eq!(
        fixture_result.memory_event.unwrap().classification,
        "ocsf-fixture"
    );
    assert_eq!(live_result.memory_event.unwrap().classification, "ocsf");
}

// Admission policy is configurable
#[test]
fn custom_policy_promotes_normally_dropped() {
    let custom_policy = AdmissionPolicy {
        always_promote_class_uids: vec![OCSF_CLASS_PROCESS_ACTIVITY],
        ..AdmissionPolicy::default()
    };
    let adapter = OcsfAdapter::new(custom_policy, OcsfAdapterConfig::default());

    let event = load_fixture("process_activity_start");
    let result = adapter.process(&event).unwrap();
    assert!(
        result.memory_event.is_some(),
        "custom policy should promote process_activity"
    );
}

// All 7 OCSF class types produce valid MemoryEventV1
#[test]
fn all_class_types_produce_valid_events() {
    let adapter = OcsfAdapter::new(AdmissionPolicy::allow_all(), OcsfAdapterConfig::default());

    let fixtures = [
        "network_activity_allowed",
        "network_activity_denied",
        "http_activity_routine",
        "ssh_activity_denied",
        "process_activity_start",
        "detection_finding",
        "config_state_change",
        "app_lifecycle_heartbeat",
        "policy_denial_high_severity",
    ];

    for name in fixtures {
        let event = load_fixture(name);
        let result = adapter.process(&event).unwrap();
        let mem = result.memory_event.unwrap();
        assert!(
            mem.validate().is_ok(),
            "fixture {} produced invalid MemoryEventV1: {:?}",
            name,
            mem.validate()
        );
    }
}

// Policy revision visible in projected observations
#[tokio::test]
async fn policy_revision_visible_in_projection() {
    let adapter = fixture_adapter();
    let (repo, service) = build_service().await;

    let event = load_fixture("config_state_change");
    let result = adapter.process(&event).unwrap();
    let mem = result.memory_event.unwrap();

    service
        .write_event("test-agent", mem.clone())
        .await
        .unwrap();

    let records = repo
        .get_memory_records("openshell", &mem.subject_id)
        .await
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].record_content["policy_id"].as_str(),
        Some("pol-global-001")
    );
    assert_eq!(
        records[0].record_content["policy_revision"].as_str(),
        Some("7")
    );
    assert_eq!(
        records[0].record_content["policy_hash"].as_str(),
        Some("sha256:newpolicyhash")
    );
}

// End-to-end: OCSF denial → write → context bundle
#[tokio::test]
async fn end_to_end_ocsf_to_context_bundle() {
    let adapter = fixture_adapter();
    let (_repo, service) = build_service().await;

    let event = load_fixture("policy_denial_high_severity");
    let result = adapter.process(&event).unwrap();

    assert!(result.verdict.is_promoted());
    assert_eq!(result.ocsf_class_name, "network_activity");

    let mem = result.memory_event.unwrap();
    assert_eq!(mem.event_type, OBSERVATION_RECORDED);
    assert_eq!(mem.tenant_id, "openshell");
    assert_eq!(mem.actor.actor_type, "system");
    assert_eq!(mem.classification, "ocsf-fixture");
    assert_eq!(mem.correlation_id.as_deref(), Some("req-full-corr-001"));

    service
        .write_event("test-agent", mem.clone())
        .await
        .unwrap();

    let bundle = service
        .get_context("test-agent", "openshell", &mem.subject_id)
        .await
        .unwrap();

    assert_eq!(bundle.tenant_id, "openshell");
    assert_eq!(bundle.observations.len(), 1);

    let obs = &bundle.observations[0];
    assert_eq!(obs.event_type, OBSERVATION_RECORDED);
    assert_eq!(obs.claim_state, "asserted");
    assert!(obs.content.get("ocsf_raw").is_some());
    assert_eq!(obs.content["ocsf_class_uid"].as_u64(), Some(4001));
}
