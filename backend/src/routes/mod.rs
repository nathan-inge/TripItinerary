pub mod day_notes;
pub mod email_senders;
pub mod health;
pub mod intake;
pub mod items;
pub mod share;
pub mod shared;
pub mod travelers;
pub mod trips;

use axum::{middleware, Router};

use crate::auth;
use crate::AppState;

pub fn api_router(state: AppState) -> Router {
    // Health is unauthenticated (used by deploys / curl checks).
    let public = Router::new().merge(health::router());

    // Owner routes — CF Access JWT (or DEV_BYPASS_AUTH=1).
    let owner = Router::new()
        .merge(trips::router())
        .merge(travelers::router())
        .merge(items::router())
        .merge(day_notes::router())
        .merge(share::router())
        .merge(email_senders::router())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_owner,
        ));

    // Guest share-token routes — token in path, auth happens in shared::middleware.
    let shared_routes = shared::router(state.clone());

    // Inbound email intake — HMAC-authed via the Cloudflare Email Worker.
    let intake_routes = intake::router();

    public
        .merge(owner)
        .merge(shared_routes)
        .merge(intake_routes)
        .with_state(state)
}
