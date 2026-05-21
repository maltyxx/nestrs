mod app;
mod graphql;
mod users;

use anyhow::Result;
use async_graphql_poem::GraphQL;
use nestrs_core::{Container, Module};
use nestrs_health::HealthController;
use nestrs_middleware::EndpointExt as _;
use nestrs_server_timing::ServerTiming;
use nestrs_telemetry::{OtelHttp, Telemetry, TelemetryConfig};
use poem::{listener::TcpListener, post, Route, Server};

#[tokio::main]
async fn main() -> Result<()> {
    let config = TelemetryConfig::from_env("api");
    let otel_http = OtelHttp::with_config(&config);
    let _telemetry = Telemetry::init_with(config)?;

    let container = app::AppModule::register(Container::builder()).build();
    let schema = graphql::build_schema(container.clone());

    let routes = Route::new()
        .at(
            "/graphql",
            post(GraphQL::new(schema)).get(graphql::playground),
        )
        .nest("/", HealthController::routes(&container))
        .interceptor(ServerTiming::new())
        .interceptor(otel_http);

    tracing::info!("api listening on http://0.0.0.0:3000");
    tracing::info!("  GraphQL playground: http://0.0.0.0:3000/graphql");
    tracing::info!("  Liveness probe:     http://0.0.0.0:3000/health/live");
    tracing::info!("  Readiness probe:    http://0.0.0.0:3000/health/ready");
    tracing::info!("  Startup probe:      http://0.0.0.0:3000/health/startup");

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(routes)
        .await?;
    Ok(())
}
