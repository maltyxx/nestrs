use nestrs_core::module;

use crate::users::resolver::{UsersMutation, UsersQuery};
use crate::users::service::UsersService;

#[module(providers = [UsersService, UsersQuery, UsersMutation])]
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
