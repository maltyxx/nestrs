//! The app's OAuth2 authentication strategy (a GitHub-style provider), and the
//! guard that drives it. One [`Strategy`] bound to both `/auth/oauth` and
//! `/auth/oauth/callback`, exactly like Passport's `AuthGuard('github')`: with no
//! `code` it challenges the browser with a redirect to the provider; on the
//! callback it trades the `code` for an access token, reads the profile, and
//! produces the [`AuthUser`].
//!
//! The CSRF state + PKCE verifier ride between the two legs in the `oauth_tx`
//! cookie, a short JWT signed by the app's `JwtService` (see [`nestrs_auth`]).

use std::sync::Arc;

use nestrs_auth::{AuthError, JwtService, OAuth2Client, Outcome, Strategy};
use nestrs_core::injectable;
use nestrs_http::async_trait;
use poem::http::{header, StatusCode};
use poem::{Request, Response};
use serde::Deserialize;
use uuid::Uuid;

use crate::authn::principal::{AuthUser, Role};

/// The cookie carrying the signed OAuth transaction across the redirect.
const TRANSACTION_COOKIE: &str = "oauth_tx";

/// The query a provider appends to the callback URL.
#[derive(Debug, Default, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

#[injectable]
pub struct OAuthStrategy {
    #[inject]
    jwt: Arc<JwtService>,
    #[inject]
    oauth: Arc<OAuth2Client>,
}

#[async_trait]
impl Strategy for OAuthStrategy {
    type Principal = AuthUser;

    async fn authenticate(&self, req: &mut Request) -> Result<Outcome<AuthUser>, AuthError> {
        let query: CallbackQuery = req.params().unwrap_or_default();

        let Some(code) = query.code else {
            // Initiating leg: redirect to the provider, stashing the transaction.
            let authorization = self.oauth.authorize(&self.jwt)?;
            let redirect = Response::builder()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, authorization.url)
                .header(
                    header::SET_COOKIE,
                    format!(
                        "{TRANSACTION_COOKIE}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=600",
                        authorization.transaction
                    ),
                )
                .finish();
            return Ok(Outcome::Challenge(redirect));
        };

        // Callback leg: validate state against the cookie, exchange, identify.
        let state = query
            .state
            .ok_or_else(|| AuthError::Failed("OAuth callback missing state".into()))?;
        let transaction = transaction_cookie(req)
            .ok_or_else(|| AuthError::Failed("OAuth transaction cookie missing".into()))?;
        let access_token = self
            .oauth
            .exchange(&self.jwt, &transaction, &state, &code)
            .await?;

        // The profile fetch proves the access token; a real app reads the caller's
        // identity and org from it. The demo issues a token for a fixed org so the
        // example stays self-contained.
        let _profile: serde_json::Value = self.oauth.userinfo(&access_token).await?;
        Ok(Outcome::Authenticated(AuthUser {
            org_id: Uuid::from_u128(0x018f_0000_0000_7000_8000_0000_0000_0001),
            roles: vec![Role::User],
        }))
    }
}

/// Read the `oauth_tx` cookie value out of the request's `Cookie` header. The
/// value is a JWT (base64url + `.`), all cookie-safe characters, so a plain split
/// is enough.
fn transaction_cookie(req: &Request) -> Option<String> {
    let header = req.headers().get(header::COOKIE)?.to_str().ok()?;
    header.split(';').find_map(|pair| {
        let pair = pair.trim();
        pair.strip_prefix(TRANSACTION_COOKIE)?
            .strip_prefix('=')
            .map(str::to_owned)
    })
}

/// The app's OAuth guard: [`nestrs_auth::AuthGuard`] bound to [`OAuthStrategy`].
pub type OAuthGuard = nestrs_auth::AuthGuard<OAuthStrategy>;
