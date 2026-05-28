//! The app's authentication business logic — issuing session tokens. Kept out of
//! the controller, which only adapts HTTP to this service.

use std::sync::Arc;

use nestrs_auth::{AuthError, JwtService};
use nestrs_core::injectable;
use uuid::Uuid;

use crate::authn::principal::{Claims, Role};

#[injectable]
pub struct AuthService {
    #[inject]
    jwt: Arc<JwtService>,
}

impl AuthService {
    /// Mint a bearer token for a caller: build the claims, stamp the configured
    /// expiry, and sign. A real app would gate this behind a credential check.
    pub fn issue(&self, org_id: Uuid, roles: Vec<Role>) -> Result<String, AuthError> {
        self.jwt.sign(&Claims {
            org_id,
            roles,
            exp: self.jwt.expiry(),
        })
    }
}
