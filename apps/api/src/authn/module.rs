use nestrs_core::module;
use nestrs_throttler::ThrottlerGuard;

use crate::authn::controller::AuthController;
use crate::authn::guard::AuthGuard;
use crate::authn::oauth::{OAuthGuard, OAuthStrategy};
use crate::authn::service::AuthService;
use crate::authn::strategy::JwtStrategy;

#[module(providers = [
    JwtStrategy,
    AuthGuard,
    OAuthStrategy,
    OAuthGuard,
    ThrottlerGuard,
    AuthService,
    AuthController,
])]
pub struct AuthnModule;
