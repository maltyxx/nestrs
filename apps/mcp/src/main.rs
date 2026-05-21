mod app;
mod weather;

use anyhow::Result;
use nestrs_core::{Container, Module};
use nestrs_health::HealthController;
use nestrs_middleware::EndpointExt as _;
use nestrs_server_timing::ServerTiming;
use nestrs_telemetry::{OtelHttp, Telemetry, TelemetryConfig};
use poem::{listener::TcpListener, Route, Server};

use crate::weather::WeatherController;

#[tokio::main]
async fn main() -> Result<()> {
    let config = TelemetryConfig::from_env("mcp");
    let otel_http = OtelHttp::with_config(&config);
    let _telemetry = Telemetry::init_with(config)?;

    let container = app::AppModule::register(Container::builder()).build();

    let mcp = {
        let c = container.clone();
        nestrs_mcp::endpoint(move || WeatherController::from_container(&c))
    };

    let routes = Route::new()
        .nest("/mcp", mcp)
        .nest("/", HealthController::routes(&container))
        .interceptor(ServerTiming::new())
        .interceptor(otel_http);

    tracing::info!("mcp listening on http://0.0.0.0:3001");
    tracing::info!("  MCP streamable HTTP: http://0.0.0.0:3001/mcp");
    tracing::info!("  Liveness probe:      http://0.0.0.0:3001/health/live");

    Server::new(TcpListener::bind("0.0.0.0:3001"))
        .run(routes)
        .await?;
    Ok(())
}
