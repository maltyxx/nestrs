mod app;
mod weather;

use std::time::Duration;

use anyhow::Result;
use nestrs_core::App;
use nestrs_http::HttpTransport;
use nestrs_telemetry::Telemetry;

use crate::weather::WeatherConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let _telemetry = Telemetry::init("mcp")?;

    // Seed the config, then build the upstream HTTP client from it once at boot
    // (the timeout is configurable). `OpenMeteoClient` injects both. Modules
    // register last, so they see these roots.
    App::builder()
        .provide(WeatherConfig::from_env())
        .provide_factory::<reqwest::Client, _, _>(|c| async move {
            let cfg = c
                .get::<WeatherConfig>()
                .expect("WeatherConfig is seeded before factories run");
            let client = reqwest::Client::builder()
                .timeout(Duration::from_millis(cfg.request_timeout_ms))
                .user_agent(concat!("nestrs-mcp/", env!("CARGO_PKG_VERSION")))
                .build()?;
            Ok(client)
        })
        .module::<app::AppModule>()
        .build()
        .await?
        .transport(HttpTransport::new().bind("0.0.0.0:3003"))
        .run()
        .await
}
