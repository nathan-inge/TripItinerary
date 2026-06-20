use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;

use crate::auth::OwnerIdentity;
use crate::error::{AppError, AppResult};
use crate::models::{CreateTripEmailSender, TripEmailSender};
use crate::routes::trips::require_trip_access;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/trips/:trip_id/email-senders",
            get(list).post(create),
        )
        .route(
            "/trips/:trip_id/email-senders/:email",
            axum::routing::delete(delete),
        )
}

pub fn normalize_email(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub async fn list(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(trip_id): Path<String>,
) -> AppResult<Json<Vec<TripEmailSender>>> {
    require_trip_access(&state, &trip_id, &owner.email).await?;
    let rows: Vec<TripEmailSender> = sqlx::query_as(
        "SELECT * FROM trip_email_senders WHERE trip_id = ?1 ORDER BY email",
    )
    .bind(&trip_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn create(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(trip_id): Path<String>,
    Json(payload): Json<CreateTripEmailSender>,
) -> AppResult<Json<TripEmailSender>> {
    require_trip_access(&state, &trip_id, &owner.email).await?;
    let email = normalize_email(&payload.email);
    if !email.contains('@') {
        return Err(AppError::BadRequest("email must contain @".into()));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO trip_email_senders (trip_id, email, created_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (trip_id, email) DO NOTHING",
    )
    .bind(&trip_id)
    .bind(&email)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    let row: TripEmailSender = sqlx::query_as(
        "SELECT * FROM trip_email_senders WHERE trip_id = ?1 AND email = ?2",
    )
    .bind(&trip_id)
    .bind(&email)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

pub async fn delete(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path((trip_id, email)): Path<(String, String)>,
) -> AppResult<axum::http::StatusCode> {
    require_trip_access(&state, &trip_id, &owner.email).await?;
    let email = normalize_email(&email);
    sqlx::query("DELETE FROM trip_email_senders WHERE trip_id = ?1 AND email = ?2")
        .bind(&trip_id)
        .bind(&email)
        .execute(&state.pool)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
