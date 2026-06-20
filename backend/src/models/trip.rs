use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use super::{DayNote, Item, Traveler};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Trip {
    pub id: String,
    pub owner_email: String,
    pub name: String,
    pub destination: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub notes_md: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TripDetail {
    #[serde(flatten)]
    pub trip: Trip,
    pub travelers: Vec<Traveler>,
    pub items: Vec<Item>,
    pub day_notes: Vec<DayNote>,
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
    pub destination: Option<Option<String>>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub notes_md: Option<String>,
}
