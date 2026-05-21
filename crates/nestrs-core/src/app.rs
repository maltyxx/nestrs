use anyhow::{anyhow, Result};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::container::Container;
use crate::module::Module;
use crate::transport::Transport;

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

    /// Configure each transport against the container, then run all of them
    /// concurrently. SIGINT / SIGTERM signals cancel the shared token; the
    /// first transport that errors also cancels the others.
    pub async fn run(self) -> Result<()> {
        let App {
            container,
            mut transports,
        } = self;

        for t in transports.iter_mut() {
            t.configure(&container).await?;
        }

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

        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
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
