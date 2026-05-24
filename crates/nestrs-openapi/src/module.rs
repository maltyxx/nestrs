//! `OpenApiModule` — import it to serve the auto-generated OpenAPI document and
//! Swagger UI over HTTP.

use nestrs_core::{ContainerBuilder, Module};
use nestrs_http::HttpEndpointMeta;
use poem::{get, Route};

use crate::document::build_document;
use crate::ui;

// NestJS convention (`SwaggerModule.setup('api', …)`): UI at `/api`, document
// at `/api-json`. The OpenAPI spec mandates no serving path, so we follow the
// reference NestJS surface this framework mirrors.
const DOCS_PATH: &str = "/api";
const SPEC_PATH: &str = "/api-json";

/// Add to a `#[module(imports = [...])]` to expose:
/// - `GET /api-json` — the OpenAPI 3.1 document, and
/// - `GET /api` — bundled Swagger UI.
///
/// Like [`nestrs_graphql::GraphqlModule`], it self-mounts via an
/// [`HttpEndpointMeta`]: there is nothing to wire in `main.rs`. The spec is
/// composed from every `#[controller]` linked into the binary, so importing
/// this module is the only step. This is the REST analog of NestJS's
/// `SwaggerModule.setup(...)`.
pub struct OpenApiModule;

impl Module for OpenApiModule {
    fn register(builder: ContainerBuilder) -> ContainerBuilder {
        builder.provide_meta(HttpEndpointMeta::new(
            DOCS_PATH,
            "openapi",
            |container, route: Route| {
                // Built once at configure time — the container is fully
                // assembled, so every controller is present.
                let document = build_document(container, "nestrs API", "0.1.0");
                let spec = serde_json::to_string_pretty(&document)
                    .unwrap_or_else(|_| document.to_string());
                route
                    .at(SPEC_PATH, get(ui::spec_endpoint(spec)))
                    .at(DOCS_PATH, get(ui::swagger_index))
                    .at("/api/swagger-ui.css", get(ui::swagger_css))
                    .at("/api/swagger-ui-bundle.js", get(ui::swagger_bundle))
                    .at(
                        "/api/swagger-ui-standalone-preset.js",
                        get(ui::swagger_preset),
                    )
            },
        ))
    }
}
