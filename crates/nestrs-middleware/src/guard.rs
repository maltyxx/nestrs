use async_trait::async_trait;
use poem::{Endpoint, IntoResponse, Request, Response, Result};

/// A `Guard` runs before the handler and decides whether the request is
/// allowed through. Returning `Err(response)` short-circuits the chain with
/// that response — typically a 401/403/429 — so the handler never runs.
///
/// ```ignore
/// struct RequireAuth;
///
/// #[async_trait::async_trait]
/// impl nestrs_middleware::Guard for RequireAuth {
///     async fn check(&self, req: &Request) -> Result<(), Response> {
///         if req.headers().contains_key("authorization") {
///             Ok(())
///         } else {
///             Err(Response::builder()
///                 .status(StatusCode::UNAUTHORIZED)
///                 .body("missing token"))
///         }
///     }
/// }
/// ```
#[async_trait]
pub trait Guard: Send + Sync + 'static {
    async fn check(&self, req: &Request) -> std::result::Result<(), Response>;
}

#[async_trait]
impl<T: Guard + ?Sized> Guard for std::sync::Arc<T> {
    async fn check(&self, req: &Request) -> std::result::Result<(), Response> {
        (**self).check(req).await
    }
}

/// Endpoint wrapper produced by [`EndpointExt::guard`](crate::EndpointExt::guard).
pub struct GuardEndpoint<E, G> {
    inner: E,
    guard: G,
}

impl<E, G> GuardEndpoint<E, G> {
    pub fn new(inner: E, guard: G) -> Self {
        Self { inner, guard }
    }
}

impl<E, G> Endpoint for GuardEndpoint<E, G>
where
    E: Endpoint + Send + Sync,
    E::Output: IntoResponse,
    G: Guard,
{
    type Output = Response;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        match self.guard.check(&req).await {
            Ok(()) => self.inner.call(req).await.map(IntoResponse::into_response),
            Err(response) => Ok(response),
        }
    }
}
