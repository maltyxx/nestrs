//! The workspace's shared database: schema migrations and demo-data seeding.
//!
//! Every app shares one database, so its schema and seed live in a single
//! application — `apps/db` — not under any one app. Migrations are Rust,
//! compiled into the `migrate` binary (shipped in the same container image as
//! the apps); the [`seed`] module backs the `seed` binary. Run them with
//! `just db up` / `just db seed` (or `cargo run -p db --bin migrate -- up`).
//! Migrations live under [`migrations`]; add new ones there.
pub mod seed;

mod migrations;

pub use migrations::Migrator;

/// Apply every pending migration to `conn` — the programmatic form of the
/// `migrate up` binary. Lets a test harness bring a throwaway database up to the
/// current schema before booting an app against it.
pub async fn migrate(conn: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
    use sea_orm_migration::MigratorTrait;
    Migrator::up(conn, None).await?;
    Ok(())
}
