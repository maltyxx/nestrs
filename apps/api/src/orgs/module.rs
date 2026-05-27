use nestrs_core::module;

use crate::orgs::controller::OrgsController;
use crate::orgs::service::OrgsService;

#[module(providers = [OrgsService, OrgsController])]
pub struct OrgsModule;

#[cfg(test)]
mod tests {
    use super::*;
    use nestrs_core::{Container, Module};
    use std::sync::Arc;

    #[test]
    fn registers_orgs_service() {
        let container = OrgsModule::register(
            Container::builder().provide(sea_orm::DatabaseConnection::default()),
        )
        .build();
        let svc: Option<Arc<OrgsService>> = container.get();
        assert!(svc.is_some());
    }
}
