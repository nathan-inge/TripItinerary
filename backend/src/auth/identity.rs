use axum::{
    extract::State,
    http::{header::HeaderMap, HeaderName, Request},
    middleware::Next,
    response::Response,
};

use crate::config::AuthMode;
use crate::error::AppError;
use crate::AppState;

use super::cf_access;

/// A signed-in caller. `email` is lowercased so it compares cleanly against
/// the `trip_members` / owner allow-lists.
///
/// `provisioned` means "this address is known to the app" — it is in
/// `OWNER_EMAIL`, holds a `trip_members` row, or owns a trip. The auth layer
/// in front (Cloudflare Access, a reverse proxy) is the outer gate, but we
/// keep this app-level check so a widened outer policy can't silently hand an
/// unknown address a working account.
#[derive(Clone, Debug)]
pub struct Identity {
    pub email: String,
    pub provisioned: bool,
}

/// Authenticate the caller per [`AuthMode`] and attach an [`Identity`]. Does
/// *not* reject unprovisioned callers — `/api/me` is reachable without a
/// grant so the SPA can tell a signed-in-but-not-invited person what's wrong.
/// Every other authenticated route sits behind [`require_provisioned`].
pub async fn require_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    let email = match &state.cfg.auth_mode {
        AuthMode::None => {
            req.extensions_mut().insert(Identity {
                email: state.cfg.primary_owner().to_string(),
                provisioned: true,
            });
            return Ok(next.run(req).await);
        }
        AuthMode::CloudflareAccess => cf_access::authenticate(&state, &headers).await?,
        AuthMode::TrustedHeader(name) => header_email(&headers, name)?,
    };

    let provisioned = is_provisioned(&state, &email).await?;
    if !provisioned {
        tracing::warn!(
            "authenticated email {} has no owner-allow-list entry, trip membership, \
             or owned trip; only /api/me is reachable",
            email
        );
    }

    req.extensions_mut().insert(Identity { email, provisioned });
    Ok(next.run(req).await)
}

/// Read the caller's email from a header set by the authenticating proxy.
fn header_email(headers: &HeaderMap, name: &HeaderName) -> Result<String, AppError> {
    let email = headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| v.contains('@'))
        .ok_or(AppError::Unauthorized)?;
    Ok(email)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn hdrs(name: &'static str, value: &'static str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(name, value.parse().unwrap());
        h
    }

    #[test]
    fn header_email_lowercases_and_trims() {
        let name = HeaderName::from_static("x-forwarded-email");
        let h = hdrs("x-forwarded-email", "  Owner@Example.com ");
        assert_eq!(header_email(&h, &name).unwrap(), "owner@example.com");
    }

    #[test]
    fn header_email_rejects_missing_or_non_email() {
        let name = HeaderName::from_static("x-forwarded-email");
        assert!(header_email(&HeaderMap::new(), &name).is_err());
        assert!(header_email(&hdrs("x-forwarded-email", "alice"), &name).is_err());
        assert!(header_email(&hdrs("x-other", "a@b.com"), &name).is_err());
    }
}
