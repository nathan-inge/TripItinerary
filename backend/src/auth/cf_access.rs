use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use axum::{
    extract::State,
    http::{header::HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::error::AppError;
use crate::AppState;

const JWT_HEADER: &str = "Cf-Access-Jwt-Assertion";

/// A Cloudflare-Access-authenticated caller. `email` is lowercased so it
/// compares cleanly against the `trip_members` / owner allow-lists.
///
/// `provisioned` means "this address is known to the app" — it is in
/// `OWNER_EMAIL`, holds a `trip_members` row, or owns a trip. Cloudflare
/// Access is the outer gate, but we keep this app-level check so a widened
/// Access policy can't silently hand an unknown address a working account.
#[derive(Clone, Debug)]
pub struct Identity {
    pub email: String,
    pub provisioned: bool,
}

#[derive(Debug, Deserialize)]
struct Claims {
    email: String,
    #[allow(dead_code)]
    exp: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Jwk {
    kid: String,
    #[serde(rename = "n")]
    n: String,
    #[serde(rename = "e")]
    e: String,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Default)]
struct JwksCache {
    fetched_at: Option<Instant>,
    keys: Vec<Jwk>,
}

#[derive(Clone, Default)]
pub struct JwksClient {
    inner: Arc<RwLock<JwksCache>>,
}

impl JwksClient {
    pub(crate) async fn keys(&self, team_domain: &str) -> Result<Vec<Jwk>> {
        const TTL: Duration = Duration::from_secs(3600);
        {
            let cache = self.inner.read().await;
            if let Some(t) = cache.fetched_at {
                if t.elapsed() < TTL && !cache.keys.is_empty() {
                    return Ok(cache.keys.clone());
                }
            }
        }
        let url = format!("https://{}/cdn-cgi/access/certs", team_domain);
        let body: Jwks = reqwest::get(&url).await?.error_for_status()?.json().await?;
        let mut cache = self.inner.write().await;
        cache.fetched_at = Some(Instant::now());
        cache.keys = body.keys.clone();
        Ok(body.keys)
    }
}

/// Verify the Access JWT and attach an [`Identity`]. Does *not* reject
/// unprovisioned callers — `/api/me` is reachable without a grant so the SPA
/// can tell a signed-in-but-not-invited person what's wrong. Every other
/// authenticated route sits behind [`require_provisioned`].
pub async fn require_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    if state.cfg.dev_bypass_auth {
        req.extensions_mut().insert(Identity {
            email: state.cfg.primary_owner().to_string(),
            provisioned: true,
        });
        return Ok(next.run(req).await);
    }

    let token = headers
        .get(JWT_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let team = state
        .cfg
        .cf_access_team_domain
        .as_deref()
        .ok_or_else(|| AppError::Internal(anyhow!("CF_ACCESS_TEAM_DOMAIN not set")))?;
    let expected_aud = state
        .cfg
        .cf_access_aud
        .as_deref()
        .ok_or_else(|| AppError::Internal(anyhow!("CF_ACCESS_AUD not set")))?;

    let email = verify(&state.jwks_client, team, expected_aud, token)
        .await
        .map_err(|e| {
            tracing::warn!("CF Access JWT rejected: {e:#}");
            AppError::Unauthorized
        })?;

    let provisioned = is_provisioned(&state, &email).await?;
    if !provisioned {
        tracing::warn!(
            "CF Access JWT email {} has no owner-allow-list entry, trip membership, \
             or owned trip; only /api/me is reachable",
            email
        );
    }

    req.extensions_mut().insert(Identity { email, provisioned });
    Ok(next.run(req).await)
}

/// Gate for every authenticated route except `/api/me`.
pub async fn require_provisioned(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    let ok = req
        .extensions()
        .get::<Identity>()
        .is_some_and(|i| i.provisioned);
    if !ok {
        return Err(AppError::Forbidden);
    }
    Ok(next.run(req).await)
}

/// True if the app knows this address: configured owner, member of some trip,
/// or the owner of a trip it created itself. The last clause matters — without
/// it, revoking someone's last membership would also lock them out of trips
/// they own.
async fn is_provisioned(state: &AppState, email: &str) -> Result<bool, sqlx::Error> {
    if state.cfg.is_owner(email) {
        return Ok(true);
    }
    let row: (i64,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM trip_members WHERE email = ?1)
             OR EXISTS(SELECT 1 FROM trips WHERE lower(owner_email) = ?1)",
    )
    .bind(email)
    .fetch_one(&state.pool)
    .await?;
    Ok(row.0 != 0)
}

async fn verify(
    jwks: &JwksClient,
    team_domain: &str,
    expected_aud: &str,
    token: &str,
) -> Result<String> {
    let header = decode_header(token)?;
    let kid = header.kid.ok_or_else(|| anyhow!("missing kid"))?;
    let keys = jwks.keys(team_domain).await?;
    let jwk = keys
        .into_iter()
        .find(|k| k.kid == kid)
        .ok_or_else(|| anyhow!("kid {} not in JWKS", kid))?;

    let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)?;
    let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_audience(&[expected_aud]);
    validation.set_issuer(&[format!("https://{}", team_domain)]);
    validation.leeway = 30;

    let data = decode::<Claims>(token, &key, &validation)?;
    if data.claims.email.is_empty() {
        return Err(anyhow!("email claim missing"));
    }
    Ok(data.claims.email.to_ascii_lowercase())
}
