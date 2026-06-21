//! Inbound email intake from the Cloudflare Email Worker.
//!
//! Flow:
//!   1. Worker forwards mail → POSTs JSON {from, subject, text, html, received_at}
//!      with X-Tripit-Signature: hex(HMAC-SHA256(body, TRIPIT_EMAIL_HMAC_KEY)).
//!   2. We verify HMAC (constant time).
//!   3. First non-empty line of `text` is treated as the trip-name hint.
//!      Trip match is case-insensitive on whitespace-normalized name.
//!   4. The `from` address must be on the trip's email-sender allow-list.
//!   5. We respond 204 *immediately* — Cloudflare's tunnel cuts the connection
//!      at 100 s and the Worker doesn't care about the parse result. The LLM
//!      call + insert happens on a detached tokio task.
//!   6. Failures are logged but never bubbled to the Worker (a forwarded
//!      confirmation isn't worth a retry queue).

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{ItemSource, ParseStatus};
use crate::config::ParseBackend;
use crate::parse::dry_run::log_dry_run;
use crate::parse::prompt::estimate_tokens;
use crate::parse::redact::redact_for_remote;
use crate::routes::items::{restore_conf_no, traveler_names_for};
use crate::AppState;

type HmacSha256 = Hmac<Sha256>;

pub fn router() -> Router<AppState> {
    Router::new().route("/intake/email", post(intake))
}

#[derive(Debug, Deserialize)]
struct InboundEmail {
    from: String,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    #[allow(dead_code)]
    html: String,
    #[serde(default)]
    #[allow(dead_code)]
    received_at: Option<String>,
}

pub async fn intake(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<StatusCode> {
    // Step 1: HMAC. If misconfigured or invalid, drop quietly with 204 — we
    // never want a Worker retry storm on a misconfigured deploy.
    let Some(key) = state.cfg.email_hmac_key.as_deref() else {
        tracing::warn!("intake/email called but TRIPIT_EMAIL_HMAC_KEY unset; ignoring");
        return Ok(StatusCode::NO_CONTENT);
    };
    let Some(sig_header) = headers
        .get("x-tripit-signature")
        .and_then(|v| v.to_str().ok())
    else {
        tracing::warn!("intake/email missing X-Tripit-Signature; ignoring");
        return Ok(StatusCode::NO_CONTENT);
    };
    let expected = hmac_hex(key.as_bytes(), &body);
    if !ct_eq(sig_header.as_bytes(), expected.as_bytes()) {
        tracing::warn!("intake/email HMAC mismatch; ignoring");
        return Ok(StatusCode::NO_CONTENT);
    }

    // Step 2: parse JSON. Malformed → drop.
    let email: InboundEmail = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("intake/email bad JSON: {e}");
            return Ok(StatusCode::NO_CONTENT);
        }
    };

    let from = extract_email_addr(&email.from);
    if from.is_empty() {
        tracing::info!("intake/email no parseable From address; ignoring");
        return Ok(StatusCode::NO_CONTENT);
    }
    let from_norm = from.to_ascii_lowercase();

    // Step 3: trip-name hint from the first non-empty line of `text`.
    let Some(hint) = first_non_empty_line(&email.text) else {
        tracing::info!("intake/email empty body; ignoring");
        return Ok(StatusCode::NO_CONTENT);
    };
    let hint_norm = normalize(&hint);

    // Step 4: find trip whose normalized name matches. Hand-rolled to avoid
    // pulling in case-insensitive collation; trip lists are small (1s-10s).
    let trips: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM trips WHERE owner_email = ?1")
            .bind(&state.cfg.owner_email)
            .fetch_all(&state.pool)
            .await?;
    let Some(matched) = trips
        .into_iter()
        .find(|(_id, name)| normalize(name) == hint_norm)
    else {
        tracing::info!(
            sender = %from_norm,
            hint = %hint,
            "intake/email no matching trip; ignoring"
        );
        return Ok(StatusCode::NO_CONTENT);
    };
    let (trip_id, trip_name) = matched;

    // Step 5: sender allow-list.
    let allowed: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM trip_email_senders WHERE trip_id = ?1 AND email = ?2",
    )
    .bind(&trip_id)
    .bind(&from_norm)
    .fetch_one(&state.pool)
    .await?;
    if allowed.0 == 0 {
        tracing::warn!(
            trip = %trip_name,
            sender = %from_norm,
            "intake/email sender not on allow-list; ignoring"
        );
        return Ok(StatusCode::NO_CONTENT);
    }

    // Step 6: build the prompt text. Strip the trip-name annotation, prepend
    // the subject. Truncate large bodies — promotional HTML pages can be
    // hundreds of KB and the useful content fits comfortably in 50 KB.
    let raw_text = strip_first_line(&email.text);
    let mut prompt_text = if email.subject.is_empty() {
        raw_text
    } else {
        format!("Subject: {}\n\n{}", email.subject, raw_text)
    };
    if prompt_text.len() > MAX_PROMPT_BYTES {
        // Truncate at a char boundary, append a marker so the model knows.
        let mut cut = MAX_PROMPT_BYTES;
        while !prompt_text.is_char_boundary(cut) {
            cut -= 1;
        }
        prompt_text.truncate(cut);
        prompt_text.push_str("\n\n[truncated]");
        tracing::info!(
            "intake/email body truncated to {} bytes (original {} KB)",
            MAX_PROMPT_BYTES,
            email.text.len() / 1024
        );
    }

    if state.parser.is_none() {
        tracing::warn!(
            "intake/email matched trip {}, but no parse backend configured; dropping",
            trip_name
        );
        return Ok(StatusCode::NO_CONTENT);
    }

    tracing::info!(
        trip = %trip_name,
        sender = %from_norm,
        bytes = prompt_text.len(),
        "intake/email queued for background parse"
    );

    // Detach the slow path. We've already done all the cheap checks; if the
    // LLM call hangs the Cloudflare tunnel would 524 the Worker, the Worker
    // would log an error, and the parse would still be in flight here doing
    // nothing useful. Returning immediately keeps the contract honest: the
    // Worker is just notifying us; we're responsible for what comes next.
    let task_state = state.clone();
    tokio::spawn(async move {
        run_parse_and_insert(task_state, trip_id, trip_name, from_norm, prompt_text).await;
    });

    Ok(StatusCode::NO_CONTENT)
}

/// 50 KB cap on what we send to the LLM. Confirmations rarely need more
/// than ~5 KB of actual content; the rest is HTML markup or quoted threads.
const MAX_PROMPT_BYTES: usize = 50_000;

async fn run_parse_and_insert(
    state: AppState,
    trip_id: String,
    trip_name: String,
    sender: String,
    prompt_text: String,
) {
    // Redact before the prompt leaves the host. The trip's traveler list is
    // already on record (we used it on the allow-list check); use those names.
    let names = match traveler_names_for(&state.pool, &trip_id).await {
        Ok(n) => n,
        Err(e) => {
            tracing::error!("intake/email failed to load travelers: {e}");
            return;
        }
    };
    let names_ref: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let redacted = redact_for_remote(&prompt_text, &names_ref);

    // Dry-run: log what would have gone to the LLM and stop. No item is
    // inserted; the email is acknowledged silently like other dropped cases.
    if matches!(state.cfg.parse_backend, ParseBackend::DryRun) {
        log_dry_run(
            &format!("intake/email trip={trip_name} sender={sender}"),
            &prompt_text,
            &redacted,
        );
        tracing::info!(
            trip = %trip_name,
            sender = %sender,
            "intake/email dry-run — no item created"
        );
        return;
    }

    let parser = match state.parser.as_ref() {
        Some(p) => p,
        None => return, // already logged in caller
    };

    let est = estimate_tokens(&redacted.text);
    if let Err(e) = state
        .token_cap
        .check_and_reserve(est, state.cfg.daily_token_cap)
        .await
    {
        tracing::warn!("intake/email token cap: {e}; dropping");
        return;
    }

    let (parsed, used) = match parser.parse_item(&redacted.text).await {
        Ok(v) => v,
        Err(e) => {
            state.token_cap.settle(est, 0).await;
            tracing::error!(
                trip = %trip_name,
                sender = %sender,
                "intake/email parser ({}) error: {e:#}; dropping",
                parser.kind()
            );
            return;
        }
    };
    state.token_cap.settle(est, used).await;

    let confirmation_no = restore_conf_no(parsed.confirmation_no, &redacted.conf_map);

    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let details_json = serde_json::to_string(&parsed.details).unwrap_or_else(|_| "{}".into());
    let links_json = serde_json::to_string(&parsed.links).unwrap_or_else(|_| "[]".into());

    let res = sqlx::query(
        "INSERT INTO items (id, trip_id, type, title, start_at, end_at, location, confirmation_no,
                            notes, details_json, links_json, raw_source, source, parse_status,
                            created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15)",
    )
    .bind(&id)
    .bind(&trip_id)
    .bind(parsed.item_type.as_str())
    .bind(&parsed.title)
    .bind(&parsed.start_at)
    .bind(&parsed.end_at)
    .bind(&parsed.location)
    .bind(&confirmation_no)
    .bind(&parsed.notes)
    .bind(&details_json)
    .bind(&links_json)
    .bind(&prompt_text)
    .bind(ItemSource::Email.as_str())
    .bind(match parsed.parse_status {
        ParseStatus::Review => ParseStatus::Review.as_str(),
        _ => ParseStatus::Parsed.as_str(),
    })
    .bind(&now)
    .execute(&state.pool)
    .await;

    match res {
        Ok(_) => tracing::info!(
            trip = %trip_name,
            sender = %sender,
            item = %parsed.title,
            "intake/email saved"
        ),
        Err(e) => tracing::error!(
            trip = %trip_name,
            sender = %sender,
            "intake/email DB insert failed: {e}"
        ),
    }
}

fn hmac_hex(key: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(body);
    let bytes = mac.finalize().into_bytes();
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Pulls an email address out of common From-header shapes:
///   `nate@gmail.com`
///   `"Nathan" <nate@gmail.com>`
///   `Nathan <nate@gmail.com>`
fn extract_email_addr(from: &str) -> String {
    if let Some(start) = from.find('<') {
        if let Some(end) = from[start..].find('>') {
            return from[start + 1..start + end].trim().to_string();
        }
    }
    from.trim().to_string()
}

fn first_non_empty_line(text: &str) -> Option<String> {
    text.lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .map(|s| s.to_string())
}

/// Normalize a trip name for matching: lowercase, collapse runs of whitespace
/// to a single space, trim.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Drop the first non-empty line plus the (possibly blank) line immediately
/// after it, then return the remaining text. Used to remove the user's
/// trip-name annotation before forwarding the rest of the email to Claude.
fn strip_first_line(text: &str) -> String {
    let mut lines = text.lines();
    let mut found_first = false;
    let mut out = String::new();
    for line in lines.by_ref() {
        if !found_first {
            if !line.trim().is_empty() {
                found_first = true;
            }
            continue;
        }
        // Skip exactly one trailing blank line right after the annotation.
        if line.trim().is_empty() {
            // Append the rest as-is from here.
            for rest in lines {
                out.push_str(rest);
                out.push('\n');
            }
            return out.trim_start_matches('\n').to_string();
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_start_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_basic() {
        assert_eq!(normalize("Japan, Autumn 2026"), "japan, autumn 2026");
        assert_eq!(normalize("  Japan   autumn  "), "japan autumn");
        assert_eq!(normalize("JAPAN\tAUTUMN"), "japan autumn");
    }

    #[test]
    fn extract_addr() {
        assert_eq!(extract_email_addr("nate@gmail.com"), "nate@gmail.com");
        assert_eq!(extract_email_addr("Nate <nate@gmail.com>"), "nate@gmail.com");
        assert_eq!(
            extract_email_addr("\"Nathan I.\" <nate@gmail.com>"),
            "nate@gmail.com"
        );
    }

    #[test]
    fn first_line() {
        assert_eq!(
            first_non_empty_line("\n\nJapan, autumn 2026\n\nForwarded:\n"),
            Some("Japan, autumn 2026".into())
        );
    }

    #[test]
    fn strip_first_drops_annotation_and_blank() {
        let input = "Japan, autumn 2026\n\nFrom: Delta\nBody...\n";
        assert_eq!(strip_first_line(input), "From: Delta\nBody...\n");
    }

    #[test]
    fn hmac_is_stable() {
        let a = hmac_hex(b"key", b"hello");
        let b = hmac_hex(b"key", b"hello");
        assert_eq!(a, b);
        assert_ne!(a, hmac_hex(b"key", b"hellx"));
    }

    #[test]
    fn ct_eq_works() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
    }
}
