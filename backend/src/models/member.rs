use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use super::ShareScope;

/// A signed-in identity's effective access to one trip. `Owner` is the
/// `trips.owner_email` holder; `Edit`/`View` come from a `trip_members` row and
/// mirror the share-link scopes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TripRole {
    Owner,
    Edit,
    View,
}

impl TripRole {
    /// Owners and editors may change trip content (items, notes, todos,
    /// travelers, trip fields). Viewers may not.
    pub fn can_edit(self) -> bool {
        matches!(self, TripRole::Owner | TripRole::Edit)
    }

    /// Owner-only surface: membership, share links, email senders, deletion.
    pub fn is_owner(self) -> bool {
        matches!(self, TripRole::Owner)
    }

    /// Parse the stored/computed role string. `owner` is synthesised by the
    /// role query; `view`/`edit` come from `trip_members.role`.
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "owner" => Some(TripRole::Owner),
            "edit" => Some(TripRole::Edit),
            "view" => Some(TripRole::View),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
pub struct TripMember {
    pub trip_id: String,
    pub email: String,
    pub role: String, // 'view' | 'edit' — wire-validated, CHECK-constrained
    pub created_at: String,
}

/// Add a member, or change an existing member's role (upsert on email).
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertTripMember {
    pub email: String,
    pub role: ShareScope,
}
