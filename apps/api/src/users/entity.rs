//! The `users` table as a SeaORM entity.
//!
//! `#[expose]` exposes it to GraphQL + OpenAPI from one declaration — it
//! generates `User` (the GraphQL object + JSON schema) and `CreateUserInput`
//! from the fields, then leaves the entity untouched so `#[sea_orm::model]`
//! keeps the ORM's full power. `org_id` is the multi-tenant scope: `skip` keeps
//! it out of the API surface (the service sets it from the authenticated
//! caller). Routes and guards live on the controller/resolver, never here.

use nestrs_resource::expose;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[expose(name = "User", complex)]
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[expose(skip)]
    pub org_id: Uuid,
    #[expose(input(create), validate(length(min = 1)))]
    pub name: String,
    #[sea_orm(unique)]
    #[expose(input(create), validate(email))]
    pub email: String,
}

impl ActiveModelBehavior for ActiveModel {}
