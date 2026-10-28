use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use super::{DayNote, Item, Traveler, TripEmailSender, TripRole};

/// Deserialize `Option<Option<T>>` such that an explicit `null` → `Some(None)`
/// (meaning "clear this field") and an absent key → `None` ("leave alone").
fn deserialize_explicit_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct TripRow {
    pub id: String,
    pub owner_email: String,
    pub name: String,
    pub destination: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub notes_md: String,
    pub photos_url: Option<String>,
    pub maps_url: Option<String>,
    pub todos_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Trip {
    pub id: String,
    pub owner_email: String,
    pub name: String,
    pub destination: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub notes_md: String,
    pub photos_url: Option<String>,
    pub maps_url: Option<String>,
    pub todos: Vec<TodoItem>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<TripRow> for Trip {
    fn from(r: TripRow) -> Self {
        let todos: Vec<TodoItem> = serde_json::from_str(&r.todos_json).unwrap_or_default();
        Trip {
            id: r.id,
            owner_email: r.owner_email,
            name: r.name,
            destination: r.destination,
            start_date: r.start_date,
            end_date: r.end_date,
            notes_md: r.notes_md,
            photos_url: r.photos_url,
            maps_url: r.maps_url,
            todos,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// One row of `GET /trips` — the trip plus the caller's effective role, so the
/// home page can label and gate trips shared with them.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TripSummary {
    #[serde(flatten)]
    pub trip: Trip,
    pub my_role: TripRole,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TripDetail {
    #[serde(flatten)]
    pub trip: Trip,
    pub travelers: Vec<Traveler>,
    pub items: Vec<Item>,
    pub day_notes: Vec<DayNote>,
    /// Owner-level config; empty for members and share-link guests.
    pub email_senders: Vec<TripEmailSender>,
    /// The caller's access to this trip, so the UI can hide what they can't do.
    pub my_role: TripRole,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTrip {
    pub name: String,
    pub destination: Option<String>,
    pub start_date: String,
    pub end_date: String,
    #[serde(default)]
    pub notes_md: Option<String>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct UpdateTrip {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_option")]
    pub destination: Option<Option<String>>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub notes_md: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_option")]
    pub photos_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_option")]
    pub maps_url: Option<Option<String>>,
    pub todos: Option<Vec<TodoItem>>,
}
