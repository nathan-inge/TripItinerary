use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use uuid::Uuid;

use crate::auth::OwnerIdentity;
use crate::error::{AppError, AppResult};
use crate::models::{CreateTrip, Trip, TripDetail, UpdateTrip};
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
    let trips: Vec<Trip> = sqlx::query_as(
        "SELECT * FROM trips WHERE owner_email = ?1 ORDER BY start_date DESC",
    )
    .bind(&owner.email)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(trips))
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
    Ok(Json(TripDetail {
        trip,
        travelers,
        items,
        day_notes,
    }))
}

pub async fn update_trip(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTrip>,
) -> AppResult<Json<Trip>> {
    let mut trip = fetch_trip(&state, &id, &owner.email).await?;
    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest("name cannot be empty".into()));
        }
        trip.name = name;
    }
    if let Some(dest) = payload.destination {
        trip.destination = dest;
    }
    if let Some(sd) = payload.start_date {
        trip.start_date = sd;
    }
    if let Some(ed) = payload.end_date {
        trip.end_date = ed;
    }
    if trip.start_date > trip.end_date {
        return Err(AppError::BadRequest(
            "start_date must be <= end_date".into(),
        ));
    }
    if let Some(notes) = payload.notes_md {
        trip.notes_md = notes;
    }
    trip.updated_at = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE trips SET name=?1, destination=?2, start_date=?3, end_date=?4, notes_md=?5, updated_at=?6
         WHERE id=?7 AND owner_email=?8",
    )
    .bind(&trip.name)
    .bind(&trip.destination)
    .bind(&trip.start_date)
    .bind(&trip.end_date)
    .bind(&trip.notes_md)
    .bind(&trip.updated_at)
    .bind(&trip.id)
    .bind(&owner.email)
    .execute(&state.pool)
    .await?;
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
    let trip: Option<Trip> =
        sqlx::query_as("SELECT * FROM trips WHERE id=?1 AND owner_email=?2")
            .bind(id)
            .bind(owner_email)
            .fetch_optional(&state.pool)
            .await?;
    trip.ok_or_else(|| AppError::NotFound(format!("trip {} not found", id)))
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
