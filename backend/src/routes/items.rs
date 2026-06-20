use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth::OwnerIdentity;
use crate::error::{AppError, AppResult};
use crate::models::{
    CreateItem, Item, ItemSource, ItemType, Link, ParseStatus, UpdateItem,
};
use crate::routes::trips::require_trip_access;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/trips/:trip_id/items", post(create_item))
        .route(
            "/items/:id",
            axum::routing::get(get_item)
                .patch(update_item)
                .delete(delete_item),
        )
}

#[derive(sqlx::FromRow)]
struct ItemRow {
    id: String,
    trip_id: String,
    #[sqlx(rename = "type")]
    type_: String,
    title: String,
    start_at: String,
    end_at: Option<String>,
    location: Option<String>,
    confirmation_no: Option<String>,
    notes: String,
    details_json: String,
    links_json: String,
    raw_source: Option<String>,
    source: String,
    parse_status: String,
    created_at: String,
    updated_at: String,
}

fn parse_item_type(s: &str) -> AppResult<ItemType> {
    Ok(match s {
        "flight" => ItemType::Flight,
        "lodging" => ItemType::Lodging,
        "transport" => ItemType::Transport,
        "activity" => ItemType::Activity,
        other => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "unknown item type {} in DB",
                other
            )))
        }
    })
}

fn parse_source(s: &str) -> AppResult<ItemSource> {
    Ok(match s {
        "paste" => ItemSource::Paste,
        "email" => ItemSource::Email,
        "manual" => ItemSource::Manual,
        other => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "unknown source {} in DB",
                other
            )))
        }
    })
}

fn parse_parse_status(s: &str) -> AppResult<ParseStatus> {
    Ok(match s {
        "parsed" => ParseStatus::Parsed,
        "review" => ParseStatus::Review,
        "manual" => ParseStatus::Manual,
        other => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "unknown parse_status {} in DB",
                other
            )))
        }
    })
}

async fn hydrate(pool: &SqlitePool, row: ItemRow) -> AppResult<Item> {
    let details: serde_json::Value =
        serde_json::from_str(&row.details_json).map_err(|e| AppError::Internal(e.into()))?;
    let links: Vec<Link> =
        serde_json::from_str(&row.links_json).map_err(|e| AppError::Internal(e.into()))?;

    let traveler_rows: Vec<(String,)> =
        sqlx::query_as("SELECT traveler_id FROM item_travelers WHERE item_id = ?1")
            .bind(&row.id)
            .fetch_all(pool)
            .await?;
    let traveler_ids = traveler_rows.into_iter().map(|t| t.0).collect();

    Ok(Item {
        id: row.id,
        trip_id: row.trip_id,
        item_type: parse_item_type(&row.type_)?,
        title: row.title,
        start_at: row.start_at,
        end_at: row.end_at,
        location: row.location,
        confirmation_no: row.confirmation_no,
        notes: row.notes,
        details,
        links,
        traveler_ids,
        raw_source: row.raw_source,
        source: parse_source(&row.source)?,
        parse_status: parse_parse_status(&row.parse_status)?,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

pub async fn list_for_trip(pool: &SqlitePool, trip_id: &str) -> AppResult<Vec<Item>> {
    let rows: Vec<ItemRow> =
        sqlx::query_as("SELECT * FROM items WHERE trip_id = ?1 ORDER BY start_at")
            .bind(trip_id)
            .fetch_all(pool)
            .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(hydrate(pool, r).await?);
    }
    Ok(out)
}

pub async fn create_item(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(trip_id): Path<String>,
    Json(payload): Json<CreateItem>,
) -> AppResult<Json<Item>> {
    require_trip_access(&state, &trip_id, &owner.email).await?;
    if payload.title.trim().is_empty() {
        return Err(AppError::BadRequest("title required".into()));
    }

    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let details_json = serde_json::to_string(&payload.details)
        .map_err(|e| AppError::Internal(e.into()))?;
    let links_json = serde_json::to_string(&payload.links)
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO items (id, trip_id, type, title, start_at, end_at, location, confirmation_no,
                            notes, details_json, links_json, raw_source, source, parse_status,
                            created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15)",
    )
    .bind(&id)
    .bind(&trip_id)
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
        sqlx::query(
            "INSERT INTO item_travelers (item_id, traveler_id) VALUES (?1, ?2)",
        )
        .bind(&id)
        .bind(tid)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let row: ItemRow = sqlx::query_as("SELECT * FROM items WHERE id=?1")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(hydrate(&state.pool, row).await?))
}

pub async fn get_item(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(id): Path<String>,
) -> AppResult<Json<Item>> {
    let row = fetch_item_row(&state, &id, &owner.email).await?;
    Ok(Json(hydrate(&state.pool, row).await?))
}

pub async fn update_item(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateItem>,
) -> AppResult<Json<Item>> {
    let mut row = fetch_item_row(&state, &id, &owner.email).await?;
    if let Some(t) = payload.item_type {
        row.type_ = t.as_str().to_string();
    }
    if let Some(t) = payload.title {
        if t.trim().is_empty() {
            return Err(AppError::BadRequest("title cannot be empty".into()));
        }
        row.title = t;
    }
    if let Some(s) = payload.start_at {
        row.start_at = s;
    }
    if let Some(e) = payload.end_at {
        row.end_at = e;
    }
    if let Some(l) = payload.location {
        row.location = l;
    }
    if let Some(c) = payload.confirmation_no {
        row.confirmation_no = c;
    }
    if let Some(n) = payload.notes {
        row.notes = n;
    }
    if let Some(d) = payload.details {
        row.details_json =
            serde_json::to_string(&d).map_err(|e| AppError::Internal(e.into()))?;
    }
    if let Some(links) = payload.links {
        row.links_json =
            serde_json::to_string(&links).map_err(|e| AppError::Internal(e.into()))?;
    }
    if let Some(ps) = payload.parse_status {
        row.parse_status = ps.as_str().to_string();
    }
    let now = Utc::now().to_rfc3339();
    row.updated_at = now.clone();

    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE items SET type=?1, title=?2, start_at=?3, end_at=?4, location=?5,
            confirmation_no=?6, notes=?7, details_json=?8, links_json=?9,
            parse_status=?10, updated_at=?11
         WHERE id=?12",
    )
    .bind(&row.type_)
    .bind(&row.title)
    .bind(&row.start_at)
    .bind(&row.end_at)
    .bind(&row.location)
    .bind(&row.confirmation_no)
    .bind(&row.notes)
    .bind(&row.details_json)
    .bind(&row.links_json)
    .bind(&row.parse_status)
    .bind(&row.updated_at)
    .bind(&row.id)
    .execute(&mut *tx)
    .await?;

    if let Some(tids) = payload.traveler_ids {
        sqlx::query("DELETE FROM item_travelers WHERE item_id=?1")
            .bind(&row.id)
            .execute(&mut *tx)
            .await?;
        for tid in &tids {
            sqlx::query(
                "INSERT INTO item_travelers (item_id, traveler_id) VALUES (?1, ?2)",
            )
            .bind(&row.id)
            .bind(tid)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;

    Ok(Json(hydrate(&state.pool, row).await?))
}

pub async fn delete_item(
    State(state): State<AppState>,
    axum::extract::Extension(owner): axum::extract::Extension<OwnerIdentity>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    let _ = fetch_item_row(&state, &id, &owner.email).await?;
    sqlx::query("DELETE FROM items WHERE id=?1")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn fetch_item_row(state: &AppState, id: &str, owner_email: &str) -> AppResult<ItemRow> {
    let row: Option<ItemRow> = sqlx::query_as(
        "SELECT i.* FROM items i
         JOIN trips t ON t.id = i.trip_id
         WHERE i.id = ?1 AND t.owner_email = ?2",
    )
    .bind(id)
    .bind(owner_email)
    .fetch_optional(&state.pool)
    .await?;
    row.ok_or_else(|| AppError::NotFound(format!("item {} not found", id)))
}
