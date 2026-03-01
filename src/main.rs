use axum::Json;
use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Router, http::StatusCode, routing::get};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::{Error, query_as};
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use tokio::signal;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use tracing_subscriber::{EnvFilter, fmt};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    pool: Pool<Postgres>,
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
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect("postgres://postgres:KHzgNMS2SMKA5Hi2ddPXdh97dEzoGbLSLDT7tNOLU0QoipuudtcQ3tgXO0FxXAD0@localhost:5432/t_pop_app")
        .await
        .unwrap();

    let state = AppState { pool };

    let app = Router::new()
        .route("/", get(async || "t_pop_backend_is_healthy"))
        .route("/login", post(login))
        .route("/packages", get(packages))
        .route("/subscriptions/{users_uuid}", get(subscriptions))
        .route("/subscriptions/buy", post(buy_subscription))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(15),
        ))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlainText {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EmailPassword {
    email: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Users {
    users_uuid: Uuid,
    email: String,
    display_name: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Packages {
    packages_uuid: Uuid,
    title: String,
    description: String,
    price: f64,
    duration_days: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    is_active: bool,
    benefits: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct SubscriptionWithPackage {
    subscriptions_uuid: Uuid,
    users_uuid: Uuid,
    packages_uuid: Uuid,
    subscription_created_at: DateTime<Utc>,
    expired_at: DateTime<Utc>,
    is_active: bool,
    payment_method: String,
    title: String,
    description: String,
    price: f64,
    duration_days: i32,
    benefits: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BuySubscription {
    users_uuid: Uuid,
    packages_uuid: Uuid,
    duration_days: i32,
    payment_method: String,
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<EmailPassword>,
) -> Result<Json<Users>, (StatusCode, String)> {
    let hash = blake3::hash(&payload.password.as_bytes()).to_string();
    let select: Result<Option<Users>, Error> =
        query_as("SELECT * FROM public.users WHERE email = $1 AND password = $2")
            .bind(&payload.email)
            .bind(&hash)
            .fetch_optional(&state.pool)
            .await;
    match select {
        Ok(ok) => match ok {
            Some(some) => Ok(Json(some)),
            None => Err((
                StatusCode::NOT_FOUND,
                "error_pg_select_users_not_found".to_string(),
            )),
        },
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "error_pg_select_users".to_string(),
        )),
    }
}

async fn packages(
    State(state): State<AppState>,
) -> Result<Json<Vec<Packages>>, (StatusCode, String)> {
    let select: Result<Vec<Packages>, Error> =
        query_as("SELECT * FROM public.packages WHERE is_active = true")
            .fetch_all(&state.pool)
            .await;
    match select {
        Ok(ok) => Ok(Json(ok)),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "error_pg_select_packages".to_string(),
        )),
    }
}

async fn subscriptions(
    State(state): State<AppState>,
    Path(users_uuid): Path<Uuid>,
) -> Result<Json<Vec<SubscriptionWithPackage>>, (StatusCode, String)> {
    let select: Result<Vec<SubscriptionWithPackage>, Error> = query_as(
        "SELECT s.subscriptions_uuid,
    s.users_uuid,
    s.packages_uuid,
    s.created_at AS subscription_created_at,
    s.expired_at,
    s.is_active,
    s.payment_method,
    p.title,
    p.description,
    p.price,
    p.duration_days,
    p.benefits
FROM public.subscriptions s
    JOIN packages p ON p.packages_uuid = s.packages_uuid
WHERE s.users_uuid = $1
ORDER BY s.created_at DESC",
    )
    .bind(&users_uuid)
    .fetch_all(&state.pool)
    .await;
    match select {
        Ok(ok) => Ok(Json(ok)),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "error_pg_select_subscriptions".to_string(),
        )),
    }
}

async fn buy_subscription(
    State(state): State<AppState>,
    Json(payload): Json<BuySubscription>,
) -> Result<Json<PlainText>, (StatusCode, String)> {
    let expired_at = Utc::now() + Duration::days(payload.duration_days as i64);
    let insert: Result<(Uuid,), Error> = query_as("INSERT INTO public.subscriptions( users_uuid, packages_uuid, created_at, expired_at, is_active, payment_method ) VALUES ($1, $2, now(), $3, true, $4) RETURNING subscriptions_uuid")
        .bind(&payload.users_uuid)
        .bind(&payload.packages_uuid)
        .bind(&expired_at)
        .bind(&payload.payment_method)
        .fetch_one(&state.pool)
        .await;
    match insert {
        Ok((subscriptions_uuid,)) => Ok(Json(PlainText {
            text: subscriptions_uuid.to_string(),
        })),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "error_pg_insert_subscriptions".to_string(),
        )),
    }
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
