use std::collections::HashMap;
use std::env;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ProjectionConfig {
    pub db_connection_string: String,
    pub context_api_port: u16,
    pub projector_poll_interval_ms: u64,
    pub projector_batch_size: i64,
    pub metrics_port: u16,
    pub auth_tenant_config: Option<HashMap<String, Vec<String>>>,
    pub ocsf_enabled: bool,
    pub ocsf_tenant_id: String,
    pub ocsf_is_fixture: bool,
    pub ocsf_admission_policy_json: Option<String>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    Missing(String),
    #[error("invalid value for {0}: {1}")]
    Invalid(String, String),
}

impl ProjectionConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let auth_json = env::var("PROJECTION_AUTH_TENANT_CONFIG").ok();
        let auth_tenant_config = match auth_json {
            Some(json) => {
                let parsed: HashMap<String, Vec<String>> =
                    serde_json::from_str(&json).map_err(|e| {
                        ConfigError::Invalid(
                            "PROJECTION_AUTH_TENANT_CONFIG".to_string(),
                            e.to_string(),
                        )
                    })?;
                Some(parsed)
            }
            None => None,
        };

        Ok(Self {
            db_connection_string: required("PROJECTION_DB_CONNECTION_STRING")?,
            context_api_port: parse_u16("PROJECTION_CONTEXT_API_PORT", 8084)?,
            projector_poll_interval_ms: parse_u64("PROJECTION_PROJECTOR_POLL_INTERVAL_MS", 500)?,
            projector_batch_size: parse_u64("PROJECTION_PROJECTOR_BATCH_SIZE", 100)? as i64,
            metrics_port: parse_u16("PROJECTION_METRICS_PORT", 8085)?,
            auth_tenant_config,
            ocsf_enabled: parse_bool("PROJECTION_OCSF_ENABLED", false),
            ocsf_tenant_id: env::var("PROJECTION_OCSF_TENANT_ID")
                .unwrap_or_else(|_| "openshell".to_string()),
            ocsf_is_fixture: parse_bool("PROJECTION_OCSF_IS_FIXTURE", true),
            ocsf_admission_policy_json: env::var("PROJECTION_OCSF_ADMISSION_POLICY").ok(),
        })
    }
}

fn required(name: &str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError::Missing(name.to_string()))
}

fn parse_u16(name: &str, default: u16) -> Result<u16, ConfigError> {
    match env::var(name) {
        Ok(raw) => raw
            .parse::<u16>()
            .map_err(|_| ConfigError::Invalid(name.to_string(), "not a valid u16".to_string())),
        Err(_) => Ok(default),
    }
}

fn parse_bool(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(raw) => matches!(raw.to_lowercase().as_str(), "true" | "1" | "yes"),
        Err(_) => default,
    }
}

fn parse_u64(name: &str, default: u64) -> Result<u64, ConfigError> {
    match env::var(name) {
        Ok(raw) => raw
            .parse::<u64>()
            .map_err(|_| ConfigError::Invalid(name.to_string(), "not a valid u64".to_string())),
        Err(_) => Ok(default),
    }
}
