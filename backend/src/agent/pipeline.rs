use crate::agent::extract::{ExtractionDecision, ExtractedHackathon, Extractor};
use crate::agent::scrape::{ScrapedPage, Scraper};
use crate::agent::search::{dedupe_candidates, SearchCandidate, SearxClient};
use crate::agent::{dedupe_key, normalize_url};
use crate::config::Config;
use crate::email;
use anyhow::{bail, Context, Result};
use chrono::{Duration as ChronoDuration, Utc};
use chrono_tz::Asia::Shanghai;
use futures::stream::{self, StreamExt};
use reqwest::redirect::Policy;
use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tracing::Instrument;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    pub run_id: Uuid,
    pub found_count: i32,
    pub new_count: i32,
    pub sources_fetched: usize,
    pub sources_failed: usize,
}

#[derive(Debug, Default)]
struct Counters {
    found: i32,
    new: i32,
    fetched: usize,
    failed: usize,
    expired: usize,
    skipped_processed: usize,
}

#[derive(Debug, Clone)]
struct ScrapeOutcome {
    source_key: String,
    result: std::result::Result<ScrapedPage, String>,
}

#[derive(Debug, Clone)]
struct ExtractOutcome {
    source_key: String,
    page: ScrapedPage,
    result: std::result::Result<ExtractionDecision, String>,
}

enum StoreOutcome {
    Inserted,
    Updated,
    Unchanged,
}

/// Create the run row synchronously so the API can return an id before the
/// background job starts.
pub async fn create_run(pool: &PgPool, queries_count: usize, trigger: &str) -> Result<Uuid> {
    let trigger = if trigger == "cron" { "cron" } else { "manual" };
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO search_runs (trigger, queries_count, phase, progress)
         VALUES ($1, $2, 'starting', 0)
         RETURNING id",
    )
    .bind(trigger)
    .bind(queries_count as i32)
    .fetch_one(pool)
    .await
    .context("creating search run")?;
    Ok(id)
}

pub async fn execute(run_id: Uuid, pool: PgPool, config: Arc<Config>) -> Result<RunSummary> {
    let span = tracing::info_span!("agent_run", %run_id);
    let result = execute_inner(run_id, &pool, &config).instrument(span).await;

    if let Err(error) = &result {
        tracing::error!(%run_id, error = %error, "agent run failed");
        let message = format!("Run failed: {error}");
        let _ = append_error(&pool, run_id, &message, None).await;
        let _ = log(&pool, run_id, "error", &message).await;
        let _ = sqlx::query(
            "UPDATE search_runs
             SET status = 'failed', phase = 'failed', progress = 100, finished_at = now()
             WHERE id = $1 AND status = 'running'",
        )
        .bind(run_id)
        .execute(&pool)
        .await;
    }

    result
}

async fn execute_inner(run_id: Uuid, pool: &PgPool, config: &Config) -> Result<RunSummary> {
    if !config.llm_configured() {
        bail!("NINEROUTER_API_KEY is not configured; cannot extract hackathons");
    }

    let http = Client::builder()
        .user_agent(&config.user_agent)
        .redirect(Policy::limited(10))
        .timeout(Duration::from_secs(config.searxng_timeout_secs.max(30)))
        .pool_idle_timeout(Duration::from_secs(30))
        .build()
        .context("building HTTP client")?;

    let searcher = SearxClient::new(http.clone(), config);
    let scraper = Arc::new(Scraper::new(http.clone(), config));
    let extractor = Arc::new(Extractor::new(http.clone(), config));
    let mut counters = Counters::default();

    set_phase(pool, run_id, "discovering", 5).await?;
    log(
        pool,
        run_id,
        "info",
        &format!(
            "Starting discovery with {} queries via {}",
            config.search_queries.len(),
            config.searxng_url
        ),
    )
    .await?;

    let mut candidates: Vec<SearchCandidate> = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();
    let gather_limit = config.max_urls_per_run.saturating_mul(4).max(1);
    let mut query_failures = 0usize;
    let mut query_successes = 0usize;

    for query in &config.search_queries {
        match searcher.search(query).await {
            Ok(results) => {
                query_successes += 1;
                let count = results.len();
                for candidate in results {
                    if candidates.len() >= gather_limit {
                        break;
                    }
                    if seen_urls.insert(candidate.normalized_url.clone()) {
                        candidates.push(candidate);
                    }
                }
                log(
                    pool,
                    run_id,
                    "info",
                    &format!("Query {query:?} returned {count} usable results"),
                )
                .await?;
            }
            Err(error) => {
                query_failures += 1;
                let message = format!("Search failed for {query:?}: {error}");
                tracing::warn!(%run_id, %error, query, "search query failed");
                append_error(pool, run_id, &message, Some(json!({ "query": query }))).await?;
                log(pool, run_id, "warn", &message).await?;
            }
        }
    }

    if query_successes == 0 && query_failures > 0 {
        bail!("every search query failed; SearXNG may be unavailable");
    }

    let processed = load_processed_sources(pool, config.source_refresh_hours).await?;
    let before_filter = candidates.len();
    candidates.retain(|candidate| !processed.contains(&candidate.normalized_url));
    counters.skipped_processed = before_filter.saturating_sub(candidates.len());
    candidates = dedupe_candidates(candidates, config.max_urls_per_run);

    log(
        pool,
        run_id,
        "info",
        &format!(
            "Discovered {} unique URLs ({} already processed, {} queued for scraping)",
            before_filter,
            counters.skipped_processed,
            candidates.len()
        ),
    )
    .await?;

    if candidates.is_empty() {
        finish_run(pool, run_id, &counters).await?;
        return Ok(RunSummary {
            run_id,
            found_count: 0,
            new_count: 0,
            sources_fetched: 0,
            sources_failed: 0,
        });
    }

    set_phase(pool, run_id, "scraping", 30).await?;

    for candidate in &candidates {
        record_source_started(pool, run_id, candidate).await?;
    }

    let scrape_outcomes: Vec<ScrapeOutcome> = stream::iter(candidates.iter().cloned().map(
        |candidate| {
            let scraper = scraper.clone();
            let pool = pool.clone();
            async move { scrape_one(&scraper, &pool, run_id, candidate).await }
        },
    ))
    .buffer_unordered(config.scrape_concurrency.max(1))
    .collect()
    .await;

    let mut pages = Vec::new();
    for outcome in scrape_outcomes {
        match outcome.result {
            Ok(page) => {
                counters.fetched += 1;
                pages.push((outcome.source_key, page));
            }
            Err(error) => {
                counters.failed += 1;
                let message = format!("Scrape failed for {}: {error}", outcome.source_key);
                tracing::warn!(%run_id, url = %outcome.source_key, %error, "scrape failed");
                append_error(
                    pool,
                    run_id,
                    &message,
                    Some(json!({ "url": outcome.source_key })),
                )
                .await?;
            }
        }
    }

    log(
        pool,
        run_id,
        if counters.failed > 0 { "warn" } else { "info" },
        &format!(
            "Scraped {} pages successfully, {} failed",
            counters.fetched, counters.failed
        ),
    )
    .await?;

    if pages.is_empty() {
        finish_run(pool, run_id, &counters).await?;
        return Ok(RunSummary {
            run_id,
            found_count: 0,
            new_count: 0,
            sources_fetched: counters.fetched,
            sources_failed: counters.failed,
        });
    }

    set_phase(pool, run_id, "extracting", 55).await?;

    let extract_outcomes: Vec<ExtractOutcome> = stream::iter(pages.into_iter().map(
        |(source_key, page)| {
            let extractor = extractor.clone();
            let pool = pool.clone();
            async move {
                let result = extractor
                    .extract(&page.url, &page.title, &page.text)
                    .await
                    .map_err(|error| error.to_string());
                record_extraction(&pool, run_id, &source_key, &result).await;
                ExtractOutcome {
                    source_key,
                    page,
                    result,
                }
            }
        },
    ))
    .buffer_unordered(config.extract_concurrency.max(1))
    .collect()
    .await;

    set_phase(pool, run_id, "storing", 80).await?;

    let mut accepted: Vec<(String, ExtractedHackathon)> = Vec::new();
    for outcome in extract_outcomes {
        match outcome.result {
            Ok(ExtractionDecision::Accepted(item)) => {
                log(
                    pool,
                    run_id,
                    "info",
                    &format!(
                        "Extracted {:?} from {} ({:.2} confidence)",
                        item.title, outcome.page.url, item.confidence
                    ),
                )
                .await?;
                accepted.push((outcome.page.content_hash.clone(), item));
            }
            Ok(ExtractionDecision::Rejected { reason }) => {
                log(
                    pool,
                    run_id,
                    "info",
                    &format!("Skipped {}: {reason}", outcome.page.url),
                )
                .await?;
            }
            Err(error) => {
                let message = format!("Extraction failed for {}: {error}", outcome.source_key);
                tracing::warn!(%run_id, url = %outcome.source_key, %error, "extraction failed");
                append_error(
                    pool,
                    run_id,
                    &message,
                    Some(json!({ "url": outcome.source_key })),
                )
                .await?;
            }
        }
    }

    let today = Utc::now().with_timezone(&Shanghai).date_naive();
    for (content_hash, item) in accepted {
        if let Some(deadline) = item.registration_deadline {
            if deadline < today {
                counters.expired += 1;
                log(
                    pool,
                    run_id,
                    "info",
                    &format!("Skipped expired event {:?} (deadline {deadline})", item.title),
                )
                .await?;
                continue;
            }
        }

        match store_hackathon(pool, &item, &content_hash).await {
            Ok(StoreOutcome::Inserted) => {
                counters.found += 1;
                counters.new += 1;
            }
            Ok(StoreOutcome::Updated) | Ok(StoreOutcome::Unchanged) => {
                counters.found += 1;
            }
            Err(error) => {
                let message = format!("Failed to store {:?}: {error}", item.title);
                tracing::error!(%run_id, error = %error, title = %item.title, "store failed");
                append_error(pool, run_id, &message, None).await?;
            }
        }
    }

    if counters.expired > 0 {
        log(
            pool,
            run_id,
            "info",
            &format!("Skipped {} events with expired deadlines", counters.expired),
        )
        .await?;
    }

    let summary = finish_run(pool, run_id, &counters).await?;

    if config.auto_send_email && summary.new_count > 0 {
        if config.email_configured() {
            match email::send_digest(pool, config).await {
                Ok(Some(result)) => {
                    log(
                        pool,
                        run_id,
                        "info",
                        &format!(
                            "Digest email sent with {} hackathons (Resend id {})",
                            result.count,
                            result.resend_id.as_deref().unwrap_or("unknown")
                        ),
                    )
                    .await?;
                }
                Ok(None) => {
                    log(pool, run_id, "info", "No unexpired new hackathons; digest skipped")
                        .await?;
                }
                Err(error) => {
                    let message = format!("Digest email failed: {error}");
                    tracing::error!(%run_id, error = %error, "digest email failed");
                    append_error(pool, run_id, &message, None).await?;
                    log(pool, run_id, "error", &message).await?;
                }
            }
        } else {
            log(
                pool,
                run_id,
                "warn",
                "New hackathons found but RESEND_API_KEY/EMAIL_TO are not configured",
            )
            .await?;
        }
    }

    Ok(RunSummary {
        run_id,
        found_count: summary.found_count,
        new_count: summary.new_count,
        sources_fetched: counters.fetched,
        sources_failed: counters.failed,
    })
}

async fn load_processed_sources(pool: &PgPool, refresh_hours: i64) -> Result<HashSet<String>> {
    let cutoff = Utc::now() - ChronoDuration::hours(refresh_hours.max(0));
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT normalized_url FROM sources
         WHERE status IN ('fetched', 'extracted', 'skipped')
           AND fetched_at IS NOT NULL
           AND fetched_at >= $1",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .context("loading already processed sources")?;
    Ok(rows.into_iter().map(|(url,)| url).collect())
}

async fn record_source_started(pool: &PgPool, run_id: Uuid, candidate: &SearchCandidate) -> Result<()> {
    let domain = Url::parse(&candidate.url)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .unwrap_or_default();
    sqlx::query(
        "INSERT INTO sources (run_id, url, normalized_url, domain, status)
         VALUES ($1, $2, $3, $4, 'pending')
         ON CONFLICT (normalized_url) DO UPDATE
         SET run_id = EXCLUDED.run_id, url = EXCLUDED.url,
             status = 'pending', error = NULL, updated_at = now()",
    )
    .bind(run_id)
    .bind(&candidate.url)
    .bind(&candidate.normalized_url)
    .bind(&domain)
    .execute(pool)
    .await
    .context("recording source")?;
    Ok(())
}

async fn scrape_one(
    scraper: &Scraper,
    pool: &PgPool,
    run_id: Uuid,
    candidate: SearchCandidate,
) -> ScrapeOutcome {
    match scraper.fetch(&candidate.url).await {
        Ok(page) => {
            let update = sqlx::query(
                "UPDATE sources
                 SET status = 'fetched', http_status = $1, title = $2,
                     content_hash = $3, fetched_at = now(), error = NULL
                 WHERE normalized_url = $4",
            )
            .bind(page.http_status as i32)
            .bind(&page.title)
            .bind(&page.content_hash)
            .bind(&candidate.normalized_url)
            .execute(pool)
            .await;
            if let Err(error) = update {
                tracing::error!(%run_id, url = %candidate.url, %error, "failed to update source");
            }
            ScrapeOutcome {
                source_key: candidate.normalized_url,
                result: Ok(page),
            }
        }
        Err(error) => {
            let message = error.to_string();
            let http_status = match &error {
                crate::agent::scrape::ScrapeError::HttpStatus(code) => Some(*code as i32),
                _ => None,
            };
            let update = sqlx::query(
                "UPDATE sources
                 SET status = 'failed', http_status = $1, error = $2, fetched_at = now()
                 WHERE normalized_url = $3",
            )
            .bind(http_status)
            .bind(&message)
            .bind(&candidate.normalized_url)
            .execute(pool)
            .await;
            if let Err(db_error) = update {
                tracing::error!(%run_id, url = %candidate.url, %db_error, "failed to record source error");
            }
            ScrapeOutcome {
                source_key: candidate.normalized_url,
                result: Err(message),
            }
        }
    }
}

async fn record_extraction(
    pool: &PgPool,
    run_id: Uuid,
    source_key: &str,
    result: &std::result::Result<ExtractionDecision, String>,
) {
    let update = match result {
        Ok(ExtractionDecision::Accepted(_)) => {
            sqlx::query(
                "UPDATE sources SET status = 'extracted', error = NULL
                 WHERE normalized_url = $1",
            )
            .bind(source_key)
            .execute(pool)
            .await
        }
        Ok(ExtractionDecision::Rejected { reason }) => {
            sqlx::query(
                "UPDATE sources SET status = 'skipped', error = $1
                 WHERE normalized_url = $2",
            )
            .bind(reason)
            .bind(source_key)
            .execute(pool)
            .await
        }
        Err(error) => {
            sqlx::query(
                "UPDATE sources SET status = 'failed', error = $1
                 WHERE normalized_url = $2",
            )
            .bind(error)
            .bind(source_key)
            .execute(pool)
            .await
        }
    };

    if let Err(error) = update {
        tracing::error!(%run_id, source = source_key, %error, "failed to record extraction status");
    }
}

async fn store_hackathon(
    pool: &PgPool,
    item: &ExtractedHackathon,
    hash: &str,
) -> Result<StoreOutcome> {
    let source_normalized = normalize_url(&item.source_url)
        .unwrap_or_else(|| item.source_url.trim().to_owned());
    let registration_normalized = item.registration_url.as_deref().and_then(normalize_url);
    let key = dedupe_key(
        item.registration_url.as_deref(),
        &item.source_url,
        &item.title,
    );

    let existing = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, status, content_hash FROM hackathons
         WHERE dedupe_key = $1
            OR (source_url_normalized = $2 AND lower(title) = lower($3))
            OR ($4::text IS NOT NULL AND registration_url_normalized = $4)
         ORDER BY (dedupe_key = $1) DESC
         LIMIT 1",
    )
    .bind(&key)
    .bind(&source_normalized)
    .bind(&item.title)
    .bind(&registration_normalized)
    .fetch_optional(pool)
    .await
    .context("looking up existing hackathon")?;

    if let Some((id, _status, existing_hash)) = existing {
        if existing_hash == hash {
            sqlx::query("UPDATE hackathons SET last_seen_at = now() WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?;
            return Ok(StoreOutcome::Unchanged);
        }

        sqlx::query(
            "UPDATE hackathons SET
                title = $2, title_zh = $3, organizer = $4, description = $5,
                location = $6, is_online = $7, start_date = $8, end_date = $9,
                registration_deadline = $10, registration_url = $11,
                registration_url_normalized = $12, source_url = $13,
                source_url_normalized = $14, prize_summary = $15,
                prize_amount_cny = $16, tracks_or_themes = $17, eligibility = $18,
                tags = $19, confidence = $20, dedupe_key = $21, content_hash = $22,
                last_seen_at = now(), content_changed_at = now(),
                status = CASE WHEN status IN ('new', 'notified') THEN 'new' ELSE status END
             WHERE id = $1",
        )
        .bind(id)
        .bind(&item.title)
        .bind(&item.title_zh)
        .bind(&item.organizer)
        .bind(&item.description)
        .bind(&item.location)
        .bind(item.is_online)
        .bind(item.start_date)
        .bind(item.end_date)
        .bind(item.registration_deadline)
        .bind(&item.registration_url)
        .bind(&registration_normalized)
        .bind(&item.source_url)
        .bind(&source_normalized)
        .bind(&item.prize_summary)
        .bind(item.prize_amount_cny)
        .bind(&item.tracks_or_themes)
        .bind(&item.eligibility)
        .bind(&item.tags)
        .bind(item.confidence)
        .bind(&key)
        .bind(hash)
        .execute(pool)
        .await
        .context("updating hackathon")?;

        return Ok(StoreOutcome::Updated);
    }

    let insert = sqlx::query(
        "INSERT INTO hackathons (
            title, title_zh, organizer, description, location, is_online,
            start_date, end_date, registration_deadline, registration_url,
            registration_url_normalized, source_url, source_url_normalized,
            prize_summary, prize_amount_cny, tracks_or_themes, eligibility, tags,
            confidence, dedupe_key, content_hash
         ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
            $16, $17, $18, $19, $20, $21
         )",
    )
    .bind(&item.title)
    .bind(&item.title_zh)
    .bind(&item.organizer)
    .bind(&item.description)
    .bind(&item.location)
    .bind(item.is_online)
    .bind(item.start_date)
    .bind(item.end_date)
    .bind(item.registration_deadline)
    .bind(&item.registration_url)
    .bind(&registration_normalized)
    .bind(&item.source_url)
    .bind(&source_normalized)
    .bind(&item.prize_summary)
    .bind(item.prize_amount_cny)
    .bind(&item.tracks_or_themes)
    .bind(&item.eligibility)
    .bind(&item.tags)
    .bind(item.confidence)
    .bind(&key)
    .bind(hash)
    .execute(pool)
    .await;

    match insert {
        Ok(_) => Ok(StoreOutcome::Inserted),
        Err(sqlx::Error::Database(db_error)) if db_error.code().as_deref() == Some("23505") => {
            // A concurrent writer inserted the same event between our lookup and
            // insert. Treat it as an update instead of failing the run.
            let id: Option<Uuid> =
                sqlx::query_scalar("SELECT id FROM hackathons WHERE dedupe_key = $1 LIMIT 1")
                    .bind(&key)
                    .fetch_optional(pool)
                    .await?;
            if let Some(id) = id {
                sqlx::query("UPDATE hackathons SET last_seen_at = now() WHERE id = $1")
                    .bind(id)
                    .execute(pool)
                    .await?;
                Ok(StoreOutcome::Unchanged)
            } else {
                Err(anyhow::Error::new(db_error))
                    .context("unique violation without a matching hackathon")
            }
        }
        Err(error) => Err(error).context("inserting hackathon"),
    }
}

struct FinishSummary {
    found_count: i32,
    new_count: i32,
}

async fn finish_run(pool: &PgPool, run_id: Uuid, counters: &Counters) -> Result<FinishSummary> {
    sqlx::query(
        "UPDATE search_runs
         SET status = 'completed', phase = 'completed', progress = 100,
             finished_at = now(), found_count = $2, new_count = $3
         WHERE id = $1",
    )
    .bind(run_id)
    .bind(counters.found)
    .bind(counters.new)
    .execute(pool)
    .await
    .context("finishing search run")?;

    log(
        pool,
        run_id,
        "info",
        &format!(
            "Run complete: {} hackathons found, {} new, {} pages failed",
            counters.found, counters.new, counters.failed
        ),
    )
    .await?;

    Ok(FinishSummary {
        found_count: counters.found,
        new_count: counters.new,
    })
}

async fn set_phase(pool: &PgPool, run_id: Uuid, phase: &str, progress: i32) -> Result<()> {
    sqlx::query("UPDATE search_runs SET phase = $2, progress = $3 WHERE id = $1")
        .bind(run_id)
        .bind(phase)
        .bind(progress)
        .execute(pool)
        .await
        .context("updating run phase")?;
    Ok(())
}

async fn log(pool: &PgPool, run_id: Uuid, level: &str, message: &str) -> Result<()> {
    tracing::info!(%run_id, level, message, "agent log");
    let entry = json!({
        "at": Utc::now().to_rfc3339(),
        "level": level,
        "message": message,
    });
    sqlx::query(
        "UPDATE search_runs
         SET logs = logs || jsonb_build_array($2::jsonb)
         WHERE id = $1 AND jsonb_array_length(logs) < 1000",
    )
    .bind(run_id)
    .bind(entry)
    .execute(pool)
    .await
    .context("appending run log")?;
    Ok(())
}

async fn append_error(
    pool: &PgPool,
    run_id: Uuid,
    message: &str,
    context: Option<Value>,
) -> Result<()> {
    let entry = json!({
        "at": Utc::now().to_rfc3339(),
        "message": message,
        "context": context,
    });
    sqlx::query(
        "UPDATE search_runs
         SET errors = errors || jsonb_build_array($2::jsonb)
         WHERE id = $1 AND jsonb_array_length(errors) < 200",
    )
    .bind(run_id)
    .bind(entry)
    .execute(pool)
    .await
    .context("appending run error")?;
    Ok(())
}

/// Load a run with its JSON logs for the API.
pub async fn get_run(pool: &PgPool, run_id: Uuid) -> Result<Option<crate::models::SearchRun>> {
    let run = sqlx::query_as::<_, crate::models::SearchRun>(
        "SELECT id, trigger, status, phase, progress, started_at, finished_at,
                queries_count, found_count, new_count, errors, logs
         FROM search_runs WHERE id = $1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await?;
    Ok(run)
}

/// Convenience helper used by the API for recent run history.
pub async fn list_runs(pool: &PgPool, limit: i64) -> Result<Vec<crate::models::SearchRun>> {
    let runs = sqlx::query_as::<_, crate::models::SearchRun>(
        "SELECT id, trigger, status, phase, progress, started_at, finished_at,
                queries_count, found_count, new_count, errors, logs
         FROM search_runs ORDER BY started_at DESC LIMIT $1",
    )
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?;
    Ok(runs)
}

/// Exposed for the integration test and operational tooling: the number of
/// rows in a table, used to assert an empty database before a test run.
pub async fn table_count(pool: &PgPool, table: &str) -> Result<i64> {
    if !matches!(
        table,
        "hackathons" | "search_runs" | "sources" | "email_log"
    ) {
        bail!("unknown table {table}");
    }
    let row = sqlx::query(&format!("SELECT count(*) FROM {table}"))
        .fetch_one(pool)
        .await?;
    Ok(row.get::<i64, _>(0))
}
