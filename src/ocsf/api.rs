use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::AuthorizationProvider;
use crate::context::api::extract_agent_id;
use crate::context::service::{ContextError, ContextService};
use crate::metrics;
use crate::projection::repository::ProjectionRepository;

use super::adapter::OcsfAdapter;
use super::types::OcsfEvent;

struct OcsfState<P: ProjectionRepository, A: AuthorizationProvider> {
    adapter: Arc<OcsfAdapter>,
    service: Arc<ContextService<P, A>>,
}

impl<P: ProjectionRepository, A: AuthorizationProvider> Clone for OcsfState<P, A> {
    fn clone(&self) -> Self {
        Self {
            adapter: self.adapter.clone(),
            service: self.service.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize)]
struct OcsfIngestResponse {
    verdict: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<Uuid>,
    ocsf_class_uid: u32,
    ocsf_class_name: String,
    reason: String,
}

pub fn ocsf_api_router<P: ProjectionRepository + 'static, A: AuthorizationProvider + 'static>(
    adapter: Arc<OcsfAdapter>,
    service: Arc<ContextService<P, A>>,
) -> Router {
    let state = OcsfState { adapter, service };
    Router::new()
        .route("/v1/ocsf/events", post(ingest_event::<P, A>))
        .with_state(state)
}

async fn ingest_event<P: ProjectionRepository + 'static, A: AuthorizationProvider + 'static>(
    State(state): State<OcsfState<P, A>>,
    headers: HeaderMap,
    Json(ocsf_event): Json<OcsfEvent>,
) -> impl IntoResponse {
    let agent_id = match extract_agent_id(&headers) {
        Ok(id) => id,
        Err((status, msg)) => {
            metrics::OCSF_EVENTS_RECEIVED
                .with_label_values(&["unknown"])
                .inc();
            return Err((status, msg));
        }
    };

    let class_name = ocsf_event.resolved_class_name();
    metrics::OCSF_EVENTS_RECEIVED
        .with_label_values(&[&class_name])
        .inc();

    let result = match state.adapter.process(&ocsf_event) {
        Ok(r) => r,
        Err(e) => {
            return Err((StatusCode::BAD_REQUEST, e.to_string()));
        }
    };

    if let Some(memory_event) = result.memory_event {
        let event_id = memory_event.event_id;

        match state.service.write_event(&agent_id, memory_event).await {
            Ok(_) => {
                metrics::OCSF_EVENTS_PROMOTED
                    .with_label_values(&[&class_name])
                    .inc();
                Ok((
                    StatusCode::ACCEPTED,
                    Json(OcsfIngestResponse {
                        verdict: "promoted".to_string(),
                        event_id: Some(event_id),
                        ocsf_class_uid: ocsf_event.class_uid,
                        ocsf_class_name: class_name,
                        reason: result.verdict.reason().to_string(),
                    }),
                ))
            }
            Err(ContextError::Forbidden(_)) => {
                Err((StatusCode::FORBIDDEN, "access denied".to_string()))
            }
            Err(ContextError::InvalidEvent(msg)) => Err((StatusCode::BAD_REQUEST, msg)),
            Err(ContextError::Unavailable) => Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "projection unavailable".to_string(),
            )),
        }
    } else {
        metrics::OCSF_EVENTS_DROPPED
            .with_label_values(&[&class_name])
            .inc();
        Ok((
            StatusCode::OK,
            Json(OcsfIngestResponse {
                verdict: "dropped".to_string(),
                event_id: None,
                ocsf_class_uid: ocsf_event.class_uid,
                ocsf_class_name: class_name,
                reason: result.verdict.reason().to_string(),
            }),
        ))
    }
}
