use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ShareScope {
    View,
    Edit,
}

impl ShareScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            ShareScope::View => "view",
            ShareScope::Edit => "edit",
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
pub struct ShareLink {
    pub id: String,
    pub trip_id: String,
    pub label: Option<String>,
    pub scope: String, // 'view' | 'edit' — wire-validated
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateShareLink {
    pub label: Option<String>,
    pub scope: ShareScope,
}

/// Returned ONCE on creation, with the raw token visible.
#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedShareLink {
    #[serde(flatten)]
    pub link: ShareLink,
    pub token: String,
    pub url_path: String, // e.g. "/share/<token>" — convenience for the frontend
}

/// Returned to guests on GET /share/:token/trip. Embeds the trip detail plus
/// the scope the bearer has so the UI can hide write affordances for viewers.
#[derive(Debug, Serialize, ToSchema)]
pub struct SharedTripDetail {
    #[serde(flatten)]
    pub trip: crate::models::TripDetail,
    pub share_scope: ShareScope,
}
