use async_trait::async_trait;
use poem::http::{HeaderMap, Method, Uri};
use poem::{Endpoint, IntoResponse, Request, Response, Result};

/// Read-only view of the request handed to a [`Filter`]. The original
/// `poem::Request` has been consumed by the inner endpoint by the time the
/// filter runs (it isn't `Clone`), so we capture the routing-relevant bits
/// up front.
#[derive(Debug, Clone)]
pub struct RequestSnapshot {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
}

impl RequestSnapshot {
    pub(crate) fn from_req(req: &Request) -> Self {
        Self {
            method: req.method().clone(),
            uri: req.uri().clone(),
            headers: req.headers().clone(),
        }
    }
}

/// A `Filter` converts errors produced by the inner endpoint into a
/// response. Use this to map domain errors / panics-as-errors to a uniform
/// HTTP shape (problem+json, error envelope, etc.).
///
/// Filters run only on the error path — successful responses pass through.
///
/// ```ignore
/// struct DomainErrorMapper;
///
/// #[async_trait::async_trait]
/// impl nestrs_middleware::Filter for DomainErrorMapper {
///     async fn filter(&self, _req: &RequestSnapshot, err: poem::Error) -> Response {
///         Response::builder()
///             .status(err.status())
///             .body(format!("{{\"error\":\"{}\"}}", err))
///     }
/// }
/// ```
#[async_trait]
pub trait Filter: Send + Sync + 'static {
    async fn filter(&self, req: &RequestSnapshot, error: poem::Error) -> Response;
}

#[async_trait]
impl<T: Filter + ?Sized> Filter for std::sync::Arc<T> {
    async fn filter(&self, req: &RequestSnapshot, error: poem::Error) -> Response {
        (**self).filter(req, error).await
    }
}

/// Endpoint wrapper produced by [`EndpointExt::filter`](crate::EndpointExt::filter).
pub struct FilterEndpoint<E, F> {
    inner: E,
    filter: F,
}

impl<E, F> FilterEndpoint<E, F> {
    pub fn new(inner: E, filter: F) -> Self {
        Self { inner, filter }
    }
}

impl<E, F> Endpoint for FilterEndpoint<E, F>
where
    E: Endpoint + Send + Sync,
    E::Output: IntoResponse,
    F: Filter,
{
    type Output = Response;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        let snapshot = RequestSnapshot::from_req(&req);
        match self.inner.call(req).await {
            Ok(out) => Ok(out.into_response()),
            Err(err) => Ok(self.filter.filter(&snapshot, err).await),
        }
    }
}
