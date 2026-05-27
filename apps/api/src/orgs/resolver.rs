use std::sync::Arc;

use async_graphql::dataloader::DataLoader;
use async_graphql::Result;
use nestrs_graphql::resolver;
use uuid::Uuid;

use crate::errors::gql;
use crate::orgs::entity::{CreateOrgInput, Org};
use crate::orgs::service::OrgsService;
use crate::users::entity::User;
use crate::users::service::UsersServiceByOrg;

#[resolver]
pub struct OrgsResolver {
    #[inject]
    orgs: Arc<OrgsService>,
}

#[resolver]
impl OrgsResolver {
    #[query]
    async fn orgs(&self) -> Result<Vec<Org>> {
        let rows = self.orgs.list().await.map_err(gql)?;
        Ok(rows.iter().map(Org::from).collect())
    }

    #[query]
    async fn org(&self, id: String) -> Result<Option<Org>> {
        let id = Uuid::parse_str(&id).map_err(gql)?;
        Ok(self
            .orgs
            .find(id)
            .await
            .map_err(gql)?
            .as_ref()
            .map(Org::from))
    }

    #[mutation]
    async fn create_org(&self, input: CreateOrgInput) -> Result<Org> {
        let row = self.orgs.create(input).await.map_err(gql)?;
        Ok(Org::from(&row))
    }

    #[field]
    async fn users(
        &self,
        parent: &Org,
        by_org: &DataLoader<UsersServiceByOrg>,
    ) -> Result<Vec<User>> {
        let id = Uuid::parse_str(&parent.id).map_err(gql)?;
        Ok(by_org.load_one(id).await?.unwrap_or_default())
    }
}
