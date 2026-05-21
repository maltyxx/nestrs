mod app;
mod weather;

use anyhow::Result;
use nestrs_core::App;
use nestrs_http::HttpTransport;
use nestrs_telemetry::Telemetry;

use crate::weather::WeatherController;

#[tokio::main]
async fn main() -> Result<()> {
    let _telemetry = Telemetry::init("mcp")?;

    tracing::info!("mcp listening on http://0.0.0.0:3003");
    tracing::info!("  MCP streamable HTTP: http://0.0.0.0:3003/mcp");
    tracing::info!("  Liveness probe:      http://0.0.0.0:3003/health/live");

    App::new::<app::AppModule>()
        .transport(
            HttpTransport::new()
                .bind("0.0.0.0:3003")
                .mount("/mcp", |container| {
                    let c = container.clone();
                    nestrs_mcp::endpoint(move || WeatherController::from_container(&c))
                }),
        )
        .run()
        .await
}
