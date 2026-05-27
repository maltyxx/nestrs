use poem::http::StatusCode;

pub fn internal(err: impl std::fmt::Display) -> poem::Error {
    poem::Error::from_string(err.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn gql(err: impl std::fmt::Display) -> async_graphql::Error {
    async_graphql::Error::new(err.to_string())
}
