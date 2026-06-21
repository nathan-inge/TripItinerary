//! Token-bearer routes for guests. Mirrors a subset of the owner API,
//! gated by the share_link's scope (view | edit).

use axum::{
    extract::{Path, Request, State},
    middleware::Next,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use uuid::Uuid;

use crate::auth::share_token::verify_token;
use crate::auth::ScopedAccess;
use crate::error::{AppError, AppResult};
use crate::models::{
    CreateItem, DayNote, Item, ItemPreview, ParseTextRequest, ParseTextResponse, ShareScope,
    SharedTripDetail, Traveler, Trip, TripDetail, UpdateItem, UpdateTrip, UpsertDayNote,
};
use crate::parse::prompt::estimate_tokens;
use crate::parse::redact::redact_for_remote;
use crate::routes::items::{restore_conf_no, traveler_names_for};
use crate::routes::items as items_routes;
use crate::routes::travelers as travelers_routes;
use crate::AppState;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/share/:token/trip", get(get_trip).patch(update_trip))
        .route("/share/:token/items", post(create_item))
        .route("/share/:token/items/parse", post(parse_text))
        .route(
            "/share/:token/items/:item_id",
            axum::routing::patch(update_item).delete(delete_item),
        )
        .route("/share/:token/day-notes", get(list_day_notes))
        .route(
            "/share/:token/day-notes/:date",
            axum::routing::put(upsert_day_note),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_share_token,
        ))
}

/// Middleware: parse the `:token` path segment, look up share_link, attach
/// ScopedAccess to req extensions. Stamps last_used_at.
async fn require_share_token(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let raw_token = extract_token(req.uri().path()).ok_or(AppError::Unauthorized)?;
    let link = verify_token(&state.pool, &raw_token)
        .await
        .map_err(AppError::Sqlx)?
        .ok_or(AppError::Unauthorized)?;
    let scope = match link.scope.as_str() {
        "view" => ShareScope::View,
        "edit" => ShareScope::Edit,
        _ => return Err(AppError::Internal(anyhow::anyhow!("bad scope in DB"))),
    };
    req.extensions_mut().insert(ScopedAccess {
        trip_id: link.trip_id.clone(),
        scope,
    });
    Ok(next.run(req).await)
}

fn extract_token(path: &str) -> Option<String> {
    let after = path.split("/share/").nth(1)?;
    let tok = after.split('/').next()?;
    if tok.is_empty() {
        None
    } else {
        Some(tok.to_string())
    }
}

fn require_edit(access: &ScopedAccess) -> AppResult<()> {
    if access.scope == ShareScope::Edit {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

// ----- handlers -----

async fn get_trip(
    State(state): State<AppState>,
    axum::extract::Extension(access): axum::extract::Extension<ScopedAccess>,
) -> AppResult<Json<SharedTripDetail>> {
    let trip: Trip = sqlx::query_as("SELECT * FROM trips WHERE id = ?1")
        .bind(&access.trip_id)
        .fetch_one(&state.pool)
        .await?;
    let travelers: Vec<Traveler> =
        travelers_routes::list_for_trip(&state.pool, &access.trip_id).await?;
    let items: Vec<Item> =
        items_routes::list_for_trip(&state.pool, &access.trip_id).await?;
    let day_notes: Vec<DayNote> =
        sqlx::query_as("SELECT * FROM day_notes WHERE trip_id = ?1 ORDER BY date")
            .bind(&access.trip_id)
            .fetch_all(&state.pool)
            .await?;
    Ok(Json(SharedTripDetail {
        trip: TripDetail {
            trip,
            travelers,
            items,
            day_notes,
            // Email-sender allow-list is owner-level config; guests don't see it.
            email_senders: vec![],
        },
        share_scope: access.scope,
    }))
}

async fn update_trip(
    State(state): State<AppState>,
    axum::extract::Extension(access): axum::extract::Extension<ScopedAccess>,
    Json(payload): Json<UpdateTrip>,
) -> AppResult<Json<Trip>> {
    require_edit(&access)?;
    let mut trip: Trip = sqlx::query_as("SELECT * FROM trips WHERE id = ?1")
        .bind(&access.trip_id)
        .fetch_one(&state.pool)
        .await?;
    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest("name cannot be empty".into()));
        }
        trip.name = name;
    }
    if let Some(d) = payload.destination {
        trip.destination = d;
    }
    if let Some(sd) = payload.start_date {
        trip.start_date = sd;
    }
    if let Some(ed) = payload.end_date {
        trip.end_date = ed;
    }
    if trip.start_date > trip.end_date {
        return Err(AppError::BadRequest("start_date must be <= end_date".into()));
    }
    if let Some(notes) = payload.notes_md {
        trip.notes_md = notes;
    }
    trip.updated_at = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE trips SET name=?1, destination=?2, start_date=?3, end_date=?4, notes_md=?5, updated_at=?6
         WHERE id=?7",
    )
    .bind(&trip.name)
    .bind(&trip.destination)
    .bind(&trip.start_date)
    .bind(&trip.end_date)
    .bind(&trip.notes_md)
    .bind(&trip.updated_at)
    .bind(&trip.id)
    .execute(&state.pool)
    .await?;
    Ok(Json(trip))
}

async fn create_item(
    State(state): State<AppState>,
    axum::extract::Extension(access): axum::extract::Extension<ScopedAccess>,
    Json(payload): Json<CreateItem>,
) -> AppResult<Json<Item>> {
    require_edit(&access)?;
    if payload.title.trim().is_empty() {
        return Err(AppError::BadRequest("title required".into()));
    }
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let details_json = serde_json::to_string(&payload.details).map_err(|e| AppError::Internal(e.into()))?;
    let links_json = serde_json::to_string(&payload.links).map_err(|e| AppError::Internal(e.into()))?;

    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO items (id, trip_id, type, title, start_at, end_at, location, confirmation_no,
                            notes, details_json, links_json, raw_source, source, parse_status,
                            created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15)",
    )
    .bind(&id)
    .bind(&access.trip_id)
    .bind(payload.item_type.as_str())
    .bind(&payload.title)
    .bind(&payload.start_at)
    .bind(&payload.end_at)
    .bind(&payload.location)
    .bind(&payload.confirmation_no)
    .bind(&payload.notes)
    .bind(&details_json)
    .bind(&links_json)
    .bind(&payload.raw_source)
    .bind(payload.source.as_str())
    .bind(payload.parse_status.as_str())
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    for tid in &payload.traveler_ids {
        sqlx::query("INSERT INTO item_travelers (item_id, traveler_id) VALUES (?1, ?2)")
            .bind(&id)
            .bind(tid)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    let row = items_routes::fetch_item_unscoped(&state.pool, &id).await?;
    Ok(Json(row))
}

async fn update_item(
    State(state): State<AppState>,
    axum::extract::Extension(access): axum::extract::Extension<ScopedAccess>,
    Path((_token, item_id)): Path<(String, String)>,
    Json(payload): Json<UpdateItem>,
) -> AppResult<Json<Item>> {
    require_edit(&access)?;
    items_routes::update_for_trip(&state.pool, &access.trip_id, &item_id, payload)
        .await
        .map(Json)
}

async fn delete_item(
    State(state): State<AppState>,
    axum::extract::Extension(access): axum::extract::Extension<ScopedAccess>,
    Path((_token, item_id)): Path<(String, String)>,
) -> AppResult<axum::http::StatusCode> {
    require_edit(&access)?;
    let res = sqlx::query("DELETE FROM items WHERE id=?1 AND trip_id=?2")
        .bind(&item_id)
        .bind(&access.trip_id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("item {} not found", item_id)));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_day_notes(
    State(state): State<AppState>,
    axum::extract::Extension(access): axum::extract::Extension<ScopedAccess>,
) -> AppResult<Json<Vec<DayNote>>> {
    let rows: Vec<DayNote> = sqlx::query_as(
        "SELECT * FROM day_notes WHERE trip_id = ?1 ORDER BY date",
    )
    .bind(&access.trip_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn upsert_day_note(
    State(state): State<AppState>,
    axum::extract::Extension(access): axum::extract::Extension<ScopedAccess>,
    Path((_token, date)): Path<(String, String)>,
    Json(payload): Json<UpsertDayNote>,
) -> AppResult<Json<DayNote>> {
    require_edit(&access)?;
    let now = Utc::now().to_rfc3339();
    if payload.notes_md.is_empty() {
        sqlx::query("DELETE FROM day_notes WHERE trip_id=?1 AND date=?2")
            .bind(&access.trip_id)
            .bind(&date)
            .execute(&state.pool)
            .await?;
        return Ok(Json(DayNote {
            trip_id: access.trip_id,
            date,
            notes_md: String::new(),
            updated_at: now,
        }));
    }
    sqlx::query(
        "INSERT INTO day_notes (trip_id, date, notes_md, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (trip_id, date) DO UPDATE SET notes_md=excluded.notes_md, updated_at=excluded.updated_at",
    )
    .bind(&access.trip_id)
    .bind(&date)
    .bind(&payload.notes_md)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    let row: DayNote = sqlx::query_as("SELECT * FROM day_notes WHERE trip_id=?1 AND date=?2")
        .bind(&access.trip_id)
        .bind(&date)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row))
}

async fn parse_text(
    State(state): State<AppState>,
    axum::extract::Extension(access): axum::extract::Extension<ScopedAccess>,
    Json(payload): Json<ParseTextRequest>,
) -> AppResult<Json<ParseTextResponse>> {
    require_edit(&access)?;
    let raw_text = payload.raw_text.trim();
    if raw_text.is_empty() {
        return Err(AppError::BadRequest("raw_text required".into()));
    }
    if raw_text.len() > 50_000 {
        return Err(AppError::BadRequest("raw_text exceeds 50 KB limit".into()));
    }
    let parser = state.parser.as_ref().ok_or_else(|| {
        AppError::BadRequest("no parse backend configured".into())
    })?;

    let names = traveler_names_for(&state.pool, &access.trip_id).await?;
    let names_ref: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let redacted = redact_for_remote(raw_text, &names_ref);

    let est = estimate_tokens(&redacted.text);
    state
        .token_cap
        .check_and_reserve(est, state.cfg.daily_token_cap)
        .await
        .map_err(AppError::BadRequest)?;
    let (parsed, actual) = match parser.parse_item(&redacted.text).await {
        Ok(v) => v,
        Err(e) => {
            state.token_cap.settle(est, 0).await;
            return Err(AppError::Internal(e));
        }
    };
    state.token_cap.settle(est, actual).await;
    let confirmation_no = restore_conf_no(parsed.confirmation_no, &redacted.conf_map);
    let preview = ItemPreview {
        item_type: parsed.item_type,
        title: parsed.title,
        start_at: parsed.start_at,
        end_at: parsed.end_at,
        location: parsed.location,
        confirmation_no,
        notes: parsed.notes,
        details: parsed.details,
        links: parsed.links,
        raw_source: raw_text.to_string(),
        source: parsed.source,
        parse_status: parsed.parse_status,
    };
    Ok(Json(ParseTextResponse {
        preview,
        review_reason: parsed.review_reason,
    }))
}

