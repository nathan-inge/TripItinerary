use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct Health {
    pub status: &'static str,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

pub async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}
