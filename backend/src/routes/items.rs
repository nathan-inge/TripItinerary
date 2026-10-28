use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth::Identity;
use crate::error::{AppError, AppResult};
use crate::models::{
    CreateItem, Item, ItemPreview, ItemSource, ItemType, Link, ParseStatus,
    ParseTextRequest, ParseTextResponse, UpdateItem,
};
use crate::config::ParseBackend;
use crate::parse::dry_run::{log_dry_run, log_llm_input};
use crate::parse::prompt::estimate_tokens;
use crate::parse::redact::{lookup_conf_placeholder, redact_for_remote};
use crate::routes::trips::{require_trip_access, require_trip_edit};
use crate::AppState;

/// Load traveler names for a trip — needed to redact them out of bodies
/// before they leave the host. Returns owned strings so the caller can
/// borrow them as `&[&str]` for redact_for_remote.
pub(crate) async fn traveler_names_for(
    pool: &SqlitePool,
    trip_id: &str,
) -> AppResult<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM travelers WHERE trip_id = ?1")
            .bind(trip_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Substitute a placeholder confirmation_no back to its real value via the
/// map. If the LLM emitted a non-placeholder value, log a WARN so the
/// operator can see the regex needs another label, and keep the raw value
/// (lenient — strict-drop would silently lose conf #s on unlabeled emails).
pub(crate) fn restore_conf_no(
    confirmation_no: Option<String>,
    conf_map: &std::collections::HashMap<String, String>,
) -> Option<String> {
    let Some(value) = confirmation_no else {
        return None;
    };
    if value.starts_with("[REDACTED:CONF_") {
        match lookup_conf_placeholder(&value, conf_map) {
            Some(real) => Some(real.clone()),
            None => {
                tracing::warn!(
                    "parser returned unknown conf placeholder {:?}; dropping",
                    value
                );
                None
            }
        }
    } else {
        tracing::warn!(
            "parser returned a raw conf value ({} chars); the label regex \
             likely missed this email's format. Keeping value but consider \
             adding the label to redact_booking_refs.",
            value.len()
        );
        Some(value)
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/trips/:trip_id/items", post(create_item))
        .route("/trips/:trip_id/items/parse", post(parse_text))
        .route(
            "/items/:id",
            axum::routing::get(get_item)
                .patch(update_item)
                .delete(delete_item),
        )
}

pub async fn parse_text(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(trip_id): Path<String>,
    Json(payload): Json<ParseTextRequest>,
) -> AppResult<Json<ParseTextResponse>> {
    require_trip_edit(&state.pool, &trip_id, &me.email).await?;

    let raw_text = payload.raw_text.trim();
    if raw_text.is_empty() {
        return Err(AppError::BadRequest("raw_text required".into()));
    }
    if raw_text.len() > 50_000 {
        return Err(AppError::BadRequest(
            "raw_text exceeds 50 KB limit".into(),
        ));
    }

    // Redact PII + conf #s + traveler names before the text leaves the host.
    // raw_source kept in the response is the original; only the prompt going
    // to the LLM is the redacted text.
    let names = traveler_names_for(&state.pool, &trip_id).await?;
    let names_ref: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let redacted = redact_for_remote(raw_text, &names_ref);

    // Dry-run short-circuit: log what would have been sent and bail. Skipped
    // before checking for a parser since dry-run intentionally has none.
    if matches!(state.cfg.parse_backend, ParseBackend::DryRun) {
        log_dry_run("items/parse", raw_text, &redacted);
        return Err(AppError::BadRequest(
            "dry-run mode: parse not executed; see server logs for redaction details"
                .into(),
        ));
    }

    let parser = state.parser.as_ref().ok_or_else(|| {
        AppError::BadRequest(
            "no parse backend configured (set TRIPIT_PARSE_BACKEND and credentials)".into(),
        )
    })?;

    // Audit log of what is about to leave the redactor — independent of
    // which backend processes it. Spot-check with `journalctl -f`.
    log_llm_input("items/parse", &redacted);

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
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(trip_id): Path<String>,
    Json(payload): Json<CreateItem>,
) -> AppResult<Json<Item>> {
    require_trip_edit(&state.pool, &trip_id, &me.email).await?;
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
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(id): Path<String>,
) -> AppResult<Json<Item>> {
    let row = fetch_item_row(&state, &id, &me.email, false).await?;
    Ok(Json(hydrate(&state.pool, row).await?))
}

pub async fn update_item(
    State(state): State<AppState>,
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateItem>,
) -> AppResult<Json<Item>> {
    let mut row = fetch_item_row(&state, &id, &me.email, true).await?;
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
    axum::extract::Extension(me): axum::extract::Extension<Identity>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    let _ = fetch_item_row(&state, &id, &me.email, true).await?;
    sqlx::query("DELETE FROM items WHERE id=?1")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Load an item, then authorize the caller against its trip. `/items/:id`
/// carries no trip id, so the row has to come first; the role resolver stays
/// the single place access is decided.
async fn fetch_item_row(
    state: &AppState,
    id: &str,
    email: &str,
    for_write: bool,
) -> AppResult<ItemRow> {
    let row: Option<ItemRow> = sqlx::query_as("SELECT * FROM items WHERE id = ?1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    let row = row.ok_or_else(|| AppError::NotFound(format!("item {} not found", id)))?;
    if for_write {
        require_trip_edit(&state.pool, &row.trip_id, email).await?;
    } else {
        require_trip_access(&state.pool, &row.trip_id, email).await?;
    }
    Ok(row)
}

pub async fn fetch_item_unscoped(pool: &SqlitePool, id: &str) -> AppResult<Item> {
    let row: ItemRow = sqlx::query_as("SELECT * FROM items WHERE id = ?1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    hydrate(pool, row).await
}

/// Update an item scoped to a trip (no owner check). Used by share-token routes.
pub async fn update_for_trip(
    pool: &SqlitePool,
    trip_id: &str,
    item_id: &str,
    payload: UpdateItem,
) -> AppResult<Item> {
    let mut row: ItemRow =
        sqlx::query_as("SELECT * FROM items WHERE id = ?1 AND trip_id = ?2")
            .bind(item_id)
            .bind(trip_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("item {} not found", item_id)))?;

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
    row.updated_at = Utc::now().to_rfc3339();

    let mut tx = pool.begin().await?;
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

    hydrate(pool, row).await
}
