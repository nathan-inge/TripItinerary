use axum::{extract::Extension, routing::get, Json, Router};
use serde::Serialize;
use utoipa::ToSchema;

use crate::auth::OwnerIdentity;
use crate::AppState;

#[derive(Serialize, ToSchema)]
pub struct Me {
    pub email: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/me", get(me))
}

/// Return the authenticated owner's identity. Behind `require_owner`, so the
/// `OwnerIdentity` extension is always present.
pub async fn me(Extension(owner): Extension<OwnerIdentity>) -> Json<Me> {
    Json(Me {
        email: owner.email,
    })
}
