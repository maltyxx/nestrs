//! The app's bearer-token authentication strategy: verify the `Authorization:
//! Bearer <jwt>` and turn its [`Claims`] into the [`AuthUser`] the rest of the
//! request reads.

use std::sync::Arc;

use nestrs_auth::{bearer_token, AuthError, JwtService, Outcome, Strategy};
use nestrs_core::injectable;
use nestrs_http::async_trait;
use poem::Request;

use crate::authn::principal::{AuthUser, Claims};

#[injectable]
pub struct JwtStrategy {
    #[inject]
    jwt: Arc<JwtService>,
}

#[async_trait]
impl Strategy for JwtStrategy {
    type Principal = AuthUser;

    async fn authenticate(&self, req: &mut Request) -> Result<Outcome<AuthUser>, AuthError> {
        let token = bearer_token(req).ok_or(AuthError::MissingCredentials)?;
        let claims: Claims = self.jwt.verify(token)?;
        Ok(Outcome::Authenticated(AuthUser {
            org_id: claims.org_id,
            roles: claims.roles,
        }))
    }
}
