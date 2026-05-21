use anyhow::Result;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::container::Container;

/// A `Transport` is anything that accepts inbound requests on behalf of the
/// app — an HTTP server, an MCP-over-stdio loop, a gRPC server, …
///
/// The trait is intentionally thin: lifecycle (`configure`, `serve`) only.
/// It does **not** define a message-pattern matcher, retry policy, or ack
/// semantics — those are concerns of the specific protocol/SDK and live in
/// the transport's own crate, not in this core abstraction.
///
/// Boot sequence orchestrated by [`crate::App::run`]:
///
/// 1. `configure(&container)` is awaited on each transport, in registration
///    order. A transport reads from the container's
///    [`registry`](Container::registry) here to discover its surfaces.
/// 2. Each transport's `serve` future is spawned with a shared
///    [`CancellationToken`]. SIGTERM/SIGINT cancels the token; transports
///    must observe it and shut down gracefully.
#[async_trait]
pub trait Transport: Send + Sync + 'static {
    async fn configure(&mut self, container: &Container) -> Result<()>;
    async fn serve(self: Box<Self>, cancel: CancellationToken) -> Result<()>;
}
