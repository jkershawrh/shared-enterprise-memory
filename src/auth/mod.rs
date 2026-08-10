use std::collections::HashMap;

use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthAction {
    ReadContext,
    ReadDecisions,
    ReadHistory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult {
    Allowed,
    Denied(String),
}

#[async_trait]
pub trait AuthorizationProvider: Send + Sync {
    async fn authorize(&self, agent_id: &str, tenant_id: &str, action: AuthAction) -> AuthResult;
}

pub struct DevAuthProvider {
    tenant_agents: HashMap<String, Vec<String>>,
}

impl DevAuthProvider {
    pub fn open() -> Self {
        Self {
            tenant_agents: HashMap::new(),
        }
    }

    pub fn with_config(tenant_agents: HashMap<String, Vec<String>>) -> Self {
        Self { tenant_agents }
    }
}

#[async_trait]
impl AuthorizationProvider for DevAuthProvider {
    async fn authorize(&self, agent_id: &str, tenant_id: &str, _action: AuthAction) -> AuthResult {
        if self.tenant_agents.is_empty() {
            return AuthResult::Allowed;
        }
        match self.tenant_agents.get(tenant_id) {
            Some(allowed) if allowed.iter().any(|a| a == agent_id) => AuthResult::Allowed,
            _ => AuthResult::Denied("agent not authorized for this tenant".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn open_mode_allows_all() {
        let auth = DevAuthProvider::open();
        let result = auth
            .authorize("any-agent", "any-tenant", AuthAction::ReadContext)
            .await;
        assert_eq!(result, AuthResult::Allowed);
    }

    #[tokio::test]
    async fn configured_allows_listed_agent() {
        let mut config = HashMap::new();
        config.insert("acme".to_string(), vec!["ops-agent".to_string()]);
        let auth = DevAuthProvider::with_config(config);
        let result = auth
            .authorize("ops-agent", "acme", AuthAction::ReadContext)
            .await;
        assert_eq!(result, AuthResult::Allowed);
    }

    #[tokio::test]
    async fn configured_denies_unlisted_agent() {
        let mut config = HashMap::new();
        config.insert("acme".to_string(), vec!["ops-agent".to_string()]);
        let auth = DevAuthProvider::with_config(config);
        let result = auth
            .authorize("external-agent", "acme", AuthAction::ReadContext)
            .await;
        assert!(matches!(result, AuthResult::Denied(_)));
    }

    #[tokio::test]
    async fn configured_denies_wrong_tenant() {
        let mut config = HashMap::new();
        config.insert("acme".to_string(), vec!["ops-agent".to_string()]);
        let auth = DevAuthProvider::with_config(config);
        let result = auth
            .authorize("ops-agent", "other-tenant", AuthAction::ReadContext)
            .await;
        assert!(matches!(result, AuthResult::Denied(_)));
    }
}
