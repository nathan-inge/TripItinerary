use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct TripEmailSender {
    pub trip_id: String,
    pub email: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTripEmailSender {
    pub email: String,
}
