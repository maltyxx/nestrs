mod app;
mod graphql;
mod users;

use anyhow::Result;
use async_graphql_poem::GraphQL;
use nestrs_core::App;
use nestrs_http::HttpTransport;
use nestrs_telemetry::Telemetry;
use poem::post;

#[tokio::main]
async fn main() -> Result<()> {
    let _telemetry = Telemetry::init("api")?;

    tracing::info!("api listening on http://0.0.0.0:3002");
    tracing::info!("  GraphQL playground: http://0.0.0.0:3002/graphql");
    tracing::info!("  Liveness probe:     http://0.0.0.0:3002/health/live");
    tracing::info!("  Readiness probe:    http://0.0.0.0:3002/health/ready");
    tracing::info!("  Startup probe:      http://0.0.0.0:3002/health/startup");

    App::new::<app::AppModule>()
        .transport(
            HttpTransport::new()
                .bind("0.0.0.0:3002")
                .mount("/graphql", |container| {
                    let schema = graphql::AppSchema::build(container.clone());
                    post(GraphQL::new(schema)).get(graphql::playground)
                }),
        )
        .run()
        .await
}
