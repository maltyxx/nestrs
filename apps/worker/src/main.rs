mod app;
mod audio;

use anyhow::Result;
use nestrs_core::App;
use nestrs_queue::QueueWorker;
use nestrs_schedule::Scheduler;
use nestrs_telemetry::Telemetry;

use crate::app::AppModule;

#[tokio::main]
async fn main() -> Result<()> {
    let _telemetry = Telemetry::init("worker")?;

    App::builder()
        .module::<AppModule>()
        .build()
        .await?
        .transport(Scheduler::new())
        .transport(QueueWorker::new())
        .run()
        .await
}
