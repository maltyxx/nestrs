//! [`AuthModule`] — import it to make a configured [`JwtService`] injectable
//! everywhere. The analog of NestJS's `JwtModule.register({ secret, ... })`.

use nestrs_core::{ContainerBuilder, DynamicModule};

use crate::jwt::{JwtOptions, JwtService};
use crate::oauth::{OAuth2Client, OAuth2Config};

/// Provides the app's [`JwtService`]. It carries a secret, so there is no
/// zero-config bare-type form — always import it via [`for_root`](Self::for_root):
///
/// ```ignore
/// #[module(imports = [
///     AuthModule::for_root(JwtOptions::new(std::env::var("JWT_SECRET").unwrap())),
/// ])]
/// ```
pub struct AuthModule;

impl AuthModule {
    /// Configure JWT signing at the import site. Returns a [`DynamicModule`] to
    /// list in `#[module(imports = [...])]`.
    pub fn for_root(options: JwtOptions) -> AuthSetup {
        AuthSetup { options }
    }
}

/// The configured form of [`AuthModule`], produced by [`AuthModule::for_root`].
pub struct AuthSetup {
    options: JwtOptions,
}

impl DynamicModule for AuthSetup {
    // The `JwtService` is provided through the factory phase so it lands as
    // global infrastructure: injectable by any strategy, guard, or login handler
    // regardless of module import order — the same contract the database and
    // queue connections rely on. (Construction is synchronous; the async factory
    // is just the channel that makes it global.)
    fn collect(&self, builder: ContainerBuilder) -> ContainerBuilder {
        let options = self.options.clone();
        builder.provide_factory::<JwtService, _, _>(move |_| {
            let options = options.clone();
            async move { Ok(JwtService::new(options)) }
        })
    }
}

/// Provides a configured [`OAuth2Client`] for a single provider, injectable as
/// `Arc<OAuth2Client>` by an OAuth [`Strategy`](crate::Strategy). Import it via
/// [`for_root`](Self::for_root); the per-provider strategy (userinfo → principal)
/// stays in the app.
///
/// The flat container keys by type, so one app currently wires one
/// [`OAuth2Client`]; multiple providers would need per-provider newtypes (a
/// future addition).
pub struct OAuth2Module;

impl OAuth2Module {
    /// Configure the OAuth2 provider at the import site.
    pub fn for_root(config: OAuth2Config) -> OAuth2Setup {
        OAuth2Setup { config }
    }
}

/// The configured form of [`OAuth2Module`], produced by [`OAuth2Module::for_root`].
pub struct OAuth2Setup {
    config: OAuth2Config,
}

impl DynamicModule for OAuth2Setup {
    // Provided in the factory phase so it lands as global infrastructure, like the
    // `JwtService` above.
    fn collect(&self, builder: ContainerBuilder) -> ContainerBuilder {
        let config = self.config.clone();
        builder.provide_factory::<OAuth2Client, _, _>(move |_| {
            let config = config.clone();
            async move { OAuth2Client::new(config).map_err(anyhow::Error::new) }
        })
    }
}
