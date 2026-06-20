pub mod cf_access;
pub mod share_token;

pub use cf_access::{require_owner, OwnerIdentity};

use crate::models::ShareScope;

#[derive(Clone, Debug)]
pub struct ScopedAccess {
    pub trip_id: String,
    pub scope: ShareScope,
}
