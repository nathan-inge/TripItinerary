use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;

use crate::auth::Identity;
use crate::error::AppResult;
use crate::models::{DayNote, UpsertDayNote};
use crate::routes::trips::{require_trip_access, require_trip_edit};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/trips/:trip_id/day-notes", get(list_notes))
        .route("/trips/:trip_id/day-notes/:date", axum::routing::put(upsert_note))
}

pub async fn list_notes(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(trip_id): Path<String>,
) -> AppResult<Json<Vec<DayNote>>> {
    require_trip_access(&state.pool, &trip_id, &me.email).await?;
    let rows: Vec<DayNote> =
        sqlx::query_as("SELECT * FROM day_notes WHERE trip_id = ?1 ORDER BY date")
            .bind(&trip_id)
            .fetch_all(&state.pool)
            .await?;
    Ok(Json(rows))
}

pub async fn upsert_note(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path((trip_id, date)): Path<(String, String)>,
    Json(payload): Json<UpsertDayNote>,
) -> AppResult<Json<DayNote>> {
    require_trip_edit(&state.pool, &trip_id, &me.email).await?;
    let now = Utc::now().to_rfc3339();

    if payload.notes_md.is_empty() {
        sqlx::query("DELETE FROM day_notes WHERE trip_id=?1 AND date=?2")
            .bind(&trip_id)
            .bind(&date)
            .execute(&state.pool)
            .await?;
        return Ok(Json(DayNote {
            trip_id,
            date,
            notes_md: String::new(),
            updated_at: now,
        }));
    }

    sqlx::query(
        "INSERT INTO day_notes (trip_id, date, notes_md, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (trip_id, date) DO UPDATE SET notes_md=excluded.notes_md, updated_at=excluded.updated_at",
    )
    .bind(&trip_id)
    .bind(&date)
    .bind(&payload.notes_md)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let row: DayNote =
        sqlx::query_as("SELECT * FROM day_notes WHERE trip_id=?1 AND date=?2")
            .bind(&trip_id)
            .bind(&date)
            .fetch_one(&state.pool)
            .await?;
    Ok(Json(row))
}
