use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn connect(database_url: &str, max_connections: u32) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(database_url)
        .await
        .context("connecting to PostgreSQL")
}

pub async fn migrate(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("running SQLx migrations")
}

/// Mark runs that were interrupted by a restart as failed so the UI does not
/// display a permanently running job.
pub async fn recover_interrupted_runs(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query(
        "UPDATE search_runs
         SET status = 'failed', phase = 'interrupted', finished_at = now(),
             errors = errors || jsonb_build_array(jsonb_build_object(
                 'message', 'Run interrupted by a backend restart', 'at', now()))
         WHERE status = 'running'",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
