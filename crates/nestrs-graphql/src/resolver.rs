use std::any::TypeId;

/// Which root each resolver contributes to. The `#[resolver(kind = ...)]`
/// macro stamps this on a [`GraphQLResolverMeta`] so introspection tools
/// (or a `/_resolvers` debug endpoint) can list resolvers by kind without
/// reparsing the GraphQL schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolverKind {
    Query,
    Mutation,
    Subscription,
}

/// Discovery metadata attached by the `#[resolver]` macro to every
/// resolver struct. The schema composition itself happens statically in
/// the `#[graphql_app]` macro — this meta is informational, not
/// load-bearing. Read it via
/// [`nestrs_core::DiscoveryService::meta`].
pub struct GraphQLResolverMeta {
    pub kind: ResolverKind,
    pub type_id: TypeId,
}

impl GraphQLResolverMeta {
    pub fn new(kind: ResolverKind, type_id: TypeId) -> Self {
        Self { kind, type_id }
    }
}
