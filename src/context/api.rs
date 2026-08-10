use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::auth::AuthorizationProvider;
use crate::context::service::{ContextError, ContextService};
use crate::contract::memory_event::MemoryEventV1;
use crate::metrics;
use crate::projection::repository::ProjectionRepository;

type SharedService<P, A> = Arc<ContextService<P, A>>;

pub fn context_api_router<P: ProjectionRepository + 'static, A: AuthorizationProvider + 'static>(
    service: Arc<ContextService<P, A>>,
) -> Router {
    Router::new()
        .route(
            "/v1/memory/context/{tenant_id}/{subject_id}",
            get(get_context::<P, A>),
        )
        .route(
            "/v1/memory/decisions/{tenant_id}/{subject_id}/history",
            get(get_decision_history::<P, A>),
        )
        .route("/v1/memory/events", post(post_event::<P, A>))
        .route("/v1/memory/status", get(get_status::<P, A>))
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(get_readiness::<P, A>))
        .with_state(service)
}

fn extract_agent_id(headers: &HeaderMap) -> Result<String, (StatusCode, String)> {
    headers
        .get("x-agent-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or((
            StatusCode::BAD_REQUEST,
            "missing or empty X-Agent-ID header".to_string(),
        ))
}

async fn get_context<P: ProjectionRepository + 'static, A: AuthorizationProvider + 'static>(
    State(service): State<SharedService<P, A>>,
    Path((tenant_id, subject_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let agent_id = match extract_agent_id(&headers) {
        Ok(id) => id,
        Err((status, msg)) => {
            metrics::CONTEXT_API_REQUESTS
                .with_label_values(&["context", "400"])
                .inc();
            return Err((status, msg));
        }
    };

    let timer = metrics::CONTEXT_API_DURATION.start_timer();
    match service
        .get_context(&agent_id, &tenant_id, &subject_id)
        .await
    {
        Ok(bundle) => {
            timer.observe_duration();
            metrics::CONTEXT_API_REQUESTS
                .with_label_values(&["context", "200"])
                .inc();
            Ok(Json(bundle))
        }
        Err(ContextError::Forbidden(_)) => {
            timer.observe_duration();
            metrics::CONTEXT_API_REQUESTS
                .with_label_values(&["context", "403"])
                .inc();
            Err((StatusCode::FORBIDDEN, "access denied".to_string()))
        }
        Err(ContextError::Unavailable) | Err(ContextError::InvalidEvent(_)) => {
            timer.observe_duration();
            metrics::CONTEXT_API_REQUESTS
                .with_label_values(&["context", "503"])
                .inc();
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "projection unavailable".to_string(),
            ))
        }
    }
}

async fn get_decision_history<
    P: ProjectionRepository + 'static,
    A: AuthorizationProvider + 'static,
>(
    State(service): State<SharedService<P, A>>,
    Path((tenant_id, subject_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let agent_id = match extract_agent_id(&headers) {
        Ok(id) => id,
        Err((status, msg)) => return Err((status, msg)),
    };

    match service
        .get_decision_history(&agent_id, &tenant_id, &subject_id)
        .await
    {
        Ok(history) => Ok(Json(history)),
        Err(ContextError::Forbidden(_)) => {
            Err((StatusCode::FORBIDDEN, "access denied".to_string()))
        }
        Err(ContextError::Unavailable) | Err(ContextError::InvalidEvent(_)) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "projection unavailable".to_string(),
        )),
    }
}

async fn post_event<P: ProjectionRepository + 'static, A: AuthorizationProvider + 'static>(
    State(service): State<SharedService<P, A>>,
    headers: HeaderMap,
    Json(event): Json<MemoryEventV1>,
) -> impl IntoResponse {
    let agent_id = match extract_agent_id(&headers) {
        Ok(id) => id,
        Err((status, msg)) => return Err((status, msg)),
    };

    match service.write_event(&agent_id, event).await {
        Ok(response) => Ok((StatusCode::CREATED, Json(response))),
        Err(ContextError::Forbidden(_)) => {
            Err((StatusCode::FORBIDDEN, "access denied".to_string()))
        }
        Err(ContextError::InvalidEvent(msg)) => Err((StatusCode::BAD_REQUEST, msg)),
        Err(ContextError::Unavailable) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "projection unavailable".to_string(),
        )),
    }
}

async fn get_status<P: ProjectionRepository + 'static, A: AuthorizationProvider + 'static>(
    State(service): State<SharedService<P, A>>,
) -> impl IntoResponse {
    match service.get_projection_status().await {
        Ok(status) => Ok(Json(status)),
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "status unavailable".to_string(),
        )),
    }
}

async fn get_readiness<P: ProjectionRepository + 'static, A: AuthorizationProvider + 'static>(
    State(service): State<SharedService<P, A>>,
) -> impl IntoResponse {
    match service.get_projection_status().await {
        Ok(status) if status.is_healthy => (StatusCode::OK, "ready"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "not ready"),
    }
}
