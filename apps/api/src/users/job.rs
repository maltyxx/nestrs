//! A scheduled job: periodically log how many users exist. Demonstrates
//! `#[cron_job]` — discovered like a controller (listed in `UsersModule`), ticked
//! by the `Scheduler` transport, with `UsersService` injected from the container.

use std::sync::Arc;

use anyhow::Result;
use nestrs_schedule::{async_trait, cron_job, Scheduled};

use crate::users::service::UsersService;

#[cron_job(every = "2s")]
pub struct UserCountReport {
    #[inject]
    users: Arc<UsersService>,
}

#[async_trait]
impl Scheduled for UserCountReport {
    async fn run(&self) -> Result<()> {
        let count = self.users.list().await.len();
        tracing::info!(target: "nestrs::schedule", count, "user count report");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use nestrs_core::{Container, DiscoveryService, Module};
    use nestrs_schedule::CronJobMeta;

    use crate::users::UsersModule;

    #[test]
    fn cron_job_is_discovered_with_its_period() {
        let container = UsersModule::register(Container::builder()).build();
        let jobs = DiscoveryService::new(&container).meta::<CronJobMeta>();
        let report = jobs
            .iter()
            .find(|d| d.meta.name == "UserCountReport")
            .expect("UserCountReport is discovered via #[cron_job]");
        assert_eq!(report.meta.period.as_secs(), 2);
    }
}
