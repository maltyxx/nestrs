use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use nestrs_core::graphql_app;
use poem::{handler, web::Html};

use crate::users::{UsersMutation, UsersQuery};

#[graphql_app(
    queries = [UsersQuery],
    mutations = [UsersMutation],
)]
pub struct AppSchema;

#[handler]
pub async fn playground() -> Html<String> {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}
