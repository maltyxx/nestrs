//! End-to-end: boot the real MCP `AppModule` through the in-process harness and
//! exercise its live HTTP surfaces — the health probes and the self-mounted MCP
//! streamable-HTTP endpoint. No network: the weather upstream is only hit when a
//! tool is *invoked*, which these tests don't do. The regression guarded is the
//! app's wiring (DI graph + access-graph for the tool's injected
//! `dyn WeatherProvider` + the `TelemetryModule` boot guard + endpoint mounting).

use mcp::AppModule;
use nestrs_core::DiscoveryService;
use nestrs_http::HttpEndpointMeta;
use nestrs_testing::TestApp;
use serde_json::json;

/// `AppModule` imports `TelemetryModule`, which panics at boot unless telemetry
/// is initialised; `with_test_telemetry` satisfies that guard.
async fn boot() -> TestApp {
    TestApp::builder()
        .module::<AppModule>()
        .with_test_telemetry()
        .build()
        .await
        .expect("AppModule boots")
}

#[tokio::test]
async fn health_live_probe_is_ok() {
    let app = boot().await;
    app.http()
        .get("/health/live")
        .send()
        .await
        .assert_status_is_ok();
}

#[tokio::test]
async fn weather_tool_self_mounts_the_mcp_endpoint() {
    let app = boot().await;
    let endpoints = DiscoveryService::new(app.container()).meta::<HttpEndpointMeta>();
    assert!(
        endpoints
            .iter()
            .any(|d| d.meta.label() == "mcp" && d.meta.path() == "/mcp"),
        "the #[mcp] WeatherController self-mounts an MCP endpoint at /mcp",
    );
}

#[tokio::test]
async fn mcp_endpoint_accepts_an_initialize_request() {
    let app = boot().await;

    // A well-formed MCP `initialize` over the streamable-HTTP transport: the
    // server requires both JSON and SSE in `Accept`. A 200 proves the rmcp
    // service is actually mounted and serving, not just discovered.
    let resp = app
        .http()
        .post("/mcp")
        // rmcp's transport rejects a request with no Host / :authority (400).
        .header("host", "localhost")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body_json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "nestrs-e2e", "version": "0" }
            }
        }))
        .send()
        .await;

    resp.assert_status_is_ok();
}
