//! The app's authentication guard. It is just [`nestrs_auth::AuthGuard`] bound to
//! this app's [`JwtStrategy`], so it verifies the bearer JWT and attaches the
//! [`AuthUser`](crate::authn::AuthUser). Bind it with `#[use_guards(AuthGuard)]`;
//! an `AbilityGuard` may follow it to build the caller's authorization.

use crate::authn::strategy::JwtStrategy;

pub type AuthGuard = nestrs_auth::AuthGuard<JwtStrategy>;
