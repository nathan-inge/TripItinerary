pub mod day_notes;
pub mod email_senders;
pub mod health;
pub mod intake;
pub mod items;
pub mod me;
pub mod members;
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

    // Routes for a signed-in identity. Two layers: `require_identity` verifies
    // the caller per TRIPIT_AUTH_MODE (CF Access JWT, proxy header, or none), and `require_provisioned`
    // rejects an authenticated address the app has no grant for. `/me` sits
    // outside the second layer so the SPA can tell a signed-in-but-not-invited
    // visitor what's wrong instead of showing a bare 403.
    //
    // Per-trip authorization is the role resolver in `trips::require_trip_*`:
    // owners get everything, edit members get trip content, view members read.
    let provisioned = Router::new()
        .merge(trips::router())
        .merge(travelers::router())
        .merge(items::router())
        .merge(day_notes::router())
        .merge(share::router())
        .merge(members::router())
        .merge(email_senders::router())
        .route_layer(middleware::from_fn(auth::require_provisioned));

    let authed = Router::new()
        .merge(me::router())
        .merge(provisioned)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_identity,
        ));

    // Guest share-token routes — token in path, auth happens in shared::middleware.
    let shared_routes = shared::router(state.clone());

    // Inbound email intake — HMAC-authed via the Cloudflare Email Worker.
    let intake_routes = intake::router();

    public
        .merge(authed)
        .merge(shared_routes)
        .merge(intake_routes)
        .with_state(state)
}
