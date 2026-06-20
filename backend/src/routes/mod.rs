pub mod day_notes;
pub mod health;
pub mod items;
pub mod travelers;
pub mod trips;

use axum::{middleware, Router};

use crate::auth;
use crate::AppState;

pub fn api_router(state: AppState) -> Router {
    // Health is unauthenticated (used by deploys / curl checks).
    let public = Router::new().merge(health::router());

    // Everything else requires owner auth (CF Access JWT or DEV_BYPASS_AUTH).
    let protected = Router::new()
        .merge(trips::router())
        .merge(travelers::router())
        .merge(items::router())
        .merge(day_notes::router())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_owner,
        ));

    public
        .merge(protected)
        .with_state(state)
}
