use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use uuid::Uuid;

use crate::auth::share_token::{hash_token, mint_token};
use crate::auth::Identity;
use crate::error::{AppError, AppResult};
use crate::models::{CreateShareLink, CreatedShareLink, ShareLink};
use crate::routes::trips::require_trip_owner;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/trips/:trip_id/share", get(list_links).post(create_link))
        // Note: this lives under /share-links/, NOT /share/. The /share/*
        // prefix is reserved for guest token-bearing routes (which CF Access
        // bypasses entirely); putting an owner-protected endpoint under
        // that prefix would collide with the CF Access Bypass rule and the
        // JWT header would never reach the owner middleware.
        .route("/share-links/:id", axum::routing::delete(revoke_link))
}

pub async fn list_links(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(trip_id): Path<String>,
) -> AppResult<Json<Vec<ShareLink>>> {
    require_trip_owner(&state.pool, &trip_id, &me.email).await?;
    let links: Vec<ShareLink> = sqlx::query_as(
        "SELECT * FROM share_links WHERE trip_id = ?1 ORDER BY created_at DESC",
    )
    .bind(&trip_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(links))
}

pub async fn create_link(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(trip_id): Path<String>,
    Json(payload): Json<CreateShareLink>,
) -> AppResult<Json<CreatedShareLink>> {
    require_trip_owner(&state.pool, &trip_id, &me.email).await?;

    let id = Uuid::now_v7().to_string();
    let token = mint_token();
    let token_hash = hash_token(&token);
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO share_links (id, trip_id, label, scope, token_hash, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&id)
    .bind(&trip_id)
    .bind(&payload.label)
    .bind(payload.scope.as_str())
    .bind(&token_hash)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let link: ShareLink = sqlx::query_as("SELECT * FROM share_links WHERE id = ?1")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?;

    Ok(Json(CreatedShareLink {
        url_path: format!("/share/{}", token),
        link,
        token,
    }))
}

pub async fn revoke_link(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    // Ensure the link belongs to a trip the caller owns. Edit members can't
    // mint or revoke links.
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT sl.id FROM share_links sl
         JOIN trips t ON t.id = sl.trip_id
         WHERE sl.id = ?1 AND lower(t.owner_email) = ?2",
    )
    .bind(&id)
    .bind(&me.email)
    .fetch_optional(&state.pool)
    .await?;
    if row.is_none() {
        return Err(AppError::NotFound(format!("share link {} not found", id)));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE share_links SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL")
        .bind(&now)
        .bind(&id)
        .execute(&state.pool)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
