//! Expose a SeaORM entity to GraphQL **and** OpenAPI from one declaration.
//!
//! Put [`macro@expose`] on a SeaORM entity to generate its GraphQL output
//! object (`SimpleObject` + `JsonSchema`) and `Create/Update` input types, while
//! leaving the entity itself untouched — so it keeps the ORM's full power
//! (`#[sea_orm::model]` or `DeriveEntityModel`). Routes and guards stay where
//! they belong: on hand-written controllers (`#[controller]`/`#[routes]`) and
//! resolvers (`#[resolver]`), which consume the generated types.

pub use nestrs_resource_macros::expose;
