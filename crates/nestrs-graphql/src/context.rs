//! Per-request context bridge: forward selected values from the *poem* request
//! into the *async-graphql* context, so a resolver reads per-request state an
//! HTTP guard attached. This is the seam GraphQL authorization needs — the
//! actor's `Ability`, built by an HTTP guard and stored on the request, must
//! reach the resolvers — and it serves any request-scoped value.
//!
//! It is needed because async-graphql-poem does not forward poem request
//! extensions into the graphql context, and an async-graphql `Extension`
//! (`prepare_request`) never sees the poem request. So the bridge lives at the
//! poem endpoint: [`ContextEndpoint`] folds every link-time-registered
//! [`ContextSeed`] over the parsed request before executing it.
//!
//! A resolver reads what a seed attached with a `ctx: &async_graphql::Context`
//! parameter (which `#[query]` / `#[mutation]` forward natively) and
//! `ctx.data_opt::<T>()` — no `#[resolver]` macro support is required.

use async_graphql::{BatchRequest, Executor, Request as GqlRequest};
use async_graphql_poem::{GraphQLBatchRequest, GraphQLBatchResponse};
use nestrs_core::Container;
use poem::{Endpoint, FromRequest, IntoResponse, Request, Response, Result};

/// One per-request forwarder, submitted via `inventory`. `seed` reads from the
/// poem request (and the container, for anything it must resolve) and attaches
/// values to the graphql request with [`Request::data`](GqlRequest::data),
/// returning the augmented request. `pub` so a downstream crate
/// (`nestrs-authz-graphql`) can submit one.
///
/// ```ignore
/// nestrs_graphql::inventory::submit! {
///     nestrs_graphql::ContextSeed {
///         seed: |req, _container, gql| match req.extensions().get::<Arc<Ability>>() {
///             Some(ability) => gql.data(ability.clone()),
///             None => gql,
///         },
///     }
/// }
/// ```
pub struct ContextSeed {
    pub seed: fn(&Request, &Container, GqlRequest) -> GqlRequest,
}

inventory::collect!(ContextSeed);

/// The `/graphql` endpoint [`GraphqlModule`](crate::GraphqlModule) mounts. It
/// mirrors `async_graphql_poem::GraphQL`'s GET / POST / batch handling but folds
/// every [`ContextSeed`] over the request first, so per-request context reaches
/// resolvers. The upstream endpoint's experimental `accept: multipart/mixed`
/// incremental-delivery path (`@defer` / `@stream`) is not reproduced; ordinary
/// queries, mutations and batches behave identically.
pub(crate) struct ContextEndpoint<E> {
    executor: E,
    container: Container,
}

impl<E> ContextEndpoint<E> {
    pub(crate) fn new(executor: E, container: Container) -> Self {
        Self {
            executor,
            container,
        }
    }

    fn seed(&self, req: &Request, gql: GqlRequest) -> GqlRequest {
        inventory::iter::<ContextSeed>().fold(gql, |gql, reg| (reg.seed)(req, &self.container, gql))
    }
}

impl<E: Executor> Endpoint for ContextEndpoint<E> {
    type Output = Response;

    async fn call(&self, req: Request) -> Result<Response> {
        let (req, mut body) = req.split();
        let batch = GraphQLBatchRequest::from_request(&req, &mut body).await?.0;
        let batch = match batch {
            BatchRequest::Single(r) => BatchRequest::Single(self.seed(&req, r)),
            BatchRequest::Batch(rs) => {
                BatchRequest::Batch(rs.into_iter().map(|r| self.seed(&req, r)).collect())
            }
        };
        Ok(GraphQLBatchResponse(self.executor.execute_batch(batch).await).into_response())
    }
}
