use anyhow::{bail, Context, Result};
use std::env;

/// Hard-coded discovery queries. Users can extend or replace them with the
/// `SEARCH_QUERIES` environment variable (semicolon separated).
pub const DEFAULT_SEARCH_QUERIES: &[&str] = &[
    "黑客松 2026 报名",
    "hackathon China 2026 registration",
    "编程大赛 奖金 报名 2026",
    "AI 创新大赛 报名 2026",
    "黑客马拉松 2026 高校 企业 报名",
    "site:devpost.com China hackathon 2026",
    "site:segmentfault.com 黑客松 2026",
    "site:modelscope.cn 大赛 2026",
    "site:tianchi.aliyun.com 大赛 2026",
    "site:kaggle.com hackathon China 2026",
    "hackathon Shanghai Beijing Shenzhen 2026 registration",
    "大学生 编程 竞赛 2026 报名 奖金",
];

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub test_database_url: String,
    pub db_max_connections: u32,
    pub app_api_key: String,
    pub frontend_origins: Vec<String>,

    pub searxng_url: String,
    pub searxng_timeout_secs: u64,
    pub search_queries: Vec<String>,
    pub max_results_per_query: usize,
    pub max_urls_per_run: usize,

    pub ninerouter_base_url: String,
    pub ninerouter_api_key: String,
    pub ninerouter_model: String,
    pub llm_timeout_secs: u64,
    pub confidence_threshold: f32,
    pub max_text_chars: usize,

    pub scrape_timeout_secs: u64,
    pub scrape_concurrency: usize,
    pub extract_concurrency: usize,
    pub domain_rate_limit_ms: u64,
    pub source_refresh_hours: i64,
    pub user_agent: String,

    pub resend_api_key: String,
    pub email_from: String,
    pub email_to: String,
    pub auto_send_email: bool,
    pub dashboard_url: String,

    pub cron_schedule: String,
    pub cron_timezone: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let database_url = env_required("DATABASE_URL")?;
        let app_api_key = env_or("APP_API_KEY", "change-me");
        if app_api_key == "change-me" {
            tracing::warn!(
                "APP_API_KEY is the default value; set a strong key before exposing this service"
            );
        }

        let search_queries: Vec<String> = match env::var("SEARCH_QUERIES") {
            Ok(raw) if !raw.trim().is_empty() => raw
                .split(';')
                .map(str::trim)
                .filter(|q| !q.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            _ => DEFAULT_SEARCH_QUERIES
                .iter()
                .map(|q| (*q).to_owned())
                .collect(),
        };

        if search_queries.is_empty() {
            bail!("SEARCH_QUERIES must contain at least one query");
        }

        Ok(Self {
            host: env_or("APP_HOST", "0.0.0.0"),
            port: env_parse("APP_PORT", 8080)?,
            database_url,
            test_database_url: env_or("TEST_DATABASE_URL", ""),
            db_max_connections: env_parse("DB_MAX_CONNECTIONS", 10)?,
            app_api_key,
            frontend_origins: env_or("FRONTEND_ORIGINS", "http://localhost:3000")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect(),

            searxng_url: env_or("SEARXNG_URL", "http://localhost:8081")
                .trim_end_matches('/')
                .to_owned(),
            searxng_timeout_secs: env_parse("SEARXNG_TIMEOUT_SECS", 45)?,
            search_queries,
            max_results_per_query: env_parse("MAX_RESULTS_PER_QUERY", 8)?,
            max_urls_per_run: env_parse("MAX_URLS_PER_RUN", 30)?,

            ninerouter_base_url: env_or("NINEROUTER_BASE_URL", "https://api.9router.ai/v1")
                .trim_end_matches('/')
                .to_owned(),
            ninerouter_api_key: env_or("NINEROUTER_API_KEY", ""),
            ninerouter_model: env_or("NINEROUTER_MODEL", "gpt-4o-mini"),
            llm_timeout_secs: env_parse("LLM_TIMEOUT_SECS", 90)?,
            confidence_threshold: env_parse("CONFIDENCE_THRESHOLD", 0.6)?,
            max_text_chars: env_parse("MAX_TEXT_CHARS", 16_000)?,

            scrape_timeout_secs: env_parse("SCRAPE_TIMEOUT_SECS", 20)?,
            scrape_concurrency: env_parse("SCRAPE_CONCURRENCY", 4)?,
            extract_concurrency: env_parse("EXTRACT_CONCURRENCY", 2)?,
            domain_rate_limit_ms: env_parse("DOMAIN_RATE_LIMIT_MS", 1_500)?,
            source_refresh_hours: env_parse("SOURCE_REFRESH_HOURS", 72)?,
            user_agent: env_or(
                "USER_AGENT",
                "Mozilla/5.0 (compatible; HackathonHunter/1.0; +https://github.com/hackathon-hunter)",
            ),

            resend_api_key: env_or("RESEND_API_KEY", ""),
            email_from: env_or("EMAIL_FROM", "Hackathon Hunter <digest@example.com>"),
            email_to: env_or("EMAIL_TO", ""),
            auto_send_email: env_parse("AUTO_SEND_EMAIL", true)?,
            dashboard_url: env_or("DASHBOARD_URL", "http://localhost:3000"),

            cron_schedule: env_or("CRON_SCHEDULE", "0 0 8 * * *"),
            cron_timezone: env_or("CRON_TIMEZONE", "Asia/Shanghai"),
        })
    }

    /// True when the digest email can actually be sent.
    pub fn email_configured(&self) -> bool {
        !self.resend_api_key.trim().is_empty() && !self.email_to.trim().is_empty()
    }

    pub fn llm_configured(&self) -> bool {
        !self.ninerouter_api_key.trim().is_empty()
    }
}

fn env_required(key: &str) -> Result<String> {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        _ => bail!("{key} must be set (see .env.example)"),
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn env_parse<T>(key: &str, default: T) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(raw) if !raw.trim().is_empty() => raw
            .trim()
            .parse::<T>()
            .map_err(|e| anyhow::anyhow!("invalid {key}: {e}")),
        _ => Ok(default),
    }
    .with_context(|| format!("reading environment variable {key}"))
}
