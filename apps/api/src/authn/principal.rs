//! The authenticated caller — what authentication produces and authorization
//! consumes — plus the [`Claims`] shape the API signs into its JWTs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    User,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub org_id: Uuid,
    pub roles: Vec<Role>,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.roles.contains(&Role::Admin)
    }
}

/// The JWT payload this API issues and verifies. `exp` is required — `JwtService`
/// validates it on `verify`, so an expired token is rejected automatically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// The org the caller acts within; lifted into [`AuthUser::org_id`].
    pub org_id: Uuid,
    pub roles: Vec<Role>,
    /// Expiry, as a Unix timestamp.
    pub exp: u64,
}
