//! HTTP surface bindings for [`nestrs_authz`].
//!
//! `nestrs-authz` is the transport-agnostic authorization engine; this crate is
//! its poem binding, mirroring how `nestrs-http`'s `Valid`/`Piped` bind the pure
//! `nestrs-pipes`. Splitting it out keeps `poem` out of the engine and `sea-orm`
//! (the masking deserializes into `EntityTrait::Model`) out of the generic HTTP
//! crate — each side keeps a single responsibility.
//!
//! Three pieces, in request order:
//! - [`AbilityGuard<F>`] — the per-route guard that builds the request `Ability`
//!   from the actor an authentication guard attached.
//! - [`Authorize<A, S>`] — the access gate (a poem extractor): `403` unless the
//!   ability grants action `A` on subject `S`.
//! - [`Authorize`]'s `RouteResponseShaper` impl (in `shape`) — `#[routes]` masks
//!   the response to the fields and rows the ability permits, with no `mask` call
//!   in the handler.

mod extractor;
mod guard;
mod shape;

pub use extractor::Authorize;
pub use guard::AbilityGuard;
