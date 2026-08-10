#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::info;

use projections::auth::DevAuthProvider;
use projections::config::ProjectionConfig;
use projections::context::api::context_api_router;
use projections::context::ContextService;
use projections::ledger::postgres::PostgresLedgerReader;
use projections::metrics;
use projections::projection::postgres::PostgresProjectionRepository;
use projections::projection::Projector;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .json()
        .with_current_span(false)
        .with_span_list(false)
        .init();

    let config = ProjectionConfig::from_env().context("failed to load config")?;
    info!(
        context_api_port = config.context_api_port,
        metrics_port = config.metrics_port,
        "projections service starting"
    );

    metrics::register_metrics();

    let pg_config: tokio_postgres::Config = config
        .db_connection_string
        .parse()
        .context("parsing db_connection_string")?;
    let manager = deadpool_postgres::Manager::new(pg_config, tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(16)
        .build()
        .context("building connection pool")?;

    let ledger_reader = Arc::new(PostgresLedgerReader::new(pool.clone()));
    let projection_repo = Arc::new(PostgresProjectionRepository::new(pool));

    let auth = Arc::new(match config.auth_tenant_config {
        Some(tenant_config) => DevAuthProvider::with_config(tenant_config),
        None => DevAuthProvider::open(),
    });

    let context_service = Arc::new(ContextService::new(projection_repo.clone(), auth));

    let (shutdown_tx, _) = watch::channel(false);

    {
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = shutdown_tx.send(true);
        });
    }

    let projector = Arc::new(Projector::new(
        ledger_reader,
        projection_repo,
        config.projector_batch_size,
    ));
    projector.clone().start_background(
        Duration::from_millis(config.projector_poll_interval_ms),
        shutdown_tx.subscribe(),
    );

    let api_addr = SocketAddr::from(([0, 0, 0, 0], config.context_api_port));
    let api_app = context_api_router(context_service);
    let api_listener = TcpListener::bind(api_addr)
        .await
        .context("failed to bind context API listener")?;
    info!(port = config.context_api_port, "context API listening");

    let mut api_shutdown_rx = shutdown_tx.subscribe();
    let api_server = tokio::spawn(async move {
        axum::serve(api_listener, api_app)
            .with_graceful_shutdown(async move {
                let _ = api_shutdown_rx.changed().await;
            })
            .await
    });

    let metrics_addr = SocketAddr::from(([0, 0, 0, 0], config.metrics_port));
    let metrics_app = Router::new().route(
        "/metrics",
        get(|| async {
            let body = metrics::encode_prometheus();
            (
                [(
                    axum::http::header::CONTENT_TYPE,
                    "text/plain; version=0.0.4; charset=utf-8",
                )],
                body,
            )
        }),
    );
    let metrics_listener = TcpListener::bind(metrics_addr)
        .await
        .context("failed to bind metrics listener")?;
    info!(port = config.metrics_port, "metrics listening");

    let mut metrics_shutdown_rx = shutdown_tx.subscribe();
    let metrics_server = tokio::spawn(async move {
        axum::serve(metrics_listener, metrics_app)
            .with_graceful_shutdown(async move {
                let _ = metrics_shutdown_rx.changed().await;
            })
            .await
    });

    let (api_result, metrics_result) = tokio::try_join!(api_server, metrics_server)?;
    api_result.context("context API server failed")?;
    metrics_result.context("metrics server failed")?;

    Ok(())
}
