use std::sync::Arc;

use async_graphql::dataloader::DataLoader;
use async_graphql::Result;
use nestrs_graphql::resolver;
use sea_orm::Condition;
use uuid::Uuid;

use crate::errors::gql;
use crate::orgs::entity::Org;
use crate::orgs::service::OrgsServiceById;
use crate::users::entity::{CreateUserInput, User};
use crate::users::service::{UsersService, UsersServiceByName, ORG_ACME};

#[resolver]
pub struct UsersResolver {
    #[inject]
    users: Arc<UsersService>,
}

#[resolver]
impl UsersResolver {
    #[query]
    async fn users(&self) -> Result<Vec<User>> {
        let rows = self.users.list(Condition::all()).await.map_err(gql)?;
        Ok(rows.iter().map(User::from).collect())
    }

    #[query]
    async fn user(&self, id: String) -> Result<Option<User>> {
        let id = Uuid::parse_str(&id).map_err(gql)?;
        Ok(self
            .users
            .find(id)
            .await
            .map_err(gql)?
            .as_ref()
            .map(User::from))
    }

    #[mutation]
    async fn create_user(&self, input: CreateUserInput) -> Result<User> {
        let row = self.users.create(input, ORG_ACME).await.map_err(gql)?;
        Ok(User::from(&row))
    }

    #[field]
    async fn org(&self, parent: &User, by_id: &DataLoader<OrgsServiceById>) -> Result<Option<Org>> {
        let id = Uuid::parse_str(&parent.org_id)?;
        Ok(by_id.load_one(id).await?)
    }

    #[field]
    async fn namesakes(
        &self,
        parent: &User,
        by_name: &DataLoader<UsersServiceByName>,
    ) -> Result<Vec<User>> {
        let same_name = by_name
            .load_one(parent.name.clone())
            .await?
            .unwrap_or_default();
        Ok(same_name
            .into_iter()
            .filter(|u| u.id != parent.id)
            .collect())
    }
}
