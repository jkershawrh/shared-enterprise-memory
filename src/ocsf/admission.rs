use serde::{Deserialize, Serialize};

use super::types::*;

#[derive(Debug, Clone)]
pub enum AdmissionVerdict {
    Promote { reason: String },
    Drop { reason: String },
}

impl AdmissionVerdict {
    pub fn is_promoted(&self) -> bool {
        matches!(self, AdmissionVerdict::Promote { .. })
    }

    pub fn reason(&self) -> &str {
        match self {
            AdmissionVerdict::Promote { reason } | AdmissionVerdict::Drop { reason } => reason,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionPolicy {
    #[serde(default = "default_min_severity")]
    pub min_severity_id: u32,
    #[serde(default = "default_promote_classes")]
    pub always_promote_class_uids: Vec<u32>,
    #[serde(default = "default_promote_dispositions")]
    pub always_promote_dispositions: Vec<String>,
    #[serde(default)]
    pub always_drop_class_uids: Vec<u32>,
    #[serde(default = "default_drop_activities")]
    pub drop_activity_names: Vec<String>,
}

fn default_min_severity() -> u32 {
    3
}

fn default_promote_classes() -> Vec<u32> {
    vec![OCSF_CLASS_DETECTION_FINDING]
}

fn default_promote_dispositions() -> Vec<String> {
    vec!["denied".to_string(), "blocked".to_string()]
}

fn default_drop_activities() -> Vec<String> {
    vec!["heartbeat".to_string()]
}

impl Default for AdmissionPolicy {
    fn default() -> Self {
        Self {
            min_severity_id: default_min_severity(),
            always_promote_class_uids: default_promote_classes(),
            always_promote_dispositions: default_promote_dispositions(),
            always_drop_class_uids: Vec::new(),
            drop_activity_names: default_drop_activities(),
        }
    }
}

impl AdmissionPolicy {
    pub fn allow_all() -> Self {
        Self {
            min_severity_id: 0,
            always_promote_class_uids: Vec::new(),
            always_promote_dispositions: Vec::new(),
            always_drop_class_uids: Vec::new(),
            drop_activity_names: Vec::new(),
        }
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn evaluate(&self, event: &OcsfEvent) -> AdmissionVerdict {
        if self.always_drop_class_uids.contains(&event.class_uid) {
            return AdmissionVerdict::Drop {
                reason: format!("class {} in always-drop list", event.resolved_class_name()),
            };
        }

        if let Some(ref activity) = event.activity_name {
            let lower = activity.to_lowercase();
            if self.drop_activity_names.iter().any(|d| d == &lower) {
                return AdmissionVerdict::Drop {
                    reason: format!("activity '{}' in drop list", activity),
                };
            }
        }

        if let Some(ref disposition) = event.disposition {
            let lower = disposition.to_lowercase();
            if self
                .always_promote_dispositions
                .iter()
                .any(|d| d.to_lowercase() == lower)
            {
                return AdmissionVerdict::Promote {
                    reason: format!("{} action", disposition),
                };
            }
        }

        if self.always_promote_class_uids.contains(&event.class_uid) {
            return AdmissionVerdict::Promote {
                reason: format!("class {} always promoted", event.resolved_class_name()),
            };
        }

        if event.severity_id >= self.min_severity_id {
            return AdmissionVerdict::Promote {
                reason: format!(
                    "severity {} >= threshold {}",
                    event.severity_id, self.min_severity_id
                ),
            };
        }

        AdmissionVerdict::Drop {
            reason: format!(
                "severity {} below threshold {}, no matching rules",
                event.severity_id, self.min_severity_id
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_with(class_uid: u32, severity_id: u32) -> OcsfEvent {
        serde_json::from_value(serde_json::json!({
            "class_uid": class_uid,
            "severity_id": severity_id,
            "metadata": {}
        }))
        .unwrap()
    }

    fn event_with_disposition(class_uid: u32, severity_id: u32, disposition: &str) -> OcsfEvent {
        serde_json::from_value(serde_json::json!({
            "class_uid": class_uid,
            "severity_id": severity_id,
            "disposition": disposition,
            "metadata": {}
        }))
        .unwrap()
    }

    fn event_with_activity(class_uid: u32, severity_id: u32, activity: &str) -> OcsfEvent {
        serde_json::from_value(serde_json::json!({
            "class_uid": class_uid,
            "severity_id": severity_id,
            "activity_name": activity,
            "metadata": {}
        }))
        .unwrap()
    }

    #[test]
    fn routine_allowed_network_dropped() {
        let policy = AdmissionPolicy::default();
        let event = event_with_disposition(OCSF_CLASS_NETWORK_ACTIVITY, 1, "allowed");
        let verdict = policy.evaluate(&event);
        assert!(!verdict.is_promoted());
    }

    #[test]
    fn routine_process_start_dropped() {
        let policy = AdmissionPolicy::default();
        let event = event_with(OCSF_CLASS_PROCESS_ACTIVITY, 1);
        let verdict = policy.evaluate(&event);
        assert!(!verdict.is_promoted());
    }

    #[test]
    fn denied_action_promoted() {
        let policy = AdmissionPolicy::default();
        let event = event_with_disposition(OCSF_CLASS_NETWORK_ACTIVITY, 1, "denied");
        let verdict = policy.evaluate(&event);
        assert!(verdict.is_promoted());
        assert!(verdict.reason().contains("denied"));
    }

    #[test]
    fn blocked_action_promoted() {
        let policy = AdmissionPolicy::default();
        let event = event_with_disposition(OCSF_CLASS_HTTP_ACTIVITY, 1, "Blocked");
        let verdict = policy.evaluate(&event);
        assert!(verdict.is_promoted());
    }

    #[test]
    fn detection_finding_promoted_regardless_of_severity() {
        let policy = AdmissionPolicy::default();
        let event = event_with(OCSF_CLASS_DETECTION_FINDING, 0);
        let verdict = policy.evaluate(&event);
        assert!(verdict.is_promoted());
        assert!(verdict.reason().contains("always promoted"));
    }

    #[test]
    fn high_severity_promoted() {
        let policy = AdmissionPolicy::default();
        let event = event_with(OCSF_CLASS_PROCESS_ACTIVITY, 4);
        let verdict = policy.evaluate(&event);
        assert!(verdict.is_promoted());
        assert!(verdict.reason().contains("severity"));
    }

    #[test]
    fn heartbeat_dropped() {
        let policy = AdmissionPolicy::default();
        let event = event_with_activity(OCSF_CLASS_APP_LIFECYCLE, 1, "heartbeat");
        let verdict = policy.evaluate(&event);
        assert!(!verdict.is_promoted());
        assert!(verdict.reason().contains("drop list"));
    }

    #[test]
    fn always_drop_class_overrides_severity() {
        let policy = AdmissionPolicy {
            always_drop_class_uids: vec![OCSF_CLASS_PROCESS_ACTIVITY],
            ..AdmissionPolicy::default()
        };
        let event = event_with(OCSF_CLASS_PROCESS_ACTIVITY, 5);
        let verdict = policy.evaluate(&event);
        assert!(!verdict.is_promoted());
    }

    #[test]
    fn allow_all_promotes_everything() {
        let policy = AdmissionPolicy::allow_all();
        let event = event_with(OCSF_CLASS_PROCESS_ACTIVITY, 0);
        let verdict = policy.evaluate(&event);
        assert!(verdict.is_promoted());
    }

    #[test]
    fn custom_policy_from_json() {
        let json = r#"{"min_severity_id": 1, "always_promote_class_uids": [1007]}"#;
        let policy = AdmissionPolicy::from_json(json).unwrap();
        assert_eq!(policy.min_severity_id, 1);
        assert!(policy.always_promote_class_uids.contains(&1007));
    }

    #[test]
    fn default_policy_matches_spec() {
        let policy = AdmissionPolicy::default();
        assert_eq!(policy.min_severity_id, 3);
        assert!(policy
            .always_promote_class_uids
            .contains(&OCSF_CLASS_DETECTION_FINDING));
        assert!(policy
            .always_promote_dispositions
            .contains(&"denied".to_string()));
        assert!(policy
            .always_promote_dispositions
            .contains(&"blocked".to_string()));
        assert!(policy
            .drop_activity_names
            .contains(&"heartbeat".to_string()));
    }
}
