//! In-process testing harness for nestrs.
//!
//! [`TestApp`] boots an app's real dependency-injection graph — the same
//! four-phase [`AppBuilder`](nestrs_core::AppBuilder) build production uses, with
//! the access-graph contract enforced — and exposes its HTTP surface through
//! `poem`'s `TestClient` without binding a socket. Because GraphQL, OpenAPI and
//! MCP all self-mount as HTTP endpoints, a single client exercises every surface,
//! so wiring that previously only surfaced under `curl` against a running binary
//! is now reachable from `cargo test`.
//!
//! Swap a real provider for a fake with [`TestAppBuilder::override_dyn`] /
//! [`override_value`](TestAppBuilder::override_value) — the NestJS
//! `overrideProvider` analog.
//!
//! ```ignore
//! use nestrs_testing::TestApp;
//!
//! let app = TestApp::for_module::<AppModule>().await?;
//! let resp = app.http().get("/users").send().await;
//! resp.assert_status_is_ok();
//!
//! // With a mock swapped in:
//! let app = TestApp::builder()
//!     .module::<AppModule>()
//!     .override_dyn::<dyn Clock>(std::sync::Arc::new(FrozenClock))
//!     .build()
//!     .await?;
//! ```

use std::any::Any;
use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use nestrs_core::{App, AppBuilder, Container, Module, Transport};
use nestrs_http::HttpTransport;
use poem::endpoint::BoxEndpoint;
use poem::Response;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[cfg(feature = "orm")]
mod database;
#[cfg(feature = "orm")]
pub use database::EphemeralDatabase;

/// `poem`'s test client and its assertion helpers, re-exported so a test crate
/// needs no direct `poem` dependency — mirroring how each surface wraps its
/// backing crate.
pub use poem::test::{TestClient, TestForm, TestJson, TestRequestBuilder, TestResponse};

/// The boxed, fully-assembled HTTP endpoint a [`TestApp`] drives.
type TestEndpoint = BoxEndpoint<'static, Response>;

/// A booted app under test: its assembled [`Container`] plus a [`TestClient`]
/// over the configured HTTP endpoint.
pub struct TestApp {
    app: App,
    client: TestClient<TestEndpoint>,
}

impl TestApp {
    /// Start a [`TestAppBuilder`].
    pub fn builder() -> TestAppBuilder {
        TestAppBuilder::new()
    }

    /// Boot a root module with the default [`HttpTransport`] and no overrides —
    /// the common case.
    pub async fn for_module<M: Module + 'static>() -> Result<TestApp> {
        TestAppBuilder::new().module::<M>().build().await
    }

    /// The `poem` test client over the configured HTTP surface. Fire requests
    /// with `.get(path)`, `.post(path)`, … then `.send().await`.
    pub fn http(&self) -> &TestClient<TestEndpoint> {
        &self.client
    }

    /// The assembled container, to resolve providers and assert their state.
    pub fn container(&self) -> &Container {
        self.app.container()
    }

    /// Run the init lifecycle hooks (`OnModuleInit`, then
    /// `OnApplicationBootstrap`) — the NestJS `app.init()` analog. Deliberately
    /// **not** run by [`build`](TestAppBuilder::build), matching NestJS's
    /// `Test...compile()`, so a test that wants startup side effects opts in.
    pub async fn init(&self) -> Result<()> {
        self.app.init().await
    }
}

/// Builder for a [`TestApp`]: declare the module tree, seed runtime values,
/// override providers with fakes, optionally supply a pre-configured
/// [`HttpTransport`], then [`build`](Self::build).
pub struct TestAppBuilder {
    inner: AppBuilder,
    http: Option<HttpTransport>,
}

impl TestAppBuilder {
    fn new() -> Self {
        Self {
            inner: App::builder(),
            http: None,
        }
    }

    /// Add a root module (delegates to [`AppBuilder::module`]).
    pub fn module<M: Module + 'static>(mut self) -> Self {
        self.inner = self.inner.module::<M>();
        self
    }

    /// Seed a runtime value (delegates to [`AppBuilder::provide`]).
    pub fn provide<T: Any + Send + Sync>(mut self, value: T) -> Self {
        self.inner = self.inner.provide(value);
        self
    }

    /// Seed a shared `Arc<T>` (delegates to [`AppBuilder::provide_arc`]).
    pub fn provide_arc<T: Any + Send + Sync>(mut self, value: Arc<T>) -> Self {
        self.inner = self.inner.provide_arc(value);
        self
    }

    /// Seed a `dyn Trait` binding (delegates to [`AppBuilder::provide_dyn`]).
    pub fn provide_dyn<T: ?Sized + Send + Sync + 'static>(mut self, value: Arc<T>) -> Self {
        self.inner = self.inner.provide_dyn(value);
        self
    }

    /// Register an async factory (delegates to [`AppBuilder::provide_factory`]) —
    /// e.g. a test database pool built before the module tree wires.
    pub fn provide_factory<T, F, Fut>(mut self, factory: F) -> Self
    where
        T: Any + Send + Sync,
        F: FnOnce(Container) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.inner = self.inner.provide_factory(factory);
        self
    }

    /// Replace a concrete provider with a fake (delegates to
    /// [`AppBuilder::override_value`]). Reaches consumers resolved from the final
    /// container; see that method for the eager-build caveat.
    pub fn override_value<T: Any + Send + Sync>(mut self, value: T) -> Self {
        self.inner = self.inner.override_value(value);
        self
    }

    /// Replace a `dyn Trait` binding with a fake (delegates to
    /// [`AppBuilder::override_dyn`]) — the usual way to mock a service injected
    /// behind a trait.
    pub fn override_dyn<T: ?Sized + Send + Sync + 'static>(mut self, value: Arc<T>) -> Self {
        self.inner = self.inner.override_dyn(value);
        self
    }

    /// Use a pre-configured [`HttpTransport`] (global guards / interceptors /
    /// filters) instead of the default, so the test mirrors `main`.
    pub fn http(mut self, transport: HttpTransport) -> Self {
        self.http = Some(transport);
        self
    }

    /// Run the four-phase build (access-graph check included), configure the
    /// HTTP transport in-process against the assembled container, and return the
    /// [`TestApp`]. Propagates a factory error or an access-graph violation.
    pub async fn build(self) -> Result<TestApp> {
        let app = self.inner.build().await?;
        let mut transport = self.http.unwrap_or_default();
        transport.configure(app.container()).await?;
        let endpoint = transport
            .take_endpoint()
            .expect("HttpTransport::configure populates the endpoint");
        Ok(TestApp {
            app,
            client: TestClient::new(endpoint),
        })
    }

    /// Run the four-phase build and return the booted app **without** an HTTP
    /// surface — for an app whose transports are not HTTP (a queue worker, a
    /// scheduler). The DI graph, factory phase and access-graph check run exactly
    /// as in production, so booting alone already exercises an app's wiring. Drive
    /// its transports for a bounded window with
    /// [`HeadlessApp::spawn_transport`].
    pub async fn build_headless(self) -> Result<HeadlessApp> {
        let app = self.inner.build().await?;
        Ok(HeadlessApp { app })
    }
}

#[cfg(feature = "telemetry")]
impl TestAppBuilder {
    /// Satisfy the `TelemetryModule` boot guard for an app that imports it: it
    /// panics at boot unless [`Telemetry::init`](nestrs_telemetry::Telemetry::init)
    /// has run, so this installs console-only test telemetry once (idempotent).
    /// Enable the `telemetry` feature. Without it, such an app's e2e would have to
    /// hand-roll the same one-shot init.
    pub fn with_test_telemetry(self) -> Self {
        nestrs_telemetry::Telemetry::init_for_tests();
        self
    }
}

/// A booted app with no HTTP client — see [`TestAppBuilder::build_headless`]. It
/// exposes the assembled [`Container`] and runs non-HTTP [`Transport`]s on a
/// cancellable background task so a test can enqueue work, observe it, then shut
/// the transport down.
pub struct HeadlessApp {
    app: App,
}

impl HeadlessApp {
    /// The assembled container, to resolve providers (e.g. a queue connection to
    /// enqueue against) and assert their state.
    pub fn container(&self) -> &Container {
        self.app.container()
    }

    /// Run the init lifecycle hooks (`OnModuleInit`, then
    /// `OnApplicationBootstrap`) — the [`TestApp::init`] analog for a headless app.
    pub async fn init(&self) -> Result<()> {
        self.app.init().await
    }

    /// Configure a transport against the container and start serving it on a
    /// background task, returning a [`TransportHandle`] to stop it. A `configure`
    /// failure (the regression an app's wiring most often hits — a missing
    /// discovered dependency, an unresolved connection) propagates here.
    pub async fn spawn_transport<T: Transport>(&self, mut transport: T) -> Result<TransportHandle> {
        transport.configure(self.container()).await?;
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let join = tokio::spawn(async move { Box::new(transport).serve(token).await });
        Ok(TransportHandle { cancel, join })
    }

    /// The booted [`App`], to attach transports and `run()` it directly when a
    /// test wants the full server loop rather than the bounded
    /// [`spawn_transport`](Self::spawn_transport) driver.
    pub fn into_app(self) -> App {
        self.app
    }
}

/// Handle to a transport started by [`HeadlessApp::spawn_transport`]. Dropping it
/// detaches the task; call [`shutdown`](Self::shutdown) to cancel it and await its
/// result.
pub struct TransportHandle {
    cancel: CancellationToken,
    join: JoinHandle<Result<()>>,
}

impl TransportHandle {
    /// Signal the transport to stop (the same `CancellationToken` SIGTERM trips in
    /// production) and await its `serve` future, surfacing any error it returned.
    pub async fn shutdown(self) -> Result<()> {
        self.cancel.cancel();
        self.join.await.map_err(|e| anyhow::anyhow!(e))?
    }
}
