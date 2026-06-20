use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Traveler {
    pub id: String,
    pub trip_id: String,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTraveler {
    pub name: String,
    #[serde(default)]
    pub sort_order: Option<i64>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct UpdateTraveler {
    pub name: Option<String>,
    pub sort_order: Option<i64>,
}
