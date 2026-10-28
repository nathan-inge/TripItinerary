//! Email-based sharing: grant a Cloudflare-Access identity view or edit
//! access to a trip. Unlike a share link (anonymous bearer token), a member
//! signs in as themselves and sees the trip on their own home page.
//!
//! Owner-only surface — edit members can change trip content but not the
//! guest list.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;

use crate::auth::Identity;
use crate::error::{AppError, AppResult};
use crate::models::{TripMember, UpsertTripMember};
use crate::routes::email_senders::normalize_email;
use crate::routes::trips::require_trip_owner;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/trips/:trip_id/members", get(list).post(upsert))
        .route(
            "/trips/:trip_id/members/:email",
            axum::routing::delete(remove),
        )
}

pub async fn list(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(trip_id): Path<String>,
) -> AppResult<Json<Vec<TripMember>>> {
    require_trip_owner(&state.pool, &trip_id, &me.email).await?;
    let rows: Vec<TripMember> =
        sqlx::query_as("SELECT * FROM trip_members WHERE trip_id = ?1 ORDER BY email")
            .bind(&trip_id)
            .fetch_all(&state.pool)
            .await?;
    Ok(Json(rows))
}

/// Add a member, or change an existing member's role. Idempotent on email.
pub async fn upsert(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(trip_id): Path<String>,
    Json(payload): Json<UpsertTripMember>,
) -> AppResult<Json<TripMember>> {
    require_trip_owner(&state.pool, &trip_id, &me.email).await?;
    let email = normalize_email(&payload.email);
    if !email.contains('@') {
        return Err(AppError::BadRequest("email must contain @".into()));
    }

    // The owner's own address already resolves to the `owner` role; a member
    // row for it would be dead weight and confusing in the UI.
    let (owner_email,): (String,) =
        sqlx::query_as("SELECT lower(owner_email) FROM trips WHERE id = ?1")
            .bind(&trip_id)
            .fetch_one(&state.pool)
            .await?;
    if owner_email == email {
        return Err(AppError::BadRequest(
            "that address already owns this trip".into(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO trip_members (trip_id, email, role, created_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (trip_id, email) DO UPDATE SET role = excluded.role",
    )
    .bind(&trip_id)
    .bind(&email)
    .bind(payload.role.as_str())
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let row: TripMember =
        sqlx::query_as("SELECT * FROM trip_members WHERE trip_id = ?1 AND email = ?2")
            .bind(&trip_id)
            .bind(&email)
            .fetch_one(&state.pool)
            .await?;
    Ok(Json(row))
}

pub async fn remove(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path((trip_id, email)): Path<(String, String)>,
) -> AppResult<axum::http::StatusCode> {
    require_trip_owner(&state.pool, &trip_id, &me.email).await?;
    let email = normalize_email(&email);
    sqlx::query("DELETE FROM trip_members WHERE trip_id = ?1 AND email = ?2")
        .bind(&trip_id)
        .bind(&email)
        .execute(&state.pool)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
