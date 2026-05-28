use nestrs_auth::{AuthModule, JwtOptions, OAuth2Config, OAuth2Module};
use nestrs_core::module;
use nestrs_graphql::{GraphqlModule, GraphqlOptions};
use nestrs_health::HealthModule;
use nestrs_openapi::{OpenApiModule, OpenApiOptions};
use nestrs_orm::{DatabaseModule, DatabaseOptions};
use nestrs_server_timing::ServerTimingModule;
use nestrs_telemetry::TelemetryModule;
use nestrs_throttler::{Throttle, ThrottlerModule};

use crate::authn::AuthnModule;
use crate::authz::AuthzModule;
use crate::orgs::OrgsModule;
use crate::users::UsersModule;

#[module(
    imports = [
        DatabaseModule::for_root(DatabaseOptions {
            url: std::env::var("DATABASE_URL").unwrap_or_default(),
            ..Default::default()
        }),
        AuthModule::for_root(JwtOptions::new(
            std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me".into()),
        )),
        OAuth2Module::for_root(OAuth2Config {
            client_id: std::env::var("OAUTH_CLIENT_ID").unwrap_or_else(|_| "demo-client-id".into()),
            client_secret: std::env::var("OAUTH_CLIENT_SECRET")
                .unwrap_or_else(|_| "demo-client-secret".into()),
            auth_url: "https://github.com/login/oauth/authorize".into(),
            token_url: "https://github.com/login/oauth/access_token".into(),
            userinfo_url: "https://api.github.com/user".into(),
            redirect_url: std::env::var("OAUTH_REDIRECT_URL")
                .unwrap_or_else(|_| "http://localhost:3002/auth/oauth/callback".into()),
            scopes: vec!["read:user".into()],
        }),
        ThrottlerModule::for_root(Throttle::per_minute(60)),
        AuthnModule,
        AuthzModule,
        OrgsModule,
        UsersModule,
        GraphqlModule::for_root(GraphqlOptions {
            path: "/graphql".into(),
            playground: true,
            schema_path: concat!(env!("CARGO_MANIFEST_DIR"), "/schema.graphql").into(),
            emit_sdl: cfg!(debug_assertions),
        }),
        HealthModule,
        OpenApiModule::for_root(OpenApiOptions {
            title: "nestrs API".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Some("Demo API built with nestrs".into()),
        }),
        TelemetryModule,
        ServerTimingModule,
    ],
)]
pub struct AppModule;
