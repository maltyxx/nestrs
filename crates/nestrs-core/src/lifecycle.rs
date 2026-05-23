//! Application lifecycle hooks, mirroring NestJS's lifecycle events.
//!
//! A provider opts in by tagging methods on an impl block with `#[hooks]`
//! (`#[on_module_init]`, `#[on_application_shutdown]`, …). The macro resolves
//! the provider from the container at call time — the *same* instance request
//! handlers see — and submits each hook to the link-time [`inventory`] registry
//! below. [`crate::App::run`] drains the registry at the right moments.
//!
//! Why a registry rather than discovery metadata: a lifecycle hook decorates a
//! provider that *already* carries its single `impl Discoverable` (emitted by
//! `#[injectable]`), and a type can only have one. Submitting to `inventory`
//! sidesteps that — the same trick GraphQL resolver composition uses — so a
//! provider stays a plain `#[injectable]` and gains hooks without a second
//! `Discoverable` or a central registration list.
//!
//! Ordering within a phase is by `(provider, method)` name, since the registry's
//! link-time iteration order is not stable across builds. Cross-provider init
//! *dependencies* (open the pool before warming the cache) are not expressed
//! here — a hook that needs another service should inject it and rely on that
//! service being constructed; the hook only runs side effects.

use std::future::Future;
use std::pin::Pin;

use crate::container::Container;

/// The point in the application lifecycle at which a hook runs. Init phases run
/// after the container is built and transports configured, before serving;
/// shutdown phases run after the transports stop. Names mirror NestJS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecyclePhase {
    OnModuleInit,
    OnApplicationBootstrap,
    OnModuleDestroy,
    BeforeApplicationShutdown,
    OnApplicationShutdown,
}

/// Future a hook returns. Borrows the container for the duration of the call.
type HookFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

/// One lifecycle hook, submitted to the link-time registry by `#[hooks]`. The
/// `run` thunk resolves the provider from the container and invokes the hook
/// method; it is a no-op if the provider was never registered in any module.
pub struct LifecycleHook {
    pub phase: LifecyclePhase,
    pub provider: &'static str,
    pub method: &'static str,
    pub run: for<'a> fn(&'a Container) -> HookFuture<'a>,
}

inventory::collect!(LifecycleHook);

/// Hooks registered for `phase`, sorted by `(provider, method)` so boot is
/// deterministic regardless of link order.
fn hooks_for(phase: LifecyclePhase) -> Vec<&'static LifecycleHook> {
    let mut hooks: Vec<&'static LifecycleHook> = inventory::iter::<LifecycleHook>()
        .filter(|hook| hook.phase == phase)
        .collect();
    hooks.sort_by_key(|hook| (hook.provider, hook.method));
    hooks
}

/// Run every hook for `phase` sequentially, aborting on the first error.
/// Used for the init phases: a failed hook means the app must not start.
pub(crate) async fn run_phase(container: &Container, phase: LifecyclePhase) -> anyhow::Result<()> {
    for hook in hooks_for(phase) {
        tracing::debug!(
            target: "nestrs::lifecycle",
            ?phase,
            provider = hook.provider,
            method = hook.method,
            "running lifecycle hook",
        );
        (hook.run)(container).await.map_err(|err| {
            err.context(format!(
                "lifecycle hook {}::{} ({phase:?}) failed",
                hook.provider, hook.method
            ))
        })?;
    }
    Ok(())
}

/// Run every hook for `phase` best-effort, logging failures and continuing.
/// Used for the shutdown phases: one provider's cleanup error must not skip
/// another's.
pub(crate) async fn run_phase_lenient(container: &Container, phase: LifecyclePhase) {
    for hook in hooks_for(phase) {
        if let Err(err) = (hook.run)(container).await {
            tracing::error!(
                target: "nestrs::lifecycle",
                ?phase,
                provider = hook.provider,
                method = hook.method,
                error = %err,
                "lifecycle hook failed",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Probe {
        hits: AtomicUsize,
    }

    impl Probe {
        async fn touch(&self) -> anyhow::Result<()> {
            self.hits.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    // Hand-written thunk + submission: the runtime is exercised here without the
    // `#[hooks]` macro, which lives in `nestrs-macros` and cannot target this
    // crate's own types.
    fn run_touch(container: &Container) -> HookFuture<'_> {
        Box::pin(async move {
            match container.get::<Probe>() {
                Some(probe) => probe.touch().await,
                None => Ok(()),
            }
        })
    }

    inventory::submit! {
        LifecycleHook {
            phase: LifecyclePhase::OnModuleInit,
            provider: "Probe",
            method: "touch",
            run: run_touch,
        }
    }

    #[tokio::test]
    async fn runs_registered_init_hook_against_the_container_instance() {
        let container = Container::builder()
            .provide(Probe {
                hits: AtomicUsize::new(0),
            })
            .build();
        run_phase(&container, LifecyclePhase::OnModuleInit)
            .await
            .unwrap();
        assert_eq!(
            container
                .get::<Probe>()
                .unwrap()
                .hits
                .load(Ordering::SeqCst),
            1
        );
    }

    #[tokio::test]
    async fn phase_with_no_hooks_is_a_noop() {
        let container = Container::builder().build();
        run_phase(&container, LifecyclePhase::OnApplicationShutdown)
            .await
            .unwrap();
    }
}
