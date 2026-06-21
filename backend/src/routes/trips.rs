use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use uuid::Uuid;

use crate::auth::OwnerIdentity;
use crate::error::{AppError, AppResult};
use crate::models::{CreateTrip, Trip, TripDetail, TripRow, UpdateTrip};
use crate::routes::{items, travelers};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/trips", get(list_trips).post(create_trip))
        .route(
            "/trips/:id",
            get(get_trip).patch(update_trip).delete(delete_trip),
        )
}

pub async fn list_trips(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
) -> AppResult<Json<Vec<Trip>>> {
    let rows: Vec<TripRow> = sqlx::query_as(
        "SELECT * FROM trips WHERE owner_email = ?1 ORDER BY start_date DESC",
    )
    .bind(&owner.email)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(Trip::from).collect()))
}

pub async fn create_trip(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Json(payload): Json<CreateTrip>,
) -> AppResult<Json<Trip>> {
    if payload.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    if payload.start_date > payload.end_date {
        return Err(AppError::BadRequest(
            "start_date must be <= end_date".into(),
        ));
    }
    let now = Utc::now().to_rfc3339();
    let id = Uuid::now_v7().to_string();
    let notes_md = payload.notes_md.unwrap_or_default();

    sqlx::query(
        "INSERT INTO trips (id, owner_email, name, destination, start_date, end_date, notes_md, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
    )
    .bind(&id)
    .bind(&owner.email)
    .bind(&payload.name)
    .bind(&payload.destination)
    .bind(&payload.start_date)
    .bind(&payload.end_date)
    .bind(&notes_md)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let trip = fetch_trip(&state, &id, &owner.email).await?;
    Ok(Json(trip))
}

pub async fn get_trip(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(id): Path<String>,
) -> AppResult<Json<TripDetail>> {
    let trip = fetch_trip(&state, &id, &owner.email).await?;
    let travelers = travelers::list_for_trip(&state.pool, &id).await?;
    let items = items::list_for_trip(&state.pool, &id).await?;
    let day_notes: Vec<crate::models::DayNote> =
        sqlx::query_as("SELECT * FROM day_notes WHERE trip_id = ?1 ORDER BY date")
            .bind(&id)
            .fetch_all(&state.pool)
            .await?;
    let email_senders: Vec<crate::models::TripEmailSender> = sqlx::query_as(
        "SELECT * FROM trip_email_senders WHERE trip_id = ?1 ORDER BY email",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(TripDetail {
        trip,
        travelers,
        items,
        day_notes,
        email_senders,
    }))
}

pub async fn update_trip(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTrip>,
) -> AppResult<Json<Trip>> {
    apply_update(&state.pool, &id, Some(&owner.email), payload).await?;
    let trip = fetch_trip(&state, &id, &owner.email).await?;
    Ok(Json(trip))
}

pub async fn delete_trip(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    let res = sqlx::query("DELETE FROM trips WHERE id=?1 AND owner_email=?2")
        .bind(&id)
        .bind(&owner.email)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("trip {} not found", id)));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn fetch_trip(state: &AppState, id: &str, owner_email: &str) -> AppResult<Trip> {
    let row: Option<TripRow> =
        sqlx::query_as("SELECT * FROM trips WHERE id=?1 AND owner_email=?2")
            .bind(id)
            .bind(owner_email)
            .fetch_optional(&state.pool)
            .await?;
    row.map(Trip::from)
        .ok_or_else(|| AppError::NotFound(format!("trip {} not found", id)))
}

pub async fn fetch_trip_unscoped(pool: &sqlx::SqlitePool, id: &str) -> AppResult<Trip> {
    let row: Option<TripRow> = sqlx::query_as("SELECT * FROM trips WHERE id=?1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    row.map(Trip::from)
        .ok_or_else(|| AppError::NotFound(format!("trip {} not found", id)))
}

pub async fn require_trip_access(
    state: &AppState,
    trip_id: &str,
    owner_email: &str,
) -> AppResult<()> {
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM trips WHERE id=?1 AND owner_email=?2")
            .bind(trip_id)
            .bind(owner_email)
            .fetch_one(&state.pool)
            .await?;
    if count.0 == 0 {
        return Err(AppError::NotFound(format!("trip {} not found", trip_id)));
    }
    Ok(())
}

/// Apply a partial UpdateTrip. If `owner_email` is Some, the UPDATE is scoped
/// to that owner; otherwise it runs unscoped (caller verified access already).
pub async fn apply_update(
    pool: &sqlx::SqlitePool,
    trip_id: &str,
    owner_email: Option<&str>,
    payload: UpdateTrip,
) -> AppResult<()> {
    let row: TripRow = match owner_email {
        Some(o) => sqlx::query_as("SELECT * FROM trips WHERE id=?1 AND owner_email=?2")
            .bind(trip_id)
            .bind(o)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("trip {} not found", trip_id)))?,
        None => sqlx::query_as("SELECT * FROM trips WHERE id=?1")
            .bind(trip_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("trip {} not found", trip_id)))?,
    };
    let mut new = row;
    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest("name cannot be empty".into()));
        }
        new.name = name;
    }
    if let Some(d) = payload.destination {
        new.destination = d;
    }
    if let Some(sd) = payload.start_date {
        new.start_date = sd;
    }
    if let Some(ed) = payload.end_date {
        new.end_date = ed;
    }
    if new.start_date > new.end_date {
        return Err(AppError::BadRequest(
            "start_date must be <= end_date".into(),
        ));
    }
    if let Some(notes) = payload.notes_md {
        new.notes_md = notes;
    }
    if let Some(p) = payload.photos_url {
        new.photos_url = p.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    }
    if let Some(m) = payload.maps_url {
        new.maps_url = m.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    }
    if let Some(todos) = payload.todos {
        // Trim text; drop empty entries.
        let cleaned: Vec<_> = todos
            .into_iter()
            .map(|mut t| {
                t.text = t.text.trim().to_string();
                t
            })
            .filter(|t| !t.text.is_empty())
            .collect();
        new.todos_json =
            serde_json::to_string(&cleaned).map_err(|e| AppError::Internal(e.into()))?;
    }
    new.updated_at = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE trips
         SET name=?1, destination=?2, start_date=?3, end_date=?4, notes_md=?5,
             photos_url=?6, maps_url=?7, todos_json=?8, updated_at=?9
         WHERE id=?10",
    )
    .bind(&new.name)
    .bind(&new.destination)
    .bind(&new.start_date)
    .bind(&new.end_date)
    .bind(&new.notes_md)
    .bind(&new.photos_url)
    .bind(&new.maps_url)
    .bind(&new.todos_json)
    .bind(&new.updated_at)
    .bind(&new.id)
    .execute(pool)
    .await?;
    Ok(())
}
