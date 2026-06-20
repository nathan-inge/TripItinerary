use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth::OwnerIdentity;
use crate::error::{AppError, AppResult};
use crate::models::{CreateTraveler, Traveler, UpdateTraveler};
use crate::routes::trips::require_trip_access;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/trips/:trip_id/travelers", post(create_traveler))
        .route(
            "/travelers/:id",
            get(get_traveler).patch(update_traveler).delete(delete_traveler),
        )
}

pub async fn list_for_trip(pool: &SqlitePool, trip_id: &str) -> AppResult<Vec<Traveler>> {
    let rows: Vec<Traveler> = sqlx::query_as(
        "SELECT * FROM travelers WHERE trip_id = ?1 ORDER BY sort_order, name",
    )
    .bind(trip_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn create_traveler(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(trip_id): Path<String>,
    Json(payload): Json<CreateTraveler>,
) -> AppResult<Json<Traveler>> {
    require_trip_access(&state, &trip_id, &owner.email).await?;
    if payload.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let id = Uuid::now_v7().to_string();
    let sort_order = payload.sort_order.unwrap_or(0);
    sqlx::query(
        "INSERT INTO travelers (id, trip_id, name, sort_order) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(&id)
    .bind(&trip_id)
    .bind(&payload.name)
    .bind(sort_order)
    .execute(&state.pool)
    .await?;
    let t: Traveler = sqlx::query_as("SELECT * FROM travelers WHERE id=?1")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(t))
}

pub async fn get_traveler(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(id): Path<String>,
) -> AppResult<Json<Traveler>> {
    let t = fetch_traveler(&state, &id, &owner.email).await?;
    Ok(Json(t))
}

pub async fn update_traveler(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTraveler>,
) -> AppResult<Json<Traveler>> {
    let mut t = fetch_traveler(&state, &id, &owner.email).await?;
    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest("name cannot be empty".into()));
        }
        t.name = name;
    }
    if let Some(s) = payload.sort_order {
        t.sort_order = s;
    }
    sqlx::query("UPDATE travelers SET name=?1, sort_order=?2 WHERE id=?3")
        .bind(&t.name)
        .bind(t.sort_order)
        .bind(&t.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(t))
}

pub async fn delete_traveler(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    let _ = fetch_traveler(&state, &id, &owner.email).await?;
    sqlx::query("DELETE FROM travelers WHERE id=?1")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn fetch_traveler(state: &AppState, id: &str, owner_email: &str) -> AppResult<Traveler> {
    let row: Option<Traveler> = sqlx::query_as(
        "SELECT t.* FROM travelers t
         JOIN trips tr ON tr.id = t.trip_id
         WHERE t.id = ?1 AND tr.owner_email = ?2",
    )
    .bind(id)
    .bind(owner_email)
    .fetch_optional(&state.pool)
    .await?;
    row.ok_or_else(|| AppError::NotFound(format!("traveler {} not found", id)))
}
