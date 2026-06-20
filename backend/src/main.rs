mod auth;
mod config;
mod db;
mod error;
mod models;
mod parse;
mod routes;
mod static_assets;

use anyhow::Result;
use axum::Router;
use sqlx::SqlitePool;
use std::net::SocketAddr;
use std::sync::Arc;
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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tripit=info,tower_http=info".into()),
        )
        .init();

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
        .fallback(static_assets::handler)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = cfg.bind_addr.parse()?;
    tracing::info!("tripit listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
