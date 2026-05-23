use std::sync::Arc;

use nestrs_http::{controller, routes, Piped, Valid};
use nestrs_pipes::ParseUuidV7;
use poem::http::StatusCode;
use poem::web::{Json, Path};
use poem::{Error, Result};

use crate::users::dto::{CreateUserInput, UserDto};
use crate::users::service::UsersService;

#[controller(path = "/users")]
pub struct UsersController {
    #[inject]
    svc: Arc<UsersService>,
}

#[routes]
impl UsersController {
    #[get("/")]
    async fn list(&self) -> Json<Vec<UserDto>> {
        Json(self.svc.list().await)
    }

    #[get("/:id")]
    async fn get(&self, id: Piped<ParseUuidV7, Path<String>>) -> Result<Json<UserDto>> {
        let id = id.into_inner();
        match self.svc.find(&id.to_string()).await {
            Ok(Some(user)) => Ok(Json(user)),
            Ok(None) => Err(Error::from_status(StatusCode::NOT_FOUND)),
            Err(err) => Err(Error::from_string(
                err.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            )),
        }
    }

    #[post("/")]
    async fn create(&self, body: Valid<Json<CreateUserInput>>) -> Result<Json<UserDto>> {
        self.svc
            .create(body.into_inner())
            .await
            .map(Json)
            .map_err(|err| Error::from_string(err.to_string(), StatusCode::INTERNAL_SERVER_ERROR))
    }
}
