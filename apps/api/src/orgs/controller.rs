use std::sync::Arc;

use nestrs_http::{controller, routes, Piped, Valid};
use nestrs_pipes::ParseUuid;
use poem::http::StatusCode;
use poem::web::{Json, Path};
use poem::{Error, Result};

use crate::authn::AuthGuard;
use crate::errors::internal;
use crate::orgs::entity::{CreateOrgInput, Org};
use crate::orgs::service::OrgsService;

#[controller(path = "/orgs")]
pub struct OrgsController {
    #[inject]
    svc: Arc<OrgsService>,
}

#[routes]
impl OrgsController {
    #[get("/")]
    #[use_guards(AuthGuard)]
    #[api(summary = "List organizations", tags("Orgs"))]
    async fn list(&self) -> Result<Json<Vec<Org>>> {
        let rows = self.svc.list().await.map_err(internal)?;
        Ok(Json(rows.iter().map(Org::from).collect()))
    }

    #[get("/:id")]
    #[use_guards(AuthGuard)]
    #[api(summary = "Fetch an organization by id", tags("Orgs"))]
    async fn get(&self, id: Piped<ParseUuid, Path<String>>) -> Result<Json<Org>> {
        self.svc
            .find(id.into_inner())
            .await
            .map_err(internal)?
            .as_ref()
            .map(|row| Json(Org::from(row)))
            .ok_or_else(|| Error::from_status(StatusCode::NOT_FOUND))
    }

    #[post("/")]
    #[use_guards(AuthGuard)]
    #[api(summary = "Create an organization", tags("Orgs"))]
    async fn create(&self, body: Valid<Json<CreateOrgInput>>) -> Result<Json<Org>> {
        let row = self.svc.create(body.into_inner()).await.map_err(internal)?;
        Ok(Json(Org::from(&row)))
    }
}
