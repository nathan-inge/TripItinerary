use axum::{extract::Extension, routing::get, Json, Router};
use serde::Serialize;
use utoipa::ToSchema;

use crate::auth::Identity;
use crate::AppState;

#[derive(Serialize, ToSchema)]
pub struct Me {
    pub email: String,
    /// False when the address authenticated through Cloudflare Access but has
    /// no grant in this app — no owner-allow-list entry, no trip membership,
    /// no trip of its own. Every other authenticated route 403s for them, so
    /// the SPA uses this to explain rather than to show an error.
    pub provisioned: bool,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/me", get(me))
}

/// Return the caller's identity. Behind `require_identity` only, so it also
/// answers for an authenticated address with no access yet.
pub async fn me(Extension(me): Extension<Identity>) -> Json<Me> {
    Json(Me {
        email: me.email,
        provisioned: me.provisioned,
    })
}
