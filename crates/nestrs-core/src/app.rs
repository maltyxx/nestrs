use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::container::{Container, ContainerBuilder};
use crate::lifecycle::{run_phase, run_phase_lenient, LifecyclePhase};
use crate::module::Module;
use crate::transport::Transport;

/// A registration to apply once a factory has produced its value: it `provide`s
/// the awaited result, so a factory output flows through the same path — and the
/// same duplicate-detection — as any other provider.
type Registrar = Box<dyn FnOnce(ContainerBuilder) -> ContainerBuilder + Send>;
type FactoryFuture = Pin<Box<dyn Future<Output = Result<Registrar>> + Send>>;
type BoxedFactory = Box<dyn FnOnce(Container) -> FactoryFuture + Send>;

/// Entry point for a nestrs application. Builds the container from a root
/// [`Module`], attaches zero or more [`Transport`]s, and runs them
/// concurrently until shutdown.
///
/// For ops scripts (migrations, seeders) that need the container but no
/// transport, use [`App::context`] instead — it builds the container and
/// hands it back without starting anything.
pub struct App {
    container: Container,
    transports: Vec<Box<dyn Transport>>,
}

impl App {
    /// Build the container from the root module and return an empty app.
    pub fn new<M: Module>() -> Self {
        let container = M::register(Container::builder()).build();
        Self {
            container,
            transports: Vec::new(),
        }
    }

    /// Build only the container, with no transports attached. Use this from
    /// `bin/migrate.rs`-style tools that need the DI graph without a server.
    /// Equivalent to NestJS's `NestFactory.createApplicationContext`.
    pub fn context<M: Module>() -> Container {
        M::register(Container::builder()).build()
    }

    /// Start an [`AppBuilder`] for apps that must seed runtime values (a loaded
    /// config, parsed CLI args) or build providers asynchronously (a DB pool, a
    /// cache client) before the module tree is wired. Apps that need none of
    /// that use [`App::new`].
    pub fn builder() -> AppBuilder {
        AppBuilder::new()
    }

    /// Container reference, in case the caller needs to resolve services
    /// before attaching transports (e.g. to build a GraphQL schema from a
    /// resolver that lives in the container).
    pub fn container(&self) -> &Container {
        &self.container
    }

    pub fn transport<T: Transport>(mut self, transport: T) -> Self {
        self.transports.push(Box::new(transport));
        self
    }

    /// Configure each transport against the container, run the init lifecycle
    /// hooks, then run all transports concurrently. SIGINT / SIGTERM cancels the
    /// shared token; the first transport that errors also cancels the others.
    /// Once the transports have stopped, the shutdown lifecycle hooks run.
    pub async fn run(self) -> Result<()> {
        let App {
            container,
            mut transports,
        } = self;

        for t in transports.iter_mut() {
            t.configure(&container).await?;
        }

        // Init phases run after wiring, before serving. A failure here aborts
        // the boot — nothing is listening yet, so there is nothing to tear down.
        run_phase(&container, LifecyclePhase::OnModuleInit).await?;
        run_phase(&container, LifecyclePhase::OnApplicationBootstrap).await?;

        let cancel = CancellationToken::new();
        spawn_shutdown_signal(cancel.clone());

        let mut join = JoinSet::new();
        for transport in transports {
            let token = cancel.clone();
            join.spawn(async move { transport.serve(token).await });
        }

        let mut first_err: Option<anyhow::Error> = None;
        while let Some(res) = join.join_next().await {
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                        cancel.cancel();
                    }
                }
                Err(join_err) => {
                    if first_err.is_none() {
                        first_err = Some(anyhow!(join_err));
                        cancel.cancel();
                    }
                }
            }
        }

        // Shutdown phases run after the transports have stopped, best-effort, so
        // every provider's cleanup runs even if one fails or a transport errored.
        run_phase_lenient(&container, LifecyclePhase::OnModuleDestroy).await;
        run_phase_lenient(&container, LifecyclePhase::BeforeApplicationShutdown).await;
        run_phase_lenient(&container, LifecyclePhase::OnApplicationShutdown).await;

        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// Builder for an [`App`] that needs runtime values or asynchronously-built
/// providers in the container before the module tree is wired.
///
/// Three phases run at [`build`](AppBuilder::build), independent of call order:
///
/// 1. **Seeds** — values registered with [`provide`](AppBuilder::provide) /
///    [`provide_arc`](AppBuilder::provide_arc) /
///    [`provide_dyn`](AppBuilder::provide_dyn). These are the runtime values a
///    `main` computes — a loaded config, parsed CLI args — that the DI graph
///    then injects.
/// 2. **Factories** — async closures registered with
///    [`provide_factory`](AppBuilder::provide_factory), run in registration
///    order. Each sees the container assembled so far, so a factory may depend
///    on a seed or an earlier factory's output. This is where a DB pool or a
///    cache client is `await`ed at boot; a returned `Err` aborts the build.
/// 3. **Modules** — every [`module`](AppBuilder::module) registers its
///    providers last, so they can inject the seeds and factory outputs above.
///
/// Factories are the composition root's job: a `#[module]`'s `register` is
/// synchronous, so it *consumes* an async-built provider but does not declare
/// one. Apps that need no runtime values skip the builder entirely and use
/// [`App::new`].
pub struct AppBuilder {
    builder: ContainerBuilder,
    factories: Vec<BoxedFactory>,
    modules: Vec<fn(ContainerBuilder) -> ContainerBuilder>,
}

impl AppBuilder {
    fn new() -> Self {
        Self {
            builder: Container::builder(),
            factories: Vec::new(),
            modules: Vec::new(),
        }
    }

    /// Seed a runtime value, wrapped in `Arc` internally. Injectable as
    /// `Arc<T>` by any provider in the module tree.
    pub fn provide<T: Any + Send + Sync>(mut self, value: T) -> Self {
        self.builder = self.builder.provide(value);
        self
    }

    /// Seed an already-shared `Arc<T>`.
    pub fn provide_arc<T: Any + Send + Sync>(mut self, value: Arc<T>) -> Self {
        self.builder = self.builder.provide_arc(value);
        self
    }

    /// Seed a trait-object binding, injectable elsewhere as `Arc<dyn Trait>`.
    pub fn provide_dyn<T: ?Sized + Send + Sync + 'static>(mut self, value: Arc<T>) -> Self {
        self.builder = self.builder.provide_dyn(value);
        self
    }

    /// Register an async factory that builds a provider of type `T` from the
    /// container assembled so far (phase 2 above). Its output is stored as a
    /// provider, so the module tree can inject `Arc<T>`. The canonical use is a
    /// resource that must be `await`ed once at boot — a database pool built from
    /// a seeded config:
    ///
    /// ```ignore
    /// App::builder()
    ///     .provide(DbConfig::from_env())
    ///     .provide_factory(|c| async move {
    ///         let cfg = c.get::<DbConfig>().expect("DbConfig seeded");
    ///         Ok(DbPool::connect(&cfg.url).await?)
    ///     })
    ///     .module::<AppModule>()
    ///     .build()
    ///     .await?
    /// ```
    pub fn provide_factory<T, F, Fut>(mut self, factory: F) -> Self
    where
        T: Any + Send + Sync,
        F: FnOnce(Container) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.factories.push(Box::new(move |container| {
            Box::pin(async move {
                let value = factory(container).await?;
                let registrar: Registrar = Box::new(move |builder| builder.provide(value));
                Ok(registrar)
            })
        }));
        self
    }

    /// Register a root module. May be called more than once; all modules
    /// register after every seed and factory.
    pub fn module<M: Module>(mut self) -> Self {
        self.modules.push(M::register);
        self
    }

    /// Run the three phases and return the assembled [`App`], ready for
    /// [`transport`](App::transport) and [`run`](App::run). Propagates the first
    /// factory error.
    pub async fn build(self) -> Result<App> {
        let AppBuilder {
            mut builder,
            factories,
            modules,
        } = self;

        for factory in factories {
            let register = factory(builder.snapshot()).await?;
            builder = register(builder);
        }
        for register in modules {
            builder = register(builder);
        }

        Ok(App {
            container: builder.build(),
            transports: Vec::new(),
        })
    }
}

fn spawn_shutdown_signal(cancel: CancellationToken) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to install SIGTERM handler");
                    return;
                }
            };
            tokio::select! {
                _ = tokio::signal::ctrl_c() => tracing::info!("SIGINT received, shutting down"),
                _ = sigterm.recv()          => tracing::info!("SIGTERM received, shutting down"),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("ctrl-c received, shutting down");
        }
        cancel.cancel();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Config(u32);
    struct Doubled(u32);

    // Hand-written module (the `#[module]` macro lives in `nestrs-macros`, which
    // this crate cannot use on its own types). It builds `Doubled` from a seeded
    // `Config`, proving seeds are visible by the time modules register.
    struct DoublerModule;
    impl Module for DoublerModule {
        fn register(builder: ContainerBuilder) -> ContainerBuilder {
            let cfg = builder
                .snapshot()
                .get::<Config>()
                .expect("Config is seeded before modules register");
            builder.provide(Doubled(cfg.0 * 2))
        }
    }

    #[tokio::test]
    async fn seeds_are_visible_to_modules() {
        let app = App::builder()
            .provide(Config(21))
            .module::<DoublerModule>()
            .build()
            .await
            .expect("build succeeds");
        assert_eq!(app.container().get::<Doubled>().unwrap().0, 42);
    }

    #[tokio::test]
    async fn factory_runs_async_and_reads_a_seed() {
        let app = App::builder()
            .provide(Config(10))
            .provide_factory(|c| async move {
                let cfg = c.get::<Config>().expect("seed visible to factory");
                tokio::task::yield_now().await;
                Ok(Doubled(cfg.0 + 5))
            })
            .build()
            .await
            .expect("build succeeds");
        assert_eq!(app.container().get::<Doubled>().unwrap().0, 15);
    }

    struct First(u32);
    struct Second(u32);

    #[tokio::test]
    async fn later_factory_sees_earlier_factory_output() {
        let app = App::builder()
            .provide_factory(|_| async { Ok(First(1)) })
            .provide_factory(|c| async move {
                let first = c.get::<First>().expect("earlier factory output visible");
                Ok(Second(first.0 + 1))
            })
            .build()
            .await
            .expect("build succeeds");
        assert_eq!(app.container().get::<Second>().unwrap().0, 2);
    }

    #[tokio::test]
    async fn factory_error_aborts_build() {
        // `App` is not `Debug`, so match rather than `expect_err`.
        let err = match App::builder()
            .provide_factory::<Config, _, _>(|_| async { Err(anyhow!("connection refused")) })
            .build()
            .await
        {
            Ok(_) => panic!("a failing factory must abort the build"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("connection refused"));
    }

    #[tokio::test]
    async fn modules_inject_factory_output() {
        // The factory builds `Config`; the module then reads it — the cross-phase
        // contract a DB pool (factory) + repositories (module) will rely on.
        let app = App::builder()
            .provide_factory(|_| async { Ok(Config(7)) })
            .module::<DoublerModule>()
            .build()
            .await
            .expect("build succeeds");
        assert_eq!(app.container().get::<Doubled>().unwrap().0, 14);
    }
}
