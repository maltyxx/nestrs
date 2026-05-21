use std::sync::Arc;

use async_graphql::{Object, Result};
use nestrs_core::resolver;

use crate::users::dto::{CreateUserInput, UserDto};
use crate::users::service::UsersService;

fn to_gql_error(error: impl std::fmt::Display) -> async_graphql::Error {
    async_graphql::Error::new(error.to_string())
}

#[resolver(kind = Query)]
pub struct UsersQuery {
    #[inject]
    users: Arc<UsersService>,
}

#[Object]
impl UsersQuery {
    async fn users(&self) -> Vec<UserDto> {
        self.users.list().await
    }

    async fn user(&self, id: String) -> Result<Option<UserDto>> {
        self.users.find(&id).await.map_err(to_gql_error)
    }
}

#[resolver(kind = Mutation)]
pub struct UsersMutation {
    #[inject]
    users: Arc<UsersService>,
}

#[Object]
impl UsersMutation {
    async fn create_user(&self, input: CreateUserInput) -> Result<UserDto> {
        self.users.create(input).await.map_err(to_gql_error)
    }
}
