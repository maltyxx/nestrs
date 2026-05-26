//! `QueueModule` — owns the shared Redis [`QueueConnection`](crate::QueueConnection).
//!
//! The connection is async, which a synchronous [`Module`](nestrs_core::Module)
//! cannot build, so this is a [`DynamicModule`](nestrs_core::DynamicModule) that
//! owns its connection in the **collect phase**: declared in
//! `#[module(imports = [...])]`, it queues a factory that
//! [`AppBuilder::build`](nestrs_core::AppBuilder::build) `await`s before the
//! module tree is wired, so the `QueueWorker` transport and every producer
//! inject it — import order irrelevant:
//!
//! ```ignore
//! #[module(imports = [
//!     QueueModule::for_root(QueueOptions {
//!         url: std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".into()),
//!     }),
//!     AudioModule,
//! ])]
//! pub struct AppModule;
//! ```

use nestrs_core::{ContainerBuilder, DynamicModule};

use crate::QueueConnection;

const DEFAULT_URL: &str = "redis://127.0.0.1/";

/// Connection settings for [`QueueModule`], written as a struct literal at the
/// import site (like `DatabaseOptions`). Set `url` — usually from the
/// environment; [`Default`] uses a local Redis (`redis://127.0.0.1/`).
#[derive(Clone, Debug)]
pub struct QueueOptions {
    /// The Redis URL backing the queues.
    pub url: String,
}

impl Default for QueueOptions {
    fn default() -> Self {
        Self {
            url: DEFAULT_URL.to_string(),
        }
    }
}

/// The queue module. List it in `#[module(imports = [...])]` via
/// [`for_root`](Self::for_root) — see the [module docs](self). It registers a
/// [`QueueConnection`].
pub struct QueueModule;

impl QueueModule {
    /// Configure the Redis connection at its import site. Returns a
    /// [`DynamicModule`] to list in `#[module(imports = [...])]`.
    pub fn for_root(options: QueueOptions) -> QueueSetup {
        QueueSetup { options }
    }
}

/// The configured form of [`QueueModule`], produced by
/// [`QueueModule::for_root`].
pub struct QueueSetup {
    options: QueueOptions,
}

impl DynamicModule for QueueSetup {
    // The connection is async, so it is queued in the collect phase and awaited
    // before providers are built — never in the synchronous `register`.
    fn collect(&self, builder: ContainerBuilder) -> ContainerBuilder {
        let url = self.options.url.clone();
        builder.provide_factory::<QueueConnection, _, _>(move |_| async move {
            QueueConnection::connect(&url).await
        })
    }
}
