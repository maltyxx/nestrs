//! The [`Throttle`] limit — both the module-wide default and the per-route
//! override an app attaches with `#[meta(Throttle::...)]`.

use std::time::Duration;

/// A rate limit: at most `limit` requests per `window`, per client.
///
/// Pass one to [`ThrottlerModule::for_root`](crate::ThrottlerModule::for_root) as
/// the default, and/or attach one to a route with `#[meta(Throttle::...)]` to
/// override that default for the route (read back by [`ThrottlerGuard`] via the
/// `Reflector`). It is `Copy`, so the guard reads it without cloning.
#[derive(Clone, Copy, Debug)]
pub struct Throttle {
    pub limit: u32,
    pub window: Duration,
}

impl Throttle {
    pub const fn new(limit: u32, window: Duration) -> Self {
        Self { limit, window }
    }

    /// `limit` requests per minute.
    pub const fn per_minute(limit: u32) -> Self {
        Self::new(limit, Duration::from_secs(60))
    }

    /// `limit` requests per second.
    pub const fn per_second(limit: u32) -> Self {
        Self::new(limit, Duration::from_secs(1))
    }
}
