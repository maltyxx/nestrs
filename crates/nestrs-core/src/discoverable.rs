use crate::container::ContainerBuilder;

/// Anything a `#[module]` can pull in via `providers = [...]`.
///
/// The macros that decorate a struct (`#[injectable]`, `#[interceptor]`, and
/// future `#[cron_job]`/`#[event_handler]`/`#[mcp_tool]`/…) — together with
/// the `#[routes]` macro on a controller's `impl` block — emit a single
/// `impl Discoverable for Self` per type. The implementation either:
///
/// - registers the value as a provider (`provide` / `provide_dyn`), or
/// - attaches a piece of discovery metadata to the type
///   ([`ContainerBuilder::attach_meta`]), or both.
///
/// `#[module]` then loops over its `providers = [...]` list and calls
/// `<T as Discoverable>::register(builder)` uniformly — it knows nothing
/// about HTTP, MCP, GraphQL, or any future surface.
pub trait Discoverable {
    fn register(builder: ContainerBuilder) -> ContainerBuilder;
}
