use hackathon_hunter::config::Config;
use hackathon_hunter::db;
use hackathon_hunter::routes::{self, AppState};
use hackathon_hunter::scheduler;
use hackathon_hunter::settings;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,hackathon_hunter=debug,tower_http=info")),
        )
        .with_target(true)
        .compact()
        .init();

    let mut config = Config::from_env()?;
    match settings::apply_overrides(&mut config) {
        Ok(true) => info!(path = %settings::settings_path().display(), "persisted settings applied"),
        Ok(false) => {}
        Err(error) => warn!(error = %error, "could not load persisted settings; using environment"),
    }

    let pool = db::connect(&config.database_url, config.db_max_connections).await?;
    db::migrate(&pool).await?;

    match db::recover_interrupted_runs(&pool).await {
        Ok(0) => {}
        Ok(count) => warn!(count, "marked interrupted runs as failed"),
        Err(error) => warn!(error = %error, "could not recover interrupted runs"),
    }

    if !config.llm_configured() {
        warn!("NINEROUTER_API_KEY is missing; agent runs will fail until it is configured");
    }
    if !config.email_configured() {
        warn!("RESEND_API_KEY or EMAIL_TO is missing; digest emails are disabled");
    }

    let state = Arc::new(AppState::new(pool, config.clone()));
    scheduler::init(state.clone()).await?;
    let app = routes::router(state.clone());

    let address = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&address).await?;
    info!(%address, "Hackathon Hunter backend listening");
    info!(
        queries = config.search_queries.len(),
        schedule = %config.cron_schedule,
        timezone = %config.cron_timezone,
        "agent configured"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("server shut down cleanly");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("received Ctrl+C"),
        _ = terminate => info!("received SIGTERM"),
    }
}
