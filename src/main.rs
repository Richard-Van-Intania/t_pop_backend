use axum::{Router, http::StatusCode, routing::get};
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::time::Duration;
use tokio::signal;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool<Postgres>,
}

#[tokio::main]
async fn main() {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace")),
        )
        .with_target(true)
        .with_level(true)
        .init();

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect("postgres://postgres:KHzgNMS2SMKA5Hi2ddPXdh97dEzoGbLSLDT7tNOLU0QoipuudtcQ3tgXO0FxXAD0@localhost:5432/t_pop_app")
        .await
        .unwrap();

    let state = AppState { pool };

    let app = Router::new()
        .route("/", get(async || "t_pop_backend_is_healthy"))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(15),
        ))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
