use crate::agent::pipeline;
use crate::config::Config;
use crate::email;
use crate::error::AppError;
use crate::models::{Hackathon, HackathonPage, RunAccepted, SearchRun};
use crate::scheduler::{self, CronHandle};
use crate::settings::{self, SettingsUpdate, SettingsView};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderName, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    /// Runtime configuration. Settings changes replace this snapshot; running
    /// agent jobs keep the snapshot they started with.
    pub config: Arc<RwLock<Config>>,
    pub allowed_origins: Arc<std::sync::RwLock<Vec<String>>>,
    /// The current API key plus any key accepted until the next restart.
    pub accepted_keys: Arc<std::sync::RwLock<Vec<String>>>,
    pub running: Arc<AtomicBool>,
    pub settings_lock: Arc<Mutex<()>>,
    pub scheduler: Arc<Mutex<Option<CronHandle>>>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let allowed_origins = config.frontend_origins.clone();
        let accepted_keys = vec![config.app_api_key.clone()];
        Self {
            pool,
            config: Arc::new(RwLock::new(config)),
            allowed_origins: Arc::new(std::sync::RwLock::new(allowed_origins)),
            accepted_keys: Arc::new(std::sync::RwLock::new(accepted_keys)),
            running: Arc::new(AtomicBool::new(false)),
            settings_lock: Arc::new(Mutex::new(())),
            scheduler: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn snapshot(&self) -> Config {
        self.config.read().await.clone()
    }

    pub fn set_origins(&self, origins: Vec<String>) {
        if let Ok(mut current) = self.allowed_origins.write() {
            *current = origins;
        }
    }

    pub fn accept_key(&self, key: &str) {
        if key.trim().is_empty() {
            return;
        }
        if let Ok(mut keys) = self.accepted_keys.write() {
            if !keys.iter().any(|existing| existing == key) {
                keys.push(key.to_owned());
            }
        }
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    let allowed_origins = state.allowed_origins.clone();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, _request| {
            let Ok(origin) = origin.to_str() else {
                return false;
            };
            allowed_origins
                .read()
                .map(|origins| {
                    origins
                        .iter()
                        .any(|allowed| allowed == "*" || allowed == origin)
                })
                .unwrap_or(false)
        }))
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::PUT, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static("x-api-key"),
        ])
        .max_age(std::time::Duration::from_secs(3600));

    Router::new()
        .route("/api/health", get(health))
        .route("/api/hackathons", get(list_hackathons))
        .route("/api/hackathons/{id}", get(get_hackathon).patch(patch_hackathon))
        .route("/api/agent/run", post(trigger_run))
        .route("/api/agent/runs", get(list_runs))
        .route("/api/agent/runs/{id}", get(get_run))
        .route("/api/email/digest", post(send_digest))
        .route("/api/email/test", post(test_email))
        .route("/api/settings", get(get_settings).put(update_settings))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_key,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// In-memory guard against overlapping runs on this instance. The database is
/// still the source of truth for run history and progress.
pub async fn start_run(state: Arc<AppState>, trigger: &str) -> Result<Uuid, AppError> {
    if state
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(AppError::Conflict(
            "an agent run is already in progress".to_owned(),
        ));
    }

    let config = state.snapshot().await;
    let run_id = match pipeline::create_run(
        &state.pool,
        config.search_queries.len(),
        trigger,
    )
    .await
    {
        Ok(run_id) => run_id,
        Err(error) => {
            state.running.store(false, Ordering::SeqCst);
            return Err(AppError::Internal(error));
        }
    };

    let run_state = state.clone();
    let run_config = Arc::new(config);
    tokio::spawn(async move {
        let result =
            pipeline::execute(run_id, run_state.pool.clone(), run_config).await;
        match result {
            Ok(summary) => tracing::info!(
                %run_id,
                found = summary.found_count,
                new = summary.new_count,
                "background agent run finished"
            ),
            Err(error) => tracing::error!(%run_id, error = %error, "background agent run failed"),
        }
        run_state.running.store(false, Ordering::SeqCst);
    });

    Ok(run_id)
}

async fn require_api_key(
    State(state): State<Arc<AppState>>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, AppError> {
    let method = request.method();
    let path = request.uri().path();
    let protected_read = path == "/api/settings";
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) && !protected_read {
        return Ok(next.run(request).await);
    }

    let header_key = request
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok());
    let bearer_key = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let supplied = header_key.or(bearer_key).unwrap_or("");
    let authorized = state
        .accepted_keys
        .read()
        .map(|keys| {
            keys.iter()
                .any(|key| constant_time_eq(supplied, key))
        })
        .unwrap_or(false);
    if !authorized {
        return Err(AppError::Unauthorized);
    }

    Ok(next.run(request).await)
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

async fn health(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, AppError> {
    sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map_err(|error| AppError::Unavailable(format!("database unavailable: {error}")))?;

    Ok(Json(json!({
        "status": "ok",
        "database": "up",
        "version": env!("CARGO_PKG_VERSION"),
        "time": Utc::now().to_rfc3339(),
    })))
}

#[derive(Debug, Default, Deserialize)]
struct HackathonQuery {
    status: Option<String>,
    tag: Option<String>,
    online: Option<bool>,
    location: Option<String>,
    deadline_before: Option<NaiveDate>,
    deadline_after: Option<NaiveDate>,
    min_prize: Option<f64>,
    q: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
    sort: Option<String>,
}

fn apply_filters<'a>(
    mut builder: QueryBuilder<'a, Postgres>,
    filters: &'a HackathonQuery,
    today: NaiveDate,
) -> Result<QueryBuilder<'a, Postgres>, AppError> {
    builder.push(" WHERE 1 = 1");

    if let Some(status) = filters.status.as_deref().map(str::trim) {
        match status {
            "" | "all" => {}
            "new" => {
                builder.push(" AND status IN ('new', 'notified')");
            }
            "closing_soon" => {
                builder.push(" AND status <> 'dismissed' AND registration_deadline IS NOT NULL");
                builder
                    .push(" AND registration_deadline BETWEEN ")
                    .push_bind(today)
                    .push(" AND ")
                    .push_bind(today + ChronoDuration::days(14));
            }
            "notified" | "saved" | "dismissed" => {
                builder.push(" AND status = ").push_bind(status);
            }
            other => {
                return Err(AppError::BadRequest(format!(
                    "unknown status filter {other:?}"
                )));
            }
        }
    }

    if let Some(tag) = filters.tag.as_deref().filter(|tag| !tag.trim().is_empty()) {
        builder.push(" AND tags @> ARRAY[").push_bind(tag).push("]::text[]");
    }

    if let Some(online) = filters.online {
        builder.push(" AND is_online = ").push_bind(online);
    }

    if let Some(location) = filters
        .location
        .as_deref()
        .filter(|location| !location.trim().is_empty())
    {
        builder
            .push(" AND location ILIKE ")
            .push_bind(format!("%{location}%"));
    }

    if let Some(deadline_after) = filters.deadline_after {
        builder
            .push(" AND registration_deadline >= ")
            .push_bind(deadline_after);
    }

    if let Some(deadline_before) = filters.deadline_before {
        builder
            .push(" AND registration_deadline <= ")
            .push_bind(deadline_before);
    }

    if let Some(min_prize) = filters.min_prize {
        builder
            .push(" AND prize_amount_cny >= ")
            .push_bind(min_prize);
    }

    if let Some(query) = filters.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let pattern = format!("%{query}%");
        builder.push(" AND (title ILIKE ").push_bind(pattern.clone());
        builder.push(" OR title_zh ILIKE ").push_bind(pattern.clone());
        builder.push(" OR organizer ILIKE ").push_bind(pattern.clone());
        builder.push(" OR description ILIKE ").push_bind(pattern.clone());
        builder
            .push(" OR location ILIKE ")
            .push_bind(pattern)
            .push(")");
    }

    Ok(builder)
}

fn sort_clause(sort: Option<&str>) -> &'static str {
    match sort.unwrap_or("newest") {
        "deadline_asc" => {
            " ORDER BY registration_deadline ASC NULLS LAST, created_at DESC"
        }
        "deadline_desc" => {
            " ORDER BY registration_deadline DESC NULLS LAST, created_at DESC"
        }
        "prize_desc" => " ORDER BY prize_amount_cny DESC NULLS LAST, created_at DESC",
        "confidence_desc" => " ORDER BY confidence DESC, created_at DESC",
        "oldest" => " ORDER BY created_at ASC",
        _ => " ORDER BY created_at DESC",
    }
}

async fn list_hackathons(
    State(state): State<Arc<AppState>>,
    Query(filters): Query<HackathonQuery>,
) -> Result<Json<HackathonPage>, AppError> {
    let page = filters.page.unwrap_or(1).max(1);
    let page_size = filters.page_size.unwrap_or(24).clamp(1, 100);
    let offset = (page - 1) * page_size;
    let today = Utc::now()
        .with_timezone(&chrono_tz::Asia::Shanghai)
        .date_naive();

    let mut items_builder = QueryBuilder::<Postgres>::new("SELECT * FROM hackathons");
    items_builder = apply_filters(items_builder, &filters, today)?;
    items_builder.push(sort_clause(filters.sort.as_deref()));
    items_builder
        .push(" LIMIT ")
        .push_bind(page_size)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<Hackathon> = items_builder
        .build_query_as()
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::from)?;

    let mut count_builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM hackathons");
    count_builder = apply_filters(count_builder, &filters, today)?;
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::from)?;

    let total_pages = if total == 0 {
        0
    } else {
        (total + page_size - 1) / page_size
    };

    Ok(Json(HackathonPage {
        items,
        total,
        page,
        page_size,
        total_pages,
    }))
}

async fn get_hackathon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Hackathon>, AppError> {
    let hackathon = sqlx::query_as::<_, Hackathon>("SELECT * FROM hackathons WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound(format!("hackathon {id}")))?;
    Ok(Json(hackathon))
}

#[derive(Debug, Deserialize)]
struct StatusUpdate {
    status: String,
}

async fn patch_hackathon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<StatusUpdate>,
) -> Result<Json<Hackathon>, AppError> {
    let status = body.status.trim().to_ascii_lowercase();
    if status != "saved" && status != "dismissed" {
        return Err(AppError::BadRequest(
            "status must be 'saved' or 'dismissed'".to_owned(),
        ));
    }

    let hackathon = sqlx::query_as::<_, Hackathon>(
        "UPDATE hackathons SET status = $2 WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&status)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound(format!("hackathon {id}")))?;

    Ok(Json(hackathon))
}

async fn trigger_run(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<RunAccepted>), AppError> {
    let run_id = start_run(state, "manual").await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(RunAccepted {
            run_id,
            status: "running",
        }),
    ))
}

#[derive(Debug, Deserialize)]
struct RunsQuery {
    limit: Option<i64>,
}

async fn list_runs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RunsQuery>,
) -> Result<Json<Vec<SearchRun>>, AppError> {
    let runs = pipeline::list_runs(&state.pool, query.limit.unwrap_or(20))
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(runs))
}

async fn get_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<SearchRun>, AppError> {
    let run = pipeline::get_run(&state.pool, id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("run {id}")))?;
    Ok(Json(run))
}

#[derive(Debug, Serialize)]
struct DigestResponse {
    sent: bool,
    count: usize,
    resend_id: Option<String>,
    hackathon_ids: Vec<Uuid>,
}

async fn send_digest(State(state): State<Arc<AppState>>) -> Result<Json<DigestResponse>, AppError> {
    let config = state.snapshot().await;
    if !config.email_configured() {
        return Err(AppError::Unavailable(
            "email is not configured; set RESEND_API_KEY and EMAIL_TO".to_owned(),
        ));
    }

    match email::send_digest(&state.pool, &config).await {
        Ok(Some(result)) => Ok(Json(DigestResponse {
            sent: true,
            count: result.count,
            resend_id: result.resend_id,
            hackathon_ids: result.hackathon_ids,
        })),
        Ok(None) => Ok(Json(DigestResponse {
            sent: false,
            count: 0,
            resend_id: None,
            hackathon_ids: Vec::new(),
        })),
        Err(error) => Err(AppError::Internal(error)),
    }
}

async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SettingsView>, AppError> {
    let config = state.snapshot().await;
    Ok(Json(SettingsView::from_config(&config, Vec::new())))
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(update): Json<SettingsUpdate>,
) -> Result<Json<SettingsView>, AppError> {
    let _guard = state.settings_lock.lock().await;
    let current = state.snapshot().await;

    let (next, restart_required) = settings::apply_update(&current, &update)
        .map_err(|error| AppError::BadRequest(error.to_string()))?;

    settings::save(&next).map_err(AppError::Internal)?;

    let key_changed = next.app_api_key != current.app_api_key;
    let origins_changed = next.frontend_origins != current.frontend_origins;
    let schedule_changed =
        next.cron_schedule != current.cron_schedule || next.cron_timezone != current.cron_timezone;

    {
        let mut config = state.config.write().await;
        *config = next.clone();
    }

    state.accept_key(&next.app_api_key);
    if key_changed {
        state.accept_key(&current.app_api_key);
    }
    if origins_changed {
        state.set_origins(next.frontend_origins.clone());
    }

    if schedule_changed {
        scheduler::reschedule(state.clone(), &next.cron_schedule, &next.cron_timezone)
            .await
            .map_err(AppError::Internal)?;
    }

    tracing::info!(
        restart_required = ?restart_required,
        "application settings updated"
    );

    Ok(Json(SettingsView::from_config(&next, restart_required)))
}

#[derive(Debug, Deserialize)]
struct TestEmailRequest {
    to: Option<String>,
}

#[derive(Debug, Serialize)]
struct TestEmailResponse {
    sent: bool,
    resend_id: Option<String>,
    recipients: Vec<String>,
}

async fn test_email(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TestEmailRequest>,
) -> Result<Json<TestEmailResponse>, AppError> {
    let config = state.snapshot().await;
    let has_override = payload
        .to
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    if config.resend_api_key.trim().is_empty() || (!config.email_configured() && !has_override) {
        return Err(AppError::Unavailable(
            "email is not configured; save a Resend API key and recipient first".to_owned(),
        ));
    }

    if let Some(to) = payload.to.as_deref().filter(|value| !value.trim().is_empty()) {
        settings::validate_email(to, "test recipient", false)
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
    }

    let result = email::send_test_email(&state.pool, &config, payload.to.as_deref())
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(TestEmailResponse {
        sent: true,
        resend_id: result.resend_id,
        recipients: result.recipients,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_comparison_is_exact() {
        assert!(constant_time_eq("secret", "secret"));
        assert!(!constant_time_eq("secret", "secrets"));
        assert!(!constant_time_eq("Secret", "secret"));
        assert!(!constant_time_eq("", "secret"));
    }

    #[test]
    fn sort_whitelist_never_echoes_input() {
        assert!(sort_clause(Some("deadline_asc")).contains("registration_deadline ASC"));
        assert!(sort_clause(Some("prize_desc")).contains("prize_amount_cny DESC"));
        assert_eq!(
            sort_clause(Some("; DROP TABLE hackathons")),
            " ORDER BY created_at DESC"
        );
    }
}
