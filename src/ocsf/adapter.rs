use chrono::{DateTime, TimeZone, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::contract::memory_event::*;

use super::admission::{AdmissionPolicy, AdmissionVerdict};
use super::types::OcsfEvent;

const OCSF_UUID_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6f, 0x63, 0x73, 0x66, 0x2d, 0x70, 0x72, 0x6f, 0x6a, 0x65, 0x63, 0x74, 0x69, 0x6f, 0x6e, 0x73,
]);

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("transform failed: {0}")]
    TransformFailed(String),
}

#[derive(Debug, Clone)]
pub struct OcsfAdapterConfig {
    pub tenant_id: String,
    pub source_label: String,
    pub is_fixture: bool,
}

impl Default for OcsfAdapterConfig {
    fn default() -> Self {
        Self {
            tenant_id: "openshell".to_string(),
            source_label: "openshell-supervisor".to_string(),
            is_fixture: true,
        }
    }
}

#[derive(Debug)]
pub struct OcsfAdmissionResult {
    pub verdict: AdmissionVerdict,
    pub ocsf_class_name: String,
    pub memory_event: Option<MemoryEventV1>,
}

pub struct OcsfAdapter {
    policy: AdmissionPolicy,
    config: OcsfAdapterConfig,
}

impl OcsfAdapter {
    pub fn new(policy: AdmissionPolicy, config: OcsfAdapterConfig) -> Self {
        Self { policy, config }
    }

    pub fn process(&self, event: &OcsfEvent) -> Result<OcsfAdmissionResult, AdapterError> {
        let class_name = event.resolved_class_name();
        let verdict = self.policy.evaluate(event);

        if !verdict.is_promoted() {
            return Ok(OcsfAdmissionResult {
                verdict,
                ocsf_class_name: class_name,
                memory_event: None,
            });
        }

        let memory_event = self.transform(event, verdict.reason())?;

        Ok(OcsfAdmissionResult {
            verdict,
            ocsf_class_name: class_name,
            memory_event: Some(memory_event),
        })
    }

    fn transform(
        &self,
        event: &OcsfEvent,
        admission_reason: &str,
    ) -> Result<MemoryEventV1, AdapterError> {
        let event_id = self.derive_event_id(event);
        let class_name = event.resolved_class_name();
        let subject_id = self.derive_subject_id(event, &class_name);
        let actor = self.extract_actor(event);
        let correlation_id = self.extract_correlation_id(event);
        let recorded_at = self.extract_timestamp(event);

        let classification = if self.config.is_fixture {
            "ocsf-fixture".to_string()
        } else {
            "ocsf".to_string()
        };

        let payload = self.build_payload(event, &class_name, admission_reason);

        Ok(MemoryEventV1 {
            event_id,
            event_type: OBSERVATION_RECORDED.to_string(),
            schema_version: MEMORY_EVENT_SCHEMA_VERSION.to_string(),
            tenant_id: self.config.tenant_id.clone(),
            subject_id,
            actor,
            recorded_at,
            effective_at: None,
            correlation_id,
            supersedes_event_id: None,
            classification,
            claim_state: "asserted".to_string(),
            payload,
        })
    }

    fn derive_event_id(&self, event: &OcsfEvent) -> Uuid {
        match &event.metadata.uid {
            Some(uid) if !uid.is_empty() => Uuid::new_v5(&OCSF_UUID_NAMESPACE, uid.as_bytes()),
            _ => Uuid::new_v4(),
        }
    }

    fn derive_subject_id(&self, event: &OcsfEvent, class_name: &str) -> String {
        if let Some(sandbox_id) = event.extract_unmapped_str("sandbox_id") {
            return format!("openshell.{}", sandbox_id);
        }
        if let Some(ref uid) = event.metadata.uid {
            return format!("openshell.{}.{}", class_name, uid);
        }
        format!("openshell.{}", class_name)
    }

    fn extract_actor(&self, event: &OcsfEvent) -> Actor {
        if let Some(ref actor) = event.actor {
            if let Some(ref process) = actor.process {
                if let Some(ref name) = process.name {
                    return Actor {
                        actor_id: name.clone(),
                        actor_type: "system".to_string(),
                    };
                }
            }
            if let Some(ref user) = actor.user {
                if let Some(ref name) = user.name {
                    return Actor {
                        actor_id: name.clone(),
                        actor_type: "system".to_string(),
                    };
                }
            }
        }
        Actor {
            actor_id: self.config.source_label.clone(),
            actor_type: "system".to_string(),
        }
    }

    fn extract_correlation_id(&self, event: &OcsfEvent) -> Option<String> {
        if let Some(id) = event.extract_unmapped_str("request_id") {
            return Some(id);
        }
        if let Some(id) = event.extract_unmapped_str("trace_id") {
            return Some(id);
        }
        if let Some(id) = event.extract_unmapped_str("traceparent") {
            return Some(id);
        }
        if let Some(id) = event.extract_unmapped_str("correlation_id") {
            return Some(id);
        }
        event.metadata.correlation_uid.clone()
    }

    fn extract_timestamp(&self, event: &OcsfEvent) -> DateTime<Utc> {
        event
            .time
            .and_then(|ms| Utc.timestamp_millis_opt(ms).single())
            .unwrap_or_else(Utc::now)
    }

    fn build_payload(
        &self,
        event: &OcsfEvent,
        class_name: &str,
        admission_reason: &str,
    ) -> serde_json::Value {
        let mut payload = serde_json::json!({
            "ocsf_class_uid": event.class_uid,
            "ocsf_class_name": class_name,
            "ocsf_severity_id": event.severity_id,
            "source": &self.config.source_label,
            "admission_reason": admission_reason,
        });

        let obj = payload.as_object_mut().unwrap();

        if let Some(ref activity) = event.activity_name {
            obj.insert(
                "ocsf_activity_name".to_string(),
                serde_json::json!(activity),
            );
        }
        if let Some(aid) = event.activity_id {
            obj.insert("ocsf_activity_id".to_string(), serde_json::json!(aid));
        }
        if let Some(ref disposition) = event.disposition {
            obj.insert("disposition".to_string(), serde_json::json!(disposition));
        }
        if let Some(did) = event.disposition_id {
            obj.insert("disposition_id".to_string(), serde_json::json!(did));
        }
        if let Some(sandbox_id) = event.extract_unmapped_str("sandbox_id") {
            obj.insert("sandbox_id".to_string(), serde_json::json!(sandbox_id));
        }
        if let Some(cluster) = event.extract_unmapped_str("cluster") {
            obj.insert("cluster".to_string(), serde_json::json!(cluster));
        }
        if let Some(namespace) = event.extract_unmapped_str("namespace") {
            obj.insert("namespace".to_string(), serde_json::json!(namespace));
        }
        if let Some(ref policy) = event.policy {
            if let Some(ref uid) = policy.uid {
                obj.insert("policy_id".to_string(), serde_json::json!(uid));
            }
            if let Some(ref version) = policy.version {
                obj.insert("policy_revision".to_string(), serde_json::json!(version));
            }
        }
        if let Some(hash) = event.extract_unmapped_str("policy_hash") {
            obj.insert("policy_hash".to_string(), serde_json::json!(hash));
        }

        obj.insert(
            "ocsf_raw".to_string(),
            serde_json::to_value(event).unwrap_or(serde_json::Value::Null),
        );

        payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_adapter() -> OcsfAdapter {
        OcsfAdapter::new(AdmissionPolicy::default(), OcsfAdapterConfig::default())
    }

    fn allow_all_adapter(is_fixture: bool) -> OcsfAdapter {
        OcsfAdapter::new(
            AdmissionPolicy::allow_all(),
            OcsfAdapterConfig {
                is_fixture,
                ..OcsfAdapterConfig::default()
            },
        )
    }

    fn denied_event(class_uid: u32) -> OcsfEvent {
        serde_json::from_value(serde_json::json!({
            "class_uid": class_uid,
            "severity_id": 3,
            "disposition": "denied",
            "disposition_id": 2,
            "metadata": { "uid": "evt-denied-001" },
            "policy": { "uid": "pol-1", "version": "5" },
            "unmapped": {
                "sandbox_id": "sb-abc",
                "cluster": "prod-east",
                "namespace": "agents",
                "policy_hash": "sha256:deadbeef",
                "request_id": "req-123"
            }
        }))
        .unwrap()
    }

    #[test]
    fn dropped_event_produces_no_memory_event() {
        let adapter = default_adapter();
        let event: OcsfEvent = serde_json::from_value(serde_json::json!({
            "class_uid": 4001,
            "severity_id": 1,
            "disposition": "allowed",
            "metadata": {}
        }))
        .unwrap();

        let result = adapter.process(&event).unwrap();
        assert!(!result.verdict.is_promoted());
        assert!(result.memory_event.is_none());
    }

    #[test]
    fn promoted_denial_produces_memory_event() {
        let adapter = default_adapter();
        let event = denied_event(4001);
        let result = adapter.process(&event).unwrap();

        assert!(result.verdict.is_promoted());
        let mem = result.memory_event.unwrap();
        assert_eq!(mem.event_type, OBSERVATION_RECORDED);
        assert_eq!(mem.tenant_id, "openshell");
        assert_eq!(mem.claim_state, "asserted");
        assert!(mem.validate().is_ok());
    }

    #[test]
    fn correlation_id_extracted_from_request_id() {
        let adapter = allow_all_adapter(false);
        let event: OcsfEvent = serde_json::from_value(serde_json::json!({
            "class_uid": 4001,
            "severity_id": 0,
            "metadata": { "uid": "e1", "correlation_uid": "corr-last" },
            "unmapped": { "request_id": "req-first", "trace_id": "trace-second" }
        }))
        .unwrap();

        let result = adapter.process(&event).unwrap();
        let mem = result.memory_event.unwrap();
        assert_eq!(mem.correlation_id.as_deref(), Some("req-first"));
    }

    #[test]
    fn correlation_falls_through_to_trace_id() {
        let adapter = allow_all_adapter(false);
        let event: OcsfEvent = serde_json::from_value(serde_json::json!({
            "class_uid": 4001,
            "severity_id": 0,
            "metadata": { "uid": "e2" },
            "unmapped": { "trace_id": "trace-456" }
        }))
        .unwrap();

        let result = adapter.process(&event).unwrap();
        let mem = result.memory_event.unwrap();
        assert_eq!(mem.correlation_id.as_deref(), Some("trace-456"));
    }

    #[test]
    fn correlation_falls_through_to_metadata() {
        let adapter = allow_all_adapter(false);
        let event: OcsfEvent = serde_json::from_value(serde_json::json!({
            "class_uid": 4001,
            "severity_id": 0,
            "metadata": { "uid": "e3", "correlation_uid": "meta-corr" }
        }))
        .unwrap();

        let result = adapter.process(&event).unwrap();
        let mem = result.memory_event.unwrap();
        assert_eq!(mem.correlation_id.as_deref(), Some("meta-corr"));
    }

    #[test]
    fn sandbox_id_preserved_in_payload() {
        let adapter = default_adapter();
        let event = denied_event(4001);
        let result = adapter.process(&event).unwrap();
        let payload = &result.memory_event.unwrap().payload;
        assert_eq!(payload["sandbox_id"].as_str(), Some("sb-abc"));
    }

    #[test]
    fn policy_fields_preserved_in_payload() {
        let adapter = default_adapter();
        let event = denied_event(4001);
        let result = adapter.process(&event).unwrap();
        let payload = &result.memory_event.unwrap().payload;
        assert_eq!(payload["policy_id"].as_str(), Some("pol-1"));
        assert_eq!(payload["policy_revision"].as_str(), Some("5"));
        assert_eq!(payload["policy_hash"].as_str(), Some("sha256:deadbeef"));
    }

    #[test]
    fn cluster_and_namespace_in_payload() {
        let adapter = default_adapter();
        let event = denied_event(4001);
        let result = adapter.process(&event).unwrap();
        let payload = &result.memory_event.unwrap().payload;
        assert_eq!(payload["cluster"].as_str(), Some("prod-east"));
        assert_eq!(payload["namespace"].as_str(), Some("agents"));
    }

    #[test]
    fn fixture_classification_when_is_fixture() {
        let adapter = allow_all_adapter(true);
        let event = denied_event(4001);
        let result = adapter.process(&event).unwrap();
        assert_eq!(result.memory_event.unwrap().classification, "ocsf-fixture");
    }

    #[test]
    fn live_classification_when_not_fixture() {
        let adapter = allow_all_adapter(false);
        let event = denied_event(4001);
        let result = adapter.process(&event).unwrap();
        assert_eq!(result.memory_event.unwrap().classification, "ocsf");
    }

    #[test]
    fn deterministic_event_id_from_metadata_uid() {
        let adapter = allow_all_adapter(false);
        let event: OcsfEvent = serde_json::from_value(serde_json::json!({
            "class_uid": 4001,
            "severity_id": 0,
            "metadata": { "uid": "stable-id-abc" }
        }))
        .unwrap();

        let r1 = adapter.process(&event).unwrap();
        let r2 = adapter.process(&event).unwrap();
        assert_eq!(
            r1.memory_event.unwrap().event_id,
            r2.memory_event.unwrap().event_id
        );
    }

    #[test]
    fn subject_id_uses_sandbox_id_when_present() {
        let adapter = default_adapter();
        let event = denied_event(4001);
        let result = adapter.process(&event).unwrap();
        assert_eq!(result.memory_event.unwrap().subject_id, "openshell.sb-abc");
    }

    #[test]
    fn subject_id_uses_class_and_uid_without_sandbox() {
        let adapter = allow_all_adapter(false);
        let event: OcsfEvent = serde_json::from_value(serde_json::json!({
            "class_uid": 2004,
            "severity_id": 0,
            "metadata": { "uid": "finding-x" }
        }))
        .unwrap();

        let result = adapter.process(&event).unwrap();
        assert_eq!(
            result.memory_event.unwrap().subject_id,
            "openshell.detection_finding.finding-x"
        );
    }

    #[test]
    fn actor_from_process_name() {
        let adapter = allow_all_adapter(false);
        let event: OcsfEvent = serde_json::from_value(serde_json::json!({
            "class_uid": 1007,
            "severity_id": 0,
            "metadata": { "uid": "p1" },
            "actor": { "process": { "name": "python3" } }
        }))
        .unwrap();

        let result = adapter.process(&event).unwrap();
        let mem = result.memory_event.unwrap();
        assert_eq!(mem.actor.actor_id, "python3");
        assert_eq!(mem.actor.actor_type, "system");
    }

    #[test]
    fn actor_defaults_to_source_label() {
        let adapter = allow_all_adapter(false);
        let event: OcsfEvent = serde_json::from_value(serde_json::json!({
            "class_uid": 4001,
            "severity_id": 0,
            "metadata": { "uid": "x" }
        }))
        .unwrap();

        let result = adapter.process(&event).unwrap();
        assert_eq!(
            result.memory_event.unwrap().actor.actor_id,
            "openshell-supervisor"
        );
    }

    #[test]
    fn all_seven_class_types_transform_validly() {
        let adapter = allow_all_adapter(false);
        let classes = [4001, 4002, 4003, 1007, 2004, 6002, 5001];
        for class_uid in classes {
            let event: OcsfEvent = serde_json::from_value(serde_json::json!({
                "class_uid": class_uid,
                "severity_id": 0,
                "metadata": { "uid": format!("test-{}", class_uid) }
            }))
            .unwrap();
            let result = adapter.process(&event).unwrap();
            let mem = result.memory_event.unwrap();
            assert!(mem.validate().is_ok(), "failed for class_uid {}", class_uid);
        }
    }

    #[test]
    fn raw_ocsf_preserved_in_payload() {
        let adapter = allow_all_adapter(false);
        let event: OcsfEvent = serde_json::from_value(serde_json::json!({
            "class_uid": 2004,
            "severity_id": 4,
            "metadata": { "uid": "f1" },
            "disposition": "denied"
        }))
        .unwrap();

        let result = adapter.process(&event).unwrap();
        let payload = &result.memory_event.unwrap().payload;
        assert!(payload.get("ocsf_raw").is_some());
        assert_eq!(payload["ocsf_raw"]["class_uid"].as_u64(), Some(2004));
    }

    #[test]
    fn admission_reason_in_payload() {
        let adapter = default_adapter();
        let event = denied_event(4001);
        let result = adapter.process(&event).unwrap();
        let payload = &result.memory_event.unwrap().payload;
        assert!(payload["admission_reason"]
            .as_str()
            .unwrap()
            .contains("denied"));
    }
}
