use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use nestrs_core::{Container, DiscoveryService, Transport};
use nestrs_middleware::{EndpointExt as NestrsEndpointExt, Filter, Guard, Interceptor};
use poem::endpoint::BoxEndpoint;
use poem::listener::TcpListener;
use poem::{EndpointExt, IntoEndpoint, Response, Route, Server};
use tokio_util::sync::CancellationToken;

use crate::controller::HttpControllerMeta;
use crate::interceptor::HttpInterceptorMeta;

type MountFn = Box<dyn Fn(&Container, Route) -> Route + Send + Sync>;

/// HTTP [`Transport`] backed by poem. Built up imperatively in the app's
/// `main.rs`, attached to an [`nestrs_core::App`], and configured by it.
///
/// At [`Transport::configure`] time, the transport queries the container's
/// [`DiscoveryService`] for every [`HttpControllerMeta`] declared via
/// `#[module(providers = [...])]`, mounts them, then mounts any extra
/// endpoints registered via [`HttpTransport::mount`] (GraphQL playground,
/// MCP streamable HTTP, etc.), then folds the interceptor / guard / filter
/// chain around the assembled route.
pub struct HttpTransport {
    bind: String,
    interceptors: Vec<Arc<dyn Interceptor>>,
    guards: Vec<Arc<dyn Guard>>,
    filters: Vec<Arc<dyn Filter>>,
    mounts: Vec<MountFn>,
    endpoint: Option<BoxEndpoint<'static, Response>>,
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpTransport {
    pub fn new() -> Self {
        Self {
            bind: "0.0.0.0:3000".into(),
            interceptors: Vec::new(),
            guards: Vec::new(),
            filters: Vec::new(),
            mounts: Vec::new(),
            endpoint: None,
        }
    }

    pub fn bind(mut self, addr: impl Into<String>) -> Self {
        self.bind = addr.into();
        self
    }

    pub fn interceptor<I: Interceptor>(mut self, interceptor: I) -> Self {
        self.interceptors.push(Arc::new(interceptor));
        self
    }

    pub fn guard<G: Guard>(mut self, guard: G) -> Self {
        self.guards.push(Arc::new(guard));
        self
    }

    pub fn filter<F: Filter>(mut self, filter: F) -> Self {
        self.filters.push(Arc::new(filter));
        self
    }

    /// Mount an extra endpoint at `path`. The builder closure runs at
    /// [`Transport::configure`] time with the live container, so it can
    /// resolve services to construct framework-specific endpoints (a
    /// GraphQL schema built from container-resolved resolvers, an MCP
    /// streamable HTTP service, …).
    pub fn mount<F, E>(mut self, path: impl Into<String>, build: F) -> Self
    where
        F: Fn(&Container) -> E + Send + Sync + 'static,
        E: IntoEndpoint,
        E::Endpoint: 'static,
        <E::Endpoint as poem::Endpoint>::Output: poem::IntoResponse,
    {
        let path = path.into();
        self.mounts.push(Box::new(move |container, route| {
            let endpoint = build(container).into_endpoint().map_to_response().boxed();
            route.nest(path.clone(), endpoint)
        }));
        self
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn configure(&mut self, container: &Container) -> Result<()> {
        let discovery = DiscoveryService::new(container);
        let mut route = Route::new();

        for d in discovery.meta::<HttpControllerMeta>() {
            route = d.meta.mount(container, route);
        }
        for mount in self.mounts.drain(..) {
            route = mount(container, route);
        }

        let mut endpoint: BoxEndpoint<'static, Response> = route.map_to_response().boxed();
        for filter in self.filters.drain(..) {
            endpoint = NestrsEndpointExt::filter(endpoint, filter)
                .map_to_response()
                .boxed();
        }
        for guard in self.guards.drain(..) {
            endpoint = NestrsEndpointExt::guard(endpoint, guard)
                .map_to_response()
                .boxed();
        }
        for d in discovery.meta::<HttpInterceptorMeta>() {
            endpoint = NestrsEndpointExt::interceptor(endpoint, d.meta.interceptor())
                .map_to_response()
                .boxed();
        }
        for interceptor in self.interceptors.drain(..) {
            endpoint = NestrsEndpointExt::interceptor(endpoint, interceptor)
                .map_to_response()
                .boxed();
        }

        self.endpoint = Some(endpoint);
        Ok(())
    }

    async fn serve(self: Box<Self>, cancel: CancellationToken) -> Result<()> {
        let endpoint = self
            .endpoint
            .expect("HttpTransport::configure must run before serve");
        let bind = self.bind;
        tracing::info!(addr = %bind, "http transport listening");
        Server::new(TcpListener::bind(&bind))
            .run_with_graceful_shutdown(endpoint, async move { cancel.cancelled().await }, None)
            .await?;
        Ok(())
    }
}
