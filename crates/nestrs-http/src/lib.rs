//! HTTP transport for nestrs.
//!
//! [`HttpTransport`] is a [`nestrs_core::Transport`] backed by `poem` that
//! mounts every controller declared in the module tree (each emits an
//! [`HttpControllerMeta`] via the `#[routes]` macro), plus any extra
//! endpoints attached via [`HttpTransport::mount`]. The framework owns the
//! route assembly and the server lifecycle so apps don't have to.
//!
//! Apps interact with this crate through three entry points:
//! - [`HttpTransport`] — the transport itself (`new`, `bind`, `interceptor`,
//!   `mount`, …).
//! - [`Controller`] — implemented automatically by the `#[routes]` macro;
//!   you rarely impl it by hand.
//! - [`HttpControllerMeta`] / [`HttpInterceptorMeta`] — the discovery
//!   metadata that the `#[routes]` / `#[interceptor]` macros attach to a
//!   type; the transport reads them at boot via
//!   [`nestrs_core::DiscoveryService`].

mod controller;
mod interceptor;
mod transport;

pub use controller::{Controller, HttpControllerMeta, HttpRouteMeta, HttpVerb};
pub use interceptor::HttpInterceptorMeta;
pub use transport::HttpTransport;

pub use poem;
