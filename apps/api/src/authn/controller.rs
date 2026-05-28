//! `POST /auth/login` — the token issuer. The analog of a NestJS `AuthController`'s
//! `signIn`. The handler holds no logic: it adapts the request to [`AuthService`]
//! and the result back to JSON. Conversion is at the edge (`org_id` deserializes
//! straight to a `Uuid`); the business rule lives in the service.

use std::sync::Arc;

use nestrs_http::{controller, routes, Ctx};
use nestrs_throttler::{Throttle, ThrottlerGuard};
use poem::web::Json;
use poem::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::authn::oauth::OAuthGuard;
use crate::authn::principal::{AuthUser, Role};
use crate::authn::service::AuthService;
use crate::errors::internal;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LoginRequest {
    /// The org the issued token authorizes the caller within.
    pub org_id: Uuid,
    #[serde(default)]
    pub roles: Vec<Role>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LoginResponse {
    pub access_token: String,
}

#[controller(path = "/auth")]
pub struct AuthController {
    #[inject]
    auth: Arc<AuthService>,
}

#[routes]
impl AuthController {
    #[post("/login")]
    #[use_guards(ThrottlerGuard)]
    #[meta(Throttle::per_minute(5))]
    #[api(summary = "Issue a JWT access token (demo issuer)", tags("Auth"))]
    async fn login(&self, body: Json<LoginRequest>) -> Result<Json<LoginResponse>> {
        let LoginRequest { org_id, roles } = body.0;
        let access_token = self.auth.issue(org_id, roles).map_err(internal)?;
        Ok(Json(LoginResponse { access_token }))
    }

    #[get("/oauth")]
    #[use_guards(OAuthGuard)]
    #[api(
        summary = "Begin OAuth2 login (redirects to the provider)",
        tags("Auth")
    )]
    async fn oauth_begin(&self) {
        // Unreachable: with no `code`, OAuthGuard challenges with a 302 to the
        // provider before this handler runs.
    }

    #[get("/oauth/callback")]
    #[use_guards(OAuthGuard)]
    #[api(summary = "OAuth2 callback — issues the app's JWT", tags("Auth"))]
    async fn oauth_callback(&self, user: Ctx<AuthUser>) -> Result<Json<LoginResponse>> {
        let access_token = self
            .auth
            .issue(user.org_id, user.roles.clone())
            .map_err(internal)?;
        Ok(Json(LoginResponse { access_token }))
    }
}
