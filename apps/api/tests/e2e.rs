//! End-to-end against a **real, throwaway Postgres database**.
//!
//! `AppModule`'s `DatabaseModule` connects at boot, so this can't be faked. Each
//! test spins up a fresh [`EphemeralDatabase`] (migrated with the app's own
//! `Migrator`) and seeds its connection — the module's connect-factory is
//! short-circuited because a seed of the same type wins — then drops the database
//! when the test ends. From there the in-process harness drives the live
//! HTTP/OpenAPI surfaces: routing, the auth guard, and a real persisted
//! round-trip through SeaORM.
//!
//! Requires a reachable Postgres at `DATABASE_URL` (the devcontainer provides one).

use api::AppModule;
use nestrs_testing::{EphemeralDatabase, TestApp};
use serde_json::json;

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
    assert!(paths.get_opt("/orgs").is_some(), "OpenAPI paths include /orgs");
    assert!(
        paths.get_opt("/users").is_some(),
        "OpenAPI paths include /users",
    );
}

#[tokio::test]
async fn orgs_endpoint_requires_authentication() {
    let (_db, app) = boot().await;
    // No x-api-key / x-org-id headers: the AuthGuard short-circuits with 401.
    app.http()
        .get("/orgs")
        .send()
        .await
        .assert_status(poem::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_org_persists_and_is_listed() {
    let (_db, app) = boot().await;
    let name = "Acme E2E";

    let created = app
        .http()
        .post("/orgs")
        .header("x-api-key", "e2e-key")
        .header("x-org-id", "018f0000-0000-7000-8000-000000000000")
        .body_json(&json!({ "name": name }))
        .send()
        .await;
    created.assert_status_is_ok();
    let created_json = created.json().await;
    assert_eq!(created_json.value().object().get("name").string(), name);

    let listed = app
        .http()
        .get("/orgs")
        .header("x-api-key", "e2e-key")
        .header("x-org-id", "018f0000-0000-7000-8000-000000000000")
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
