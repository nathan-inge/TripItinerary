pub mod cf_access;
pub mod share_token;

pub use cf_access::{require_identity, require_provisioned, Identity};

use crate::models::ShareScope;

#[derive(Clone, Debug)]
pub struct ScopedAccess {
    pub trip_id: String,
    pub scope: ShareScope,
}
