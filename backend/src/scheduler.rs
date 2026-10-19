use crate::routes::{self, AppState};
use anyhow::{bail, Context, Result};
use chrono_tz::Tz;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone)]
pub struct CronHandle {
    pub scheduler: JobScheduler,
    pub job_id: Uuid,
}

pub async fn init(state: Arc<AppState>) -> Result<()> {
    let config = state.snapshot().await;
    let scheduler = JobScheduler::new().await.context("creating scheduler")?;
    let job = build_job(
        state.clone(),
        &config.cron_schedule,
        &config.cron_timezone,
    )?;
    let job_id = scheduler.add(job).await.context("adding cron job")?;
    scheduler.start().await.context("starting scheduler")?;

    *state.scheduler.lock().await = Some(CronHandle {
        scheduler,
        job_id,
    });

    info!(
        schedule = %config.cron_schedule,
        timezone = %config.cron_timezone,
        "cron scheduler started"
    );
    Ok(())
}

pub async fn reschedule(state: Arc<AppState>, schedule: &str, timezone: &str) -> Result<()> {
    let job = build_job(state.clone(), schedule, timezone)?;
    let current = state.scheduler.lock().await.clone();
    let Some(handle) = current else {
        bail!("scheduler is not initialized");
    };

    handle
        .scheduler
        .remove(&handle.job_id)
        .await
        .context("removing previous cron job")?;
    let job_id = handle
        .scheduler
        .add(job)
        .await
        .context("adding rescheduled cron job")?;
    *state.scheduler.lock().await = Some(CronHandle {
        scheduler: handle.scheduler,
        job_id,
    });
    info!(schedule, timezone, "cron schedule updated");
    Ok(())
}

pub fn validate_schedule(schedule: &str, timezone: &str) -> Result<()> {
    let timezone = parse_timezone(timezone)?;
    Job::new_async_tz(schedule, timezone, |_job_id, _lock| {
        Box::pin(async {})
    })
    .map(|_| ())
    .map_err(|error| anyhow::anyhow!("invalid cron schedule {schedule:?}: {error}"))
}

fn build_job(state: Arc<AppState>, schedule: &str, timezone: &str) -> Result<Job> {
    let timezone = parse_timezone(timezone)?;
    let job = Job::new_async_tz(schedule, timezone, move |_job_id, _lock| {
        let state = state.clone();
        Box::pin(async move {
            info!("cron trigger fired");
            match routes::start_run(state, "cron").await {
                Ok(run_id) => info!(%run_id, "cron agent run started"),
                Err(error) => warn!(%error, "cron agent run was not started"),
            }
        })
    })
    .map_err(|error| anyhow::anyhow!("invalid cron schedule {schedule:?}: {error}"))?;
    Ok(job)
}

fn parse_timezone(value: &str) -> Result<Tz> {
    value
        .trim()
        .parse::<Tz>()
        .map_err(|_| anyhow::anyhow!("invalid cron timezone {value:?}"))
}
