//! End-to-end against a **real, throwaway Postgres database**.
//!
//! `AppModule`'s `DatabaseModule` connects at boot, so this can't be faked. Each
//! test spins up a fresh [`EphemeralDatabase`] (migrated with the app's own
//! `Migrator`) and seeds its connection — the module's connect-factory is
//! short-circuited because a seed of the same type wins — then drops the database
//! when the test ends. From there the in-process harness drives the live
//! HTTP/OpenAPI surfaces: routing, the bearer-JWT auth guard, the OAuth redirect,
//! and a real persisted round-trip through SeaORM.
//!
//! Requires a reachable Postgres at `DATABASE_URL` (the devcontainer provides one).

use api::AppModule;
use nestrs_testing::{EphemeralDatabase, TestApp};
use poem::http::{header, StatusCode};
use serde_json::json;

const ORG_ID: &str = "018f0000-0000-7000-8000-000000000000";

/// A fresh database + booted app per test, so the tests are fully isolated and
/// the database is reclaimed (RAII) when the returned guard drops at scope end.
async fn boot() -> (EphemeralDatabase, TestApp) {
    let db = EphemeralDatabase::create::<db::Migrator>()
        .await
        .expect("create + migrate a throwaway database");
    let app = TestApp::builder()
        .module::<AppModule>()
        .with_test_telemetry()
        .provide_arc(db.connection())
        .build()
        .await
        .expect("AppModule boots against the throwaway database");
    (db, app)
}

/// Mint a bearer token through the real `POST /auth/login` issuer.
async fn login(app: &TestApp) -> String {
    let resp = app
        .http()
        .post("/auth/login")
        .body_json(&json!({ "org_id": ORG_ID, "roles": ["admin"] }))
        .send()
        .await;
    resp.assert_status_is_ok();
    resp.json()
        .await
        .value()
        .object()
        .get("access_token")
        .string()
        .to_owned()
}

#[tokio::test]
async fn health_live_probe_is_ok() {
    let (_db, app) = boot().await;
    app.http()
        .get("/health/live")
        .send()
        .await
        .assert_status_is_ok();
}

#[tokio::test]
async fn openapi_document_describes_the_routes() {
    let (_db, app) = boot().await;
    let resp = app.http().get("/api-json").send().await;
    resp.assert_status_is_ok();
    let doc = resp.json().await;
    let paths = doc.value().object().get("paths").object();
    assert!(
        paths.get_opt("/orgs").is_some(),
        "OpenAPI paths include /orgs"
    );
    assert!(
        paths.get_opt("/users").is_some(),
        "OpenAPI paths include /users",
    );
    assert!(
        paths.get_opt("/auth/login").is_some(),
        "OpenAPI paths include /auth/login",
    );
}

#[tokio::test]
async fn protected_route_rejects_a_missing_or_bogus_bearer_token() {
    let (_db, app) = boot().await;

    // No Authorization header: the AuthGuard short-circuits with 401.
    app.http()
        .get("/orgs")
        .send()
        .await
        .assert_status(StatusCode::UNAUTHORIZED);

    // A malformed token does not verify: also 401.
    app.http()
        .get("/orgs")
        .header(header::AUTHORIZATION, "Bearer not-a-real-jwt")
        .send()
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_org_persists_and_is_listed_with_a_bearer_token() {
    let (_db, app) = boot().await;
    let token = login(&app).await;
    let bearer = format!("Bearer {token}");
    let name = "Acme E2E";

    let created = app
        .http()
        .post("/orgs")
        .header(header::AUTHORIZATION, &bearer)
        .body_json(&json!({ "name": name }))
        .send()
        .await;
    created.assert_status_is_ok();
    let created_json = created.json().await;
    assert_eq!(created_json.value().object().get("name").string(), name);

    let listed = app
        .http()
        .get("/orgs")
        .header(header::AUTHORIZATION, &bearer)
        .send()
        .await;
    listed.assert_status_is_ok();
    let names: Vec<String> = listed
        .json()
        .await
        .value()
        .array()
        .iter()
        .map(|org| org.object().get("name").string().to_owned())
        .collect();
    assert!(
        names.contains(&name.to_string()),
        "the freshly created org appears in the list: {names:?}",
    );
}

#[tokio::test]
async fn login_is_rate_limited() {
    let (_db, app) = boot().await;
    let body = json!({ "org_id": ORG_ID, "roles": ["user"] });

    // `#[meta(Throttle::per_minute(5))]` on the login route: the first 5 pass.
    for _ in 0..5 {
        app.http()
            .post("/auth/login")
            .body_json(&body)
            .send()
            .await
            .assert_status_is_ok();
    }
    // The 6th within the window is rejected by the ThrottlerGuard.
    app.http()
        .post("/auth/login")
        .body_json(&body)
        .send()
        .await
        .assert_status(StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn oauth_begin_redirects_to_the_provider_with_a_state_cookie() {
    let (_db, app) = boot().await;
    // The OAuth guard challenges the initiating request with a 302 to the
    // provider and sets the signed-transaction cookie — no network call needed.
    let resp = app.http().get("/auth/oauth").send().await;
    resp.assert_status(StatusCode::FOUND);
    resp.assert_header_exist(header::LOCATION);
    resp.assert_header_exist(header::SET_COOKIE);
}
