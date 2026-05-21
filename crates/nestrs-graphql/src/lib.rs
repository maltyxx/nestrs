//! GraphQL support for nestrs.
//!
//! Two-layer model that mirrors `nestrs-http`:
//!
//! - **Per-resolver** — `#[resolver(kind = Query|Mutation|Subscription)]`
//!   in `nestrs-macros` builds the resolver from the container (with
//!   `#[inject]` fields) and attaches a [`GraphQLResolverMeta`] for
//!   discovery / introspection. Each resolver stays a regular
//!   `#[async_graphql::Object]` impl block.
//! - **App-level composition** — `#[graphql_app(queries=[...],
//!   mutations=[...], subscriptions=[...])]` generates the
//!   `MergedObject` wrappers and a `build(container)` constructor.
//!   Composition is static (async-graphql requires its root types at
//!   compile time), which is why composition lives in a macro at the
//!   app level rather than running off `DiscoveryService` at runtime.

mod resolver;

pub use resolver::{GraphQLResolverMeta, ResolverKind};

pub use async_graphql;
