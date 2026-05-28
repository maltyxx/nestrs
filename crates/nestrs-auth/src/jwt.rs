//! [`JwtService`] — sign and verify JSON Web Tokens. The analog of `@nestjs/jwt`'s
//! `JwtService`, and (like that one) a thin wrapper over the standard JWT engine.
//!
//! Configured once via [`AuthModule::for_root`](crate::AuthModule::for_root) and
//! injected as `Arc<JwtService>` anywhere — a login handler signs the token an
//! authenticated caller carries, a [`Strategy`](crate::Strategy) verifies it.

use std::time::Duration;

use jsonwebtoken::{
    decode, encode, errors::ErrorKind, get_current_timestamp, Algorithm, DecodingKey, EncodingKey,
    Header, Validation,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::error::AuthError;

/// How [`JwtService`] signs. Built at the import site and handed to
/// [`AuthModule::for_root`](crate::AuthModule::for_root).
///
/// Only the HMAC algorithms (`HS256`/`HS384`/`HS512`) are wired today — they key
/// off the shared `secret`. RSA/ECDSA (`RS*`/`ES*`) need a PEM key pair and are a
/// future addition.
#[derive(Clone)]
pub struct JwtOptions {
    /// The shared secret backing the HMAC signature.
    pub secret: String,
    /// The signing algorithm. Defaults to `HS256`.
    pub algorithm: Algorithm,
    /// How long a freshly minted token stays valid; surfaced by [`JwtService::expiry`].
    pub expires_in: Duration,
}

impl JwtOptions {
    /// `HS256`, one-hour expiry — override the fields as needed.
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            algorithm: Algorithm::HS256,
            expires_in: Duration::from_secs(3600),
        }
    }
}

/// Signs and verifies tokens for the app. Cloneable keys are precomputed once.
pub struct JwtService {
    encoding: EncodingKey,
    decoding: DecodingKey,
    header: Header,
    validation: Validation,
    expires_in: Duration,
}

impl JwtService {
    /// Precompute the keys from [`JwtOptions`]. Infallible for HMAC secrets.
    pub fn new(options: JwtOptions) -> Self {
        let secret = options.secret.as_bytes();
        let mut validation = Validation::new(options.algorithm);
        // No audience contract by default; an app that wants one sets it on its
        // claims and we can expose `aud` configuration when it is needed.
        validation.validate_aud = false;
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
            header: Header::new(options.algorithm),
            validation,
            expires_in: options.expires_in,
        }
    }

    /// Sign `claims` into a compact JWT. `claims` must carry an `exp` (use
    /// [`expiry`](Self::expiry)); the rest of the shape is the app's to define.
    pub fn sign<C: Serialize>(&self, claims: &C) -> Result<String, AuthError> {
        encode(&self.header, claims, &self.encoding).map_err(|e| AuthError::Failed(e.to_string()))
    }

    /// Verify a token and deserialize its claims. Validates the signature and
    /// `exp`; maps an expired token to [`AuthError::Expired`] and anything else
    /// to [`AuthError::InvalidToken`].
    pub fn verify<C: DeserializeOwned>(&self, token: &str) -> Result<C, AuthError> {
        decode::<C>(token, &self.decoding, &self.validation)
            .map(|data| data.claims)
            .map_err(|e| match e.kind() {
                ErrorKind::ExpiredSignature => AuthError::Expired,
                _ => AuthError::InvalidToken,
            })
    }

    /// The Unix timestamp `now + expires_in` — set it as the `exp` claim when
    /// signing so the token expires per the configured lifetime.
    pub fn expiry(&self) -> u64 {
        get_current_timestamp() + self.expires_in.as_secs()
    }
}
