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

#[derive(Clone, Debug)]
pub struct OwnerIdentity {
    pub email: String,
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

pub async fn require_owner(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    if state.cfg.dev_bypass_auth {
        req.extensions_mut().insert(OwnerIdentity {
            email: state.cfg.primary_owner().to_string(),
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

    let identity = verify(&state.jwks_client, team, expected_aud, token)
        .await
        .map_err(|e| {
            tracing::warn!("CF Access JWT rejected: {e:#}");
            AppError::Unauthorized
        })?;

    if !state.cfg.is_owner(&identity.email) {
        tracing::warn!(
            "CF Access JWT email {} not in owner allow-list",
            identity.email
        );
        return Err(AppError::Forbidden);
    }

    req.extensions_mut().insert(identity);
    Ok(next.run(req).await)
}

async fn verify(
    jwks: &JwksClient,
    team_domain: &str,
    expected_aud: &str,
    token: &str,
) -> Result<OwnerIdentity> {
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
    Ok(OwnerIdentity {
        email: data.claims.email,
    })
}
