//! The authentication failure type, rendered as an HTTP challenge.

use poem::http::{header, StatusCode};
use poem::{IntoResponse, Response};

/// Why authentication did not establish an identity. A [`Strategy`](crate::Strategy)
/// returns it; [`AuthGuard`](crate::AuthGuard) renders it as a `401` with a
/// `WWW-Authenticate` header.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// No credential was presented (missing/blank `Authorization`, no session).
    #[error("missing credentials")]
    MissingCredentials,
    /// A credential was presented but did not verify (bad signature, malformed).
    #[error("invalid token")]
    InvalidToken,
    /// The credential verified but is past its `exp`.
    #[error("token expired")]
    Expired,
    /// Anything strategy-specific: a failed OAuth exchange, an unknown user, a
    /// mismatched CSRF state. The message is for logs, not the client body.
    #[error("authentication failed: {0}")]
    Failed(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::WWW_AUTHENTICATE, "Bearer")
            .body(self.to_string())
    }
}
