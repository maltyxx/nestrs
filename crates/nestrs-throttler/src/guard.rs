//! [`ThrottlerGuard`] — the per-route rate-limiting guard, the `@nestjs/throttler`
//! `ThrottlerGuard` analog.

use std::sync::Arc;

use nestrs_core::injectable;
use nestrs_http::{async_trait, Guard, Reflector};
use poem::http::{header, StatusCode};
use poem::{Request, Response};

use crate::store::InMemoryThrottler;
use crate::throttle::Throttle;

/// Bind it per route with `#[use_guards(ThrottlerGuard)]`. It applies the module
/// default ([`ThrottlerModule::for_root`](crate::ThrottlerModule::for_root)) unless
/// the route overrides it with `#[meta(Throttle::...)]`, which the guard reads via
/// the [`Reflector`]. A request over the limit is rejected with
/// `429 Too Many Requests` and a `Retry-After` header — it never reaches the
/// handler.
///
/// It must be a per-route guard, not a global one: a global guard runs before
/// routing, so the route's `#[meta(Throttle)]` is not yet attached.
#[injectable]
pub struct ThrottlerGuard {
    #[inject]
    throttler: Arc<InMemoryThrottler>,
}

#[async_trait]
impl Guard for ThrottlerGuard {
    async fn check(&self, req: &mut Request) -> Result<(), Response> {
        let limit = Reflector::new(req)
            .get::<Throttle>()
            .copied()
            .unwrap_or_else(|| self.throttler.default_limit());

        let decision = self.throttler.hit(&client_key(req), limit);
        if decision.allowed {
            return Ok(());
        }
        Err(Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(
                header::RETRY_AFTER,
                decision.retry_after.as_secs().to_string(),
            )
            .body("Too Many Requests"))
    }
}

/// The rate-limit key: the first `X-Forwarded-For` hop when proxied, else the peer
/// **IP** (never the `ip:port` — each new connection gets a fresh ephemeral port,
/// so keying on the port would give every request its own bucket and never limit
/// anything), else a shared key.
fn client_key(req: &Request) -> String {
    if let Some(forwarded) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|chain| chain.split(',').next())
        .map(str::trim)
        .filter(|ip| !ip.is_empty())
    {
        return forwarded.to_owned();
    }
    match req.remote_addr().as_socket_addr() {
        Some(addr) => addr.ip().to_string(),
        None => "global".to_owned(),
    }
}
