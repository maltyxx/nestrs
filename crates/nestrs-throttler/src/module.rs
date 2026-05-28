//! [`ThrottlerModule`] — import it to make the shared [`InMemoryThrottler`]
//! injectable, carrying the default rate limit. The `ThrottlerModule.forRoot`
//! analog.

use nestrs_core::{ContainerBuilder, DynamicModule};

use crate::store::InMemoryThrottler;
use crate::throttle::Throttle;

/// Provides the process-wide [`InMemoryThrottler`] with the given default limit.
/// Import it via [`for_root`](Self::for_root):
///
/// ```ignore
/// #[module(imports = [ThrottlerModule::for_root(Throttle::per_minute(60))])]
/// ```
pub struct ThrottlerModule;

impl ThrottlerModule {
    /// Set the default limit applied to any throttled route that does not override
    /// it with `#[meta(Throttle::...)]`.
    pub fn for_root(default: Throttle) -> ThrottlerSetup {
        ThrottlerSetup { default }
    }
}

/// The configured form of [`ThrottlerModule`], produced by [`ThrottlerModule::for_root`].
pub struct ThrottlerSetup {
    default: Throttle,
}

impl DynamicModule for ThrottlerSetup {
    // Provided in the factory phase so it is global infrastructure: the guard
    // injects it regardless of import order, like the JWT and database resources.
    fn collect(&self, builder: ContainerBuilder) -> ContainerBuilder {
        let default = self.default;
        builder.provide_factory::<InMemoryThrottler, _, _>(move |_| async move {
            Ok(InMemoryThrottler::new(default))
        })
    }
}
