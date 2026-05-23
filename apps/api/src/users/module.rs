use nestrs_core::module;

use crate::users::controller::UsersController;
use crate::users::job::UserCountReport;
use crate::users::service::UsersService;

// The module wires the service, its HTTP controller, and the `UserCountReport`
// cron job. The resolver self-registers (`#[resolver]`), and its DataLoaders
// self-register too (`#[dataloader]` in service.rs, folded into the container by
// GraphqlModule) — so neither is listed here. The service's lifecycle hooks
// (`#[hooks]`) likewise self-register, via the link-time registry `App` drains at
// boot.
#[module(providers = [UsersService, UsersController, UserCountReport])]
pub struct UsersModule;

#[cfg(test)]
mod tests {
    use super::*;
    use nestrs_core::{Container, Module};
    use std::sync::Arc;

    #[test]
    fn registers_users_service() {
        let container = UsersModule::register(Container::builder()).build();
        let svc: Option<Arc<UsersService>> = container.get();
        assert!(svc.is_some());
    }
}
