//! Minimal OpenAPI spec for the tripit REST API.
//!
//! Paths are not yet annotated with `#[utoipa::path]` on each handler — the
//! published spec exposes the request/response *schemas* so clients (and
//! humans) can see the data shapes. Path entries can be filled in as a
//! follow-up.

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "TripItinerary",
        version = "0.1.0",
        description = "Self-hosted trip itinerary API. \
                       Authenticate as the owner via Cloudflare Access JWT in \
                       the `Cf-Access-Jwt-Assertion` header, or as a guest via \
                       a share token in the URL path (`/api/share/<token>/...`)."
    ),
    components(schemas(
        crate::models::Trip,
        crate::models::TripDetail,
        crate::models::CreateTrip,
        crate::models::UpdateTrip,
        crate::models::Traveler,
        crate::models::CreateTraveler,
        crate::models::UpdateTraveler,
        crate::models::Item,
        crate::models::CreateItem,
        crate::models::UpdateItem,
        crate::models::ItemPreview,
        crate::models::ItemType,
        crate::models::ItemSource,
        crate::models::ParseStatus,
        crate::models::Link,
        crate::models::ParseTextRequest,
        crate::models::ParseTextResponse,
        crate::models::DayNote,
        crate::models::UpsertDayNote,
        crate::models::ShareLink,
        crate::models::ShareScope,
        crate::models::CreateShareLink,
        crate::models::CreatedShareLink,
        crate::models::SharedTripDetail,
    ))
)]
pub struct ApiDoc;
