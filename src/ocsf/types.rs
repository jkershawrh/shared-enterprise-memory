use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const OCSF_CLASS_NETWORK_ACTIVITY: u32 = 4001;
pub const OCSF_CLASS_HTTP_ACTIVITY: u32 = 4002;
pub const OCSF_CLASS_SSH_ACTIVITY: u32 = 4003;
pub const OCSF_CLASS_PROCESS_ACTIVITY: u32 = 1007;
pub const OCSF_CLASS_DETECTION_FINDING: u32 = 2004;
pub const OCSF_CLASS_APP_LIFECYCLE: u32 = 6002;
pub const OCSF_CLASS_CONFIG_STATE_CHANGE: u32 = 5001;

pub fn ocsf_class_name(class_uid: u32) -> Option<&'static str> {
    match class_uid {
        OCSF_CLASS_NETWORK_ACTIVITY => Some("network_activity"),
        OCSF_CLASS_HTTP_ACTIVITY => Some("http_activity"),
        OCSF_CLASS_SSH_ACTIVITY => Some("ssh_activity"),
        OCSF_CLASS_PROCESS_ACTIVITY => Some("process_activity"),
        OCSF_CLASS_DETECTION_FINDING => Some("detection_finding"),
        OCSF_CLASS_APP_LIFECYCLE => Some("app_lifecycle"),
        OCSF_CLASS_CONFIG_STATE_CHANGE => Some("config_state_change"),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcsfEvent {
    pub class_uid: u32,
    #[serde(default)]
    pub class_name: Option<String>,
    #[serde(default)]
    pub category_uid: Option<u32>,
    #[serde(default)]
    pub activity_id: Option<u32>,
    #[serde(default)]
    pub activity_name: Option<String>,
    #[serde(default)]
    pub severity_id: u32,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub status_id: Option<u32>,
    #[serde(default)]
    pub time: Option<i64>,
    #[serde(default)]
    pub metadata: OcsfMetadata,
    #[serde(default)]
    pub actor: Option<OcsfActor>,
    #[serde(default)]
    pub disposition_id: Option<u32>,
    #[serde(default)]
    pub disposition: Option<String>,
    #[serde(default)]
    pub observables: Option<Vec<OcsfObservable>>,
    #[serde(default)]
    pub policy: Option<OcsfPolicy>,
    #[serde(default)]
    pub unmapped: Option<HashMap<String, serde_json::Value>>,
}

impl OcsfEvent {
    pub fn resolved_class_name(&self) -> String {
        if let Some(name) = ocsf_class_name(self.class_uid) {
            return name.to_string();
        }
        if let Some(ref name) = self.class_name {
            return name.to_lowercase();
        }
        format!("unknown_{}", self.class_uid)
    }

    pub fn extract_unmapped_str(&self, key: &str) -> Option<String> {
        self.unmapped
            .as_ref()?
            .get(key)?
            .as_str()
            .map(|s| s.to_string())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OcsfMetadata {
    #[serde(default)]
    pub uid: Option<String>,
    #[serde(default)]
    pub correlation_uid: Option<String>,
    #[serde(default)]
    pub logged_time: Option<i64>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub product: Option<OcsfProduct>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcsfProduct {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub vendor_name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcsfActor {
    #[serde(default)]
    pub user: Option<OcsfUser>,
    #[serde(default)]
    pub process: Option<OcsfProcess>,
    #[serde(default)]
    pub session: Option<OcsfSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcsfUser {
    #[serde(default)]
    pub uid: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub type_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcsfProcess {
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub cmd_line: Option<String>,
    #[serde(default)]
    pub uid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcsfSession {
    #[serde(default)]
    pub uid: Option<String>,
    #[serde(default)]
    pub issuer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcsfObservable {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub type_id: Option<u32>,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcsfPolicy {
    #[serde(default)]
    pub uid: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_network_activity() {
        let json = serde_json::json!({
            "class_uid": 4001,
            "class_name": "Network Activity",
            "severity_id": 1,
            "activity_id": 1,
            "activity_name": "Open",
            "metadata": { "uid": "evt-001" }
        });
        let event: OcsfEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.class_uid, OCSF_CLASS_NETWORK_ACTIVITY);
        assert_eq!(event.resolved_class_name(), "network_activity");
    }

    #[test]
    fn deserializes_detection_finding() {
        let json = serde_json::json!({
            "class_uid": 2004,
            "severity_id": 4,
            "metadata": { "uid": "finding-001", "correlation_uid": "trace-abc" },
            "disposition": "denied",
            "disposition_id": 2,
            "policy": { "uid": "pol-1", "name": "sandbox-default", "version": "3" }
        });
        let event: OcsfEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.resolved_class_name(), "detection_finding");
        assert_eq!(event.metadata.correlation_uid.as_deref(), Some("trace-abc"));
        assert_eq!(event.policy.as_ref().unwrap().uid.as_deref(), Some("pol-1"));
    }

    #[test]
    fn deserializes_with_unmapped_fields() {
        let json = serde_json::json!({
            "class_uid": 4002,
            "severity_id": 2,
            "metadata": {},
            "unmapped": {
                "sandbox_id": "sb-123",
                "trace_id": "trace-456",
                "request_id": "req-789",
                "cluster": "prod-east"
            }
        });
        let event: OcsfEvent = serde_json::from_value(json).unwrap();
        assert_eq!(
            event.extract_unmapped_str("sandbox_id").as_deref(),
            Some("sb-123")
        );
        assert_eq!(
            event.extract_unmapped_str("trace_id").as_deref(),
            Some("trace-456")
        );
        assert_eq!(event.extract_unmapped_str("nonexistent"), None);
    }

    #[test]
    fn unknown_class_uid_deserializes() {
        let json = serde_json::json!({
            "class_uid": 9999,
            "severity_id": 1,
            "class_name": "Custom Event"
        });
        let event: OcsfEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.resolved_class_name(), "custom event");
    }

    #[test]
    fn unknown_class_uid_no_name_fallback() {
        let json = serde_json::json!({
            "class_uid": 9999,
            "severity_id": 0
        });
        let event: OcsfEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.resolved_class_name(), "unknown_9999");
    }

    #[test]
    fn missing_optional_fields_default_to_none() {
        let json = serde_json::json!({
            "class_uid": 1007,
            "severity_id": 0
        });
        let event: OcsfEvent = serde_json::from_value(json).unwrap();
        assert!(event.actor.is_none());
        assert!(event.policy.is_none());
        assert!(event.disposition.is_none());
        assert!(event.unmapped.is_none());
        assert!(event.time.is_none());
        assert!(event.metadata.uid.is_none());
    }

    #[test]
    fn deserializes_full_actor() {
        let json = serde_json::json!({
            "class_uid": 1007,
            "severity_id": 1,
            "actor": {
                "user": { "uid": "1000", "name": "agent-a" },
                "process": { "pid": 42, "name": "python3", "cmd_line": "python3 run.py" },
                "session": { "uid": "sess-1" }
            }
        });
        let event: OcsfEvent = serde_json::from_value(json).unwrap();
        let actor = event.actor.unwrap();
        assert_eq!(
            actor.process.as_ref().unwrap().name.as_deref(),
            Some("python3")
        );
        assert_eq!(
            actor.user.as_ref().unwrap().name.as_deref(),
            Some("agent-a")
        );
    }

    #[test]
    fn all_known_class_uids_resolve() {
        let known = [
            (OCSF_CLASS_NETWORK_ACTIVITY, "network_activity"),
            (OCSF_CLASS_HTTP_ACTIVITY, "http_activity"),
            (OCSF_CLASS_SSH_ACTIVITY, "ssh_activity"),
            (OCSF_CLASS_PROCESS_ACTIVITY, "process_activity"),
            (OCSF_CLASS_DETECTION_FINDING, "detection_finding"),
            (OCSF_CLASS_APP_LIFECYCLE, "app_lifecycle"),
            (OCSF_CLASS_CONFIG_STATE_CHANGE, "config_state_change"),
        ];
        for (uid, expected) in known {
            assert_eq!(ocsf_class_name(uid), Some(expected));
        }
    }
}
