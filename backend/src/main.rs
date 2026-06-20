mod auth;
mod config;
mod db;
mod error;
mod models;
mod openapi;
mod parse;
mod routes;
mod static_assets;

use anyhow::Result;
use axum::{http::header, response::IntoResponse, routing::get, Router};
use sqlx::SqlitePool;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: Arc<config::Config>,
    pub jwks_client: auth::cf_access::JwksClient,
    pub claude: Option<parse::ClaudeClient>,
    pub token_cap: parse::TokenCap,
}

static OPENAPI_JSON: OnceLock<String> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tripit=info,tower_http=info".into()),
        )
        .init();

    // utoipa's derived `openapi()` recursively builds the spec and can
    // overflow the default thread stack on large schemas. Build once at
    // startup on an explicit large-stack thread, serialize, cache.
    OPENAPI_JSON
        .set(build_openapi_json()?)
        .map_err(|_| anyhow::anyhow!("openapi already set"))?;

    let cfg = config::Config::from_env()?;
    let pool = db::create_pool(&cfg.database_url).await?;
    db::run_migrations(&pool).await?;

    let claude = cfg
        .anthropic_api_key
        .clone()
        .map(|key| parse::ClaudeClient::new(key, cfg.parse_model.clone()));

    let state = AppState {
        pool: pool.clone(),
        cfg: Arc::new(cfg.clone()),
        jwks_client: auth::cf_access::JwksClient::default(),
        claude,
        token_cap: parse::TokenCap::default(),
    };

    let api = routes::api_router(state.clone());

    let app = Router::new()
        .nest("/api", api)
        .route("/api/openapi.json", get(openapi_json))
        .fallback(static_assets::handler)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = cfg.bind_addr.parse()?;
    tracing::info!("tripit listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_openapi_json() -> Result<String> {
    use utoipa::OpenApi;
    let json = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| serde_json::to_string(&openapi::ApiDoc::openapi()).unwrap())
        .map_err(|e| anyhow::anyhow!("spawn openapi thread: {e}"))?
        .join()
        .map_err(|_| anyhow::anyhow!("openapi build panicked"))?;
    Ok(json)
}

async fn openapi_json() -> impl IntoResponse {
    let body = OPENAPI_JSON.get().expect("openapi initialized at startup");
    (
        [(header::CONTENT_TYPE, "application/json")],
        body.as_str().to_owned(),
    )
}
