use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use uuid::Uuid;

use crate::auth::Identity;
use crate::error::{AppError, AppResult};
use crate::models::{CreateTrip, Trip, TripDetail, TripRole, TripRow, TripSummary, UpdateTrip};
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

/// A trip row plus the computed role column from the access query.
#[derive(sqlx::FromRow)]
struct TripRoleRow {
    #[sqlx(flatten)]
    trip: TripRow,
    role: String,
}

/// Trips the caller owns *plus* trips shared with their address, each tagged
/// with the role they hold on it.
pub async fn list_trips(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
) -> AppResult<Json<Vec<TripSummary>>> {
    let rows: Vec<TripRoleRow> = sqlx::query_as(
        "SELECT t.*,
                CASE WHEN lower(t.owner_email) = ?1 THEN 'owner' ELSE m.role END AS role
           FROM trips t
           LEFT JOIN trip_members m ON m.trip_id = t.id AND m.email = ?1
          WHERE lower(t.owner_email) = ?1 OR m.email IS NOT NULL
          ORDER BY t.start_date DESC",
    )
    .bind(&me.email)
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            let my_role = parse_role(&r.role)?;
            Ok(TripSummary {
                trip: Trip::from(r.trip),
                my_role,
            })
        })
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn create_trip(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<Identity>,
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

    let trip = fetch_trip_unscoped(&state.pool, &id).await?;
    Ok(Json(trip))
}

pub async fn get_trip(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(id): Path<String>,
) -> AppResult<Json<TripDetail>> {
    let my_role = require_trip_access(&state.pool, &id, &me.email).await?;
    let trip = fetch_trip_unscoped(&state.pool, &id).await?;
    let travelers = travelers::list_for_trip(&state.pool, &id).await?;
    let items = items::list_for_trip(&state.pool, &id).await?;
    let day_notes: Vec<crate::models::DayNote> =
        sqlx::query_as("SELECT * FROM day_notes WHERE trip_id = ?1 ORDER BY date")
            .bind(&id)
            .fetch_all(&state.pool)
            .await?;
    // The sender allow-list is owner-level config — members don't see it, the
    // same way share-link guests don't.
    let email_senders: Vec<crate::models::TripEmailSender> = if my_role.is_owner() {
        sqlx::query_as("SELECT * FROM trip_email_senders WHERE trip_id = ?1 ORDER BY email")
            .bind(&id)
            .fetch_all(&state.pool)
            .await?
    } else {
        vec![]
    };
    Ok(Json(TripDetail {
        trip,
        travelers,
        items,
        day_notes,
        email_senders,
        my_role,
    }))
}

pub async fn update_trip(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTrip>,
) -> AppResult<Json<Trip>> {
    require_trip_edit(&state.pool, &id, &me.email).await?;
    apply_update(&state.pool, &id, payload).await?;
    let trip = fetch_trip_unscoped(&state.pool, &id).await?;
    Ok(Json(trip))
}

pub async fn delete_trip(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    require_trip_owner(&state.pool, &id, &me.email).await?;
    sqlx::query("DELETE FROM trips WHERE id=?1")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn fetch_trip_unscoped(pool: &sqlx::SqlitePool, id: &str) -> AppResult<Trip> {
    let row: Option<TripRow> = sqlx::query_as("SELECT * FROM trips WHERE id=?1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    row.map(Trip::from)
        .ok_or_else(|| AppError::NotFound(format!("trip {} not found", id)))
}

fn parse_role(role: &str) -> AppResult<TripRole> {
    TripRole::from_db(role).ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!("unknown trip role {:?} in DB", role))
    })
}

/// The caller's effective role on a trip: `owner` if they hold
/// `trips.owner_email`, otherwise their `trip_members.role`. `None` means no
/// access — which the `require_*` helpers report as 404 rather than 403, so a
/// trip id can't be probed for existence.
pub async fn lookup_trip_role(
    pool: &sqlx::SqlitePool,
    trip_id: &str,
    email: &str,
) -> AppResult<Option<TripRole>> {
    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT CASE WHEN lower(t.owner_email) = ?2 THEN 'owner' ELSE m.role END AS role
           FROM trips t
           LEFT JOIN trip_members m ON m.trip_id = t.id AND m.email = ?2
          WHERE t.id = ?1",
    )
    .bind(trip_id)
    .bind(email)
    .fetch_optional(pool)
    .await?;
    // Outer None = no such trip; inner None = trip exists but no grant.
    match row.and_then(|(r,)| r) {
        Some(role) => parse_role(&role).map(Some),
        None => Ok(None),
    }
}

/// Any access at all (view or better). Returns the role so callers can gate
/// finer-grained affordances off it.
pub async fn require_trip_access(
    pool: &sqlx::SqlitePool,
    trip_id: &str,
    email: &str,
) -> AppResult<TripRole> {
    lookup_trip_role(pool, trip_id, email)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("trip {} not found", trip_id)))
}

/// Write access to trip *content* — items, notes, todos, travelers, trip
/// fields. Owners and edit members qualify; view members get 403.
pub async fn require_trip_edit(
    pool: &sqlx::SqlitePool,
    trip_id: &str,
    email: &str,
) -> AppResult<TripRole> {
    let role = require_trip_access(pool, trip_id, email).await?;
    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }
    Ok(role)
}

/// Owner-only surface: membership, share links, email senders, deletion.
pub async fn require_trip_owner(
    pool: &sqlx::SqlitePool,
    trip_id: &str,
    email: &str,
) -> AppResult<()> {
    if !require_trip_access(pool, trip_id, email).await?.is_owner() {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

/// Apply a partial UpdateTrip. Runs unscoped — every caller (owner route or
/// share token) has already verified edit access to `trip_id`.
pub async fn apply_update(
    pool: &sqlx::SqlitePool,
    trip_id: &str,
    payload: UpdateTrip,
) -> AppResult<()> {
    let mut new: TripRow = sqlx::query_as("SELECT * FROM trips WHERE id=?1")
        .bind(trip_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("trip {} not found", trip_id)))?;
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
