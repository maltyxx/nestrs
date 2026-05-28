//! Rate limiting for nestrs — the `@nestjs/throttler` analog.
//!
//! Import [`ThrottlerModule::for_root`] with a default [`Throttle`], then bind
//! [`ThrottlerGuard`] to a route with `#[use_guards(ThrottlerGuard)]`. Override the
//! default for a route with `#[meta(Throttle::...)]`; the guard reads it back via
//! the `Reflector`. A request over the limit gets `429 Too Many Requests` before
//! the handler runs.
//!
//! Backed by an in-memory fixed-window counter ([`InMemoryThrottler`]); a
//! Redis-backed store for multi-process deployments is a future addition.

mod guard;
mod module;
mod store;
mod throttle;

pub use guard::ThrottlerGuard;
pub use module::{ThrottlerModule, ThrottlerSetup};
pub use store::{Decision, InMemoryThrottler};
pub use throttle::Throttle;
