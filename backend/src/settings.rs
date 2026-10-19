use crate::config::Config;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use url::Url;

pub const DEFAULT_SETTINGS_FILE: &str = "data/settings.json";
pub const PASSWORD_MASK: &str = "••••";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoredSettings {
    #[serde(default)]
    pub database_url: Option<String>,
    #[serde(default)]
    pub test_database_url: Option<String>,
    #[serde(default)]
    pub searxng_url: Option<String>,
    #[serde(default)]
    pub ninerouter_base_url: Option<String>,
    #[serde(default)]
    pub ninerouter_api_key: Option<String>,
    #[serde(default)]
    pub ninerouter_model: Option<String>,
    #[serde(default)]
    pub app_api_key: Option<String>,
    #[serde(default)]
    pub frontend_origins: Option<Vec<String>>,
    #[serde(default)]
    pub dashboard_url: Option<String>,
    #[serde(default)]
    pub resend_api_key: Option<String>,
    #[serde(default)]
    pub email_from: Option<String>,
    #[serde(default)]
    pub email_to: Option<String>,
    #[serde(default)]
    pub auto_send_email: Option<bool>,
    #[serde(default)]
    pub confidence_threshold: Option<f32>,
    #[serde(default)]
    pub cron_schedule: Option<String>,
    #[serde(default)]
    pub cron_timezone: Option<String>,
}

impl StoredSettings {
    pub fn from_config(config: &Config) -> Self {
        Self {
            database_url: Some(config.database_url.clone()),
            test_database_url: Some(config.test_database_url.clone()),
            searxng_url: Some(config.searxng_url.clone()),
            ninerouter_base_url: Some(config.ninerouter_base_url.clone()),
            ninerouter_api_key: Some(config.ninerouter_api_key.clone()),
            ninerouter_model: Some(config.ninerouter_model.clone()),
            app_api_key: Some(config.app_api_key.clone()),
            frontend_origins: Some(config.frontend_origins.clone()),
            dashboard_url: Some(config.dashboard_url.clone()),
            resend_api_key: Some(config.resend_api_key.clone()),
            email_from: Some(config.email_from.clone()),
            email_to: Some(config.email_to.clone()),
            auto_send_email: Some(config.auto_send_email),
            confidence_threshold: Some(config.confidence_threshold),
            cron_schedule: Some(config.cron_schedule.clone()),
            cron_timezone: Some(config.cron_timezone.clone()),
        }
    }

    pub fn apply(&self, config: &mut Config) {
        if let Some(value) = &self.database_url {
            if !value.trim().is_empty() {
                config.database_url = value.clone();
            }
        }
        if let Some(value) = &self.test_database_url {
            config.test_database_url = value.clone();
        }
        if let Some(value) = &self.searxng_url {
            config.searxng_url = value.trim_end_matches('/').to_owned();
        }
        if let Some(value) = &self.ninerouter_base_url {
            config.ninerouter_base_url = value.trim_end_matches('/').to_owned();
        }
        if let Some(value) = &self.ninerouter_api_key {
            config.ninerouter_api_key = value.clone();
        }
        if let Some(value) = &self.ninerouter_model {
            config.ninerouter_model = value.clone();
        }
        if let Some(value) = &self.app_api_key {
            if !value.trim().is_empty() {
                config.app_api_key = value.clone();
            }
        }
        if let Some(value) = &self.frontend_origins {
            config.frontend_origins = value.clone();
        }
        if let Some(value) = &self.dashboard_url {
            config.dashboard_url = value.clone();
        }
        if let Some(value) = &self.resend_api_key {
            config.resend_api_key = value.clone();
        }
        if let Some(value) = &self.email_from {
            config.email_from = value.clone();
        }
        if let Some(value) = &self.email_to {
            config.email_to = value.clone();
        }
        if let Some(value) = self.auto_send_email {
            config.auto_send_email = value;
        }
        if let Some(value) = self.confidence_threshold {
            config.confidence_threshold = value;
        }
        if let Some(value) = &self.cron_schedule {
            config.cron_schedule = value.clone();
        }
        if let Some(value) = &self.cron_timezone {
            config.cron_timezone = value.clone();
        }
    }
}

pub fn settings_path() -> PathBuf {
    env::var("SETTINGS_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SETTINGS_FILE))
}

/// Load the settings file and overlay it on environment configuration. The
/// file is optional; environment variables remain the source of defaults.
pub fn apply_overrides(config: &mut Config) -> Result<bool> {
    let path = settings_path();
    if !path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("reading settings file {}", path.display()))?;
    let stored: StoredSettings = serde_json::from_str(&raw)
        .with_context(|| format!("parsing settings file {}", path.display()))?;
    stored.apply(config);
    Ok(true)
}

pub fn save(config: &Config) -> Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating settings directory {}", parent.display()))?;
        }
    }
    let stored = StoredSettings::from_config(config);
    let serialized = serde_json::to_vec_pretty(&stored).context("serializing settings")?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, serialized)
        .with_context(|| format!("writing temporary settings file {}", temp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&temp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&temp, &path)
        .with_context(|| format!("replacing settings file {}", path.display()))?;
    Ok(())
}

pub fn mask_database_url(raw: &str) -> String {
    let Ok(mut url) = Url::parse(raw) else {
        return raw.to_owned();
    };
    if url.password().is_some() {
        if url.set_password(Some("__HH_MASK__")).is_ok() {
            return url.to_string().replace("__HH_MASK__", PASSWORD_MASK);
        }
    }
    url.to_string()
}

/// If the UI sent back the masked password, keep the existing password while
/// allowing host, port, username and database name to change.
pub fn merge_database_url(submitted: &str, current: &str) -> Result<String> {
    let submitted = submitted.trim();
    if submitted.is_empty() {
        bail!("database_url cannot be empty");
    }
    if !submitted.contains(PASSWORD_MASK) {
        return Ok(submitted.to_owned());
    }

    let current_url = Url::parse(current).context("current DATABASE_URL is invalid")?;
    let sanitized = submitted.replace(PASSWORD_MASK, "__HH_PASSWORD__");
    let mut new_url = Url::parse(&sanitized).context("DATABASE_URL is invalid")?;
    if let Some(password) = current_url.password() {
        new_url
            .set_password(Some(password))
            .map_err(|_| anyhow::anyhow!("could not preserve the existing database password"))?;
    } else {
        let _ = new_url.set_password(None);
    }
    Ok(new_url.to_string())
}

pub fn validate_database_url(value: &str) -> Result<()> {
    let url = Url::parse(value.trim()).context("DATABASE_URL is not a valid URL")?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        bail!("DATABASE_URL must use the postgres:// or postgresql:// scheme");
    }
    if url.host_str().is_none() {
        bail!("DATABASE_URL must include a host");
    }
    if url.path().trim_matches('/').is_empty() {
        bail!("DATABASE_URL must include a database name");
    }
    Ok(())
}

pub fn validate_http_url(field: &str, value: &str) -> Result<()> {
    let url = Url::parse(value.trim()).with_context(|| format!("{field} is not a valid URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        bail!("{field} must use http:// or https://");
    }
    if url.host_str().is_none() {
        bail!("{field} must include a host");
    }
    Ok(())
}

pub fn normalize_origins(raw: &str) -> Result<Vec<String>> {
    let mut origins = Vec::new();
    for part in raw.split(',') {
        let value = part.trim();
        if value.is_empty() {
            continue;
        }
        let url = Url::parse(value)
            .with_context(|| format!("frontend origin {value:?} is not a valid URL"))?;
        if !matches!(url.scheme(), "http" | "https") {
            bail!("frontend origin {value:?} must use http:// or https://");
        }
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
            bail!("frontend origin {value:?} must not include a path, query or fragment");
        }
        let origin = url.origin().ascii_serialization();
        if !origins.contains(&origin) {
            origins.push(origin);
        }
    }
    if origins.is_empty() {
        bail!("at least one frontend origin is required");
    }
    Ok(origins)
}

pub fn validate_email(value: &str, field: &str, allow_empty: bool) -> Result<()> {
    let value = value.trim();
    if value.is_empty() {
        if allow_empty {
            return Ok(());
        }
        bail!("{field} cannot be empty");
    }
    for address in value.split(',') {
        let address = address.trim();
        if address.is_empty() {
            continue;
        }
        if !looks_like_email(address) {
            bail!("{field} contains an invalid email address: {address}");
        }
    }
    Ok(())
}

/// Accepts both `user@example.com` and `Name <user@example.com>`.
pub fn looks_like_email(value: &str) -> bool {
    let address = if let (Some(start), Some(end)) = (value.find('<'), value.rfind('>')) {
        if end > start {
            &value[start + 1..end]
        } else {
            value
        }
    } else {
        value
    };
    let address = address.trim();
    if address.contains(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = address.rsplit_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !address.contains(' ')
}

pub fn recipients(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub database_url: String,
    pub test_database_url: String,
    pub searxng_url: String,
    pub ninerouter_base_url: String,
    pub ninerouter_model: String,
    pub ninerouter_api_key_set: bool,
    pub app_api_key_set: bool,
    pub resend_api_key_set: bool,
    pub frontend_origins: String,
    pub dashboard_url: String,
    pub email_from: String,
    pub email_to: String,
    pub auto_send_email: bool,
    pub confidence_threshold: f32,
    pub cron_schedule: String,
    pub cron_timezone: String,
    pub email_configured: bool,
    pub llm_configured: bool,
    pub settings_file: String,
    pub restart_required: Vec<String>,
}

impl SettingsView {
    pub fn from_config(config: &Config, restart_required: Vec<String>) -> Self {
        Self {
            database_url: mask_database_url(&config.database_url),
            test_database_url: if config.test_database_url.is_empty() {
                String::new()
            } else {
                mask_database_url(&config.test_database_url)
            },
            searxng_url: config.searxng_url.clone(),
            ninerouter_base_url: config.ninerouter_base_url.clone(),
            ninerouter_model: config.ninerouter_model.clone(),
            ninerouter_api_key_set: !config.ninerouter_api_key.trim().is_empty(),
            app_api_key_set: !config.app_api_key.trim().is_empty(),
            resend_api_key_set: !config.resend_api_key.trim().is_empty(),
            frontend_origins: config.frontend_origins.join(", "),
            dashboard_url: config.dashboard_url.clone(),
            email_from: config.email_from.clone(),
            email_to: config.email_to.clone(),
            auto_send_email: config.auto_send_email,
            confidence_threshold: config.confidence_threshold,
            cron_schedule: config.cron_schedule.clone(),
            cron_timezone: config.cron_timezone.clone(),
            email_configured: config.email_configured(),
            llm_configured: config.llm_configured(),
            settings_file: settings_path().display().to_string(),
            restart_required,
        }
    }
}

/// Validate and build a new runtime configuration without touching the
/// process. Returns the fields that only take effect after a restart.
pub fn apply_update(current: &Config, update: &SettingsUpdate) -> Result<(Config, Vec<String>)> {
    let mut config = current.clone();
    let mut restart_required = Vec::new();

    if let Some(raw) = update.database_url.as_deref() {
        let merged = merge_database_url(raw, &current.database_url)?;
        validate_database_url(&merged)?;
        if merged != current.database_url {
            config.database_url = merged;
            restart_required.push("database_url".to_owned());
        }
    }

    if let Some(raw) = update.test_database_url.as_deref() {
        let raw = raw.trim();
        let merged = if raw.is_empty() {
            String::new()
        } else if raw.contains(PASSWORD_MASK) {
            if current.test_database_url.is_empty() {
                bail!("test_database_url cannot be masked when no test database is configured");
            }
            merge_database_url(raw, &current.test_database_url)?
        } else {
            raw.to_owned()
        };
        if !merged.is_empty() {
            validate_database_url(&merged)?;
        }
        config.test_database_url = merged;
    }

    if let Some(raw) = update.searxng_url.as_deref() {
        if raw.trim().is_empty() {
            bail!("searxng_url cannot be empty");
        }
        validate_http_url("searxng_url", raw)?;
        config.searxng_url = raw.trim().trim_end_matches('/').to_owned();
    }

    if let Some(raw) = update.ninerouter_base_url.as_deref() {
        if raw.trim().is_empty() {
            bail!("ninerouter_base_url cannot be empty");
        }
        validate_http_url("ninerouter_base_url", raw)?;
        config.ninerouter_base_url = raw.trim().trim_end_matches('/').to_owned();
    }

    if let Some(key) = update.ninerouter_api_key.as_deref() {
        if !key.trim().is_empty() {
            config.ninerouter_api_key = key.trim().to_owned();
        }
    }

    if let Some(model) = update.ninerouter_model.as_deref() {
        if model.trim().is_empty() {
            bail!("ninerouter_model cannot be empty");
        }
        config.ninerouter_model = model.trim().to_owned();
    }

    if let Some(key) = update.app_api_key.as_deref() {
        if !key.trim().is_empty() {
            if key.trim().len() < 8 {
                bail!("app_api_key must be at least 8 characters");
            }
            config.app_api_key = key.trim().to_owned();
        }
    }

    if let Some(raw) = update.frontend_origins.as_deref() {
        config.frontend_origins = normalize_origins(raw)?;
    }

    if let Some(raw) = update.dashboard_url.as_deref() {
        if raw.trim().is_empty() {
            bail!("dashboard_url cannot be empty");
        }
        validate_http_url("dashboard_url", raw)?;
        config.dashboard_url = raw.trim().to_owned();
    }

    if let Some(key) = update.resend_api_key.as_deref() {
        if !key.trim().is_empty() {
            config.resend_api_key = key.trim().to_owned();
        }
    }

    if let Some(value) = update.email_from.as_deref() {
        validate_email(value, "email_from", false)?;
        config.email_from = value.trim().to_owned();
    }

    if let Some(value) = update.email_to.as_deref() {
        validate_email(value, "email_to", true)?;
        config.email_to = value.trim().to_owned();
    }

    if let Some(value) = update.auto_send_email {
        config.auto_send_email = value;
    }

    if let Some(value) = update.confidence_threshold {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            bail!("confidence_threshold must be between 0 and 1");
        }
        config.confidence_threshold = value;
    }

    let mut cron_touched = false;
    if let Some(value) = update.cron_schedule.as_deref() {
        if value.trim().is_empty() {
            bail!("cron_schedule cannot be empty");
        }
        config.cron_schedule = value.trim().to_owned();
        cron_touched = true;
    }
    if let Some(value) = update.cron_timezone.as_deref() {
        if value.trim().is_empty() {
            bail!("cron_timezone cannot be empty");
        }
        config.cron_timezone = value.trim().to_owned();
        cron_touched = true;
    }
    if cron_touched {
        crate::scheduler::validate_schedule(&config.cron_schedule, &config.cron_timezone)?;
    }

    Ok((config, restart_required))
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SettingsUpdate {
    pub database_url: Option<String>,
    pub test_database_url: Option<String>,
    pub searxng_url: Option<String>,
    pub ninerouter_base_url: Option<String>,
    pub ninerouter_api_key: Option<String>,
    pub ninerouter_model: Option<String>,
    pub app_api_key: Option<String>,
    pub frontend_origins: Option<String>,
    pub dashboard_url: Option<String>,
    pub resend_api_key: Option<String>,
    pub email_from: Option<String>,
    pub email_to: Option<String>,
    pub auto_send_email: Option<bool>,
    pub confidence_threshold: Option<f32>,
    pub cron_schedule: Option<String>,
    pub cron_timezone: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_password_but_keeps_structure() {
        let masked = mask_database_url(
            "postgres://postgres:secret@db.example.com:5432/hackathon_hunter",
        );
        assert!(masked.contains(PASSWORD_MASK));
        assert!(!masked.contains("secret"));
        assert!(masked.contains("db.example.com"));
        assert!(masked.contains("hackathon_hunter"));
    }

    #[test]
    fn merges_masked_password_with_new_host_and_database() {
        let current = "postgres://user:s3cr3t@old.example.com:5432/old_db";
        let submitted = "postgres://user:••••@new.example.com:5432/new_db";
        let merged = merge_database_url(submitted, current).unwrap();
        assert!(merged.contains("new.example.com"));
        assert!(merged.contains("new_db"));
        assert!(!merged.contains(PASSWORD_MASK));
        let parsed = Url::parse(&merged).unwrap();
        assert_eq!(parsed.password(), Some("s3cr3t"));
    }

    #[test]
    fn keeps_full_password_when_user_types_a_new_one() {
        let current = "postgres://user:old@localhost/db";
        let submitted = "postgres://user:newpass@localhost/db";
        let merged = merge_database_url(submitted, current).unwrap();
        assert_eq!(Url::parse(&merged).unwrap().password(), Some("newpass"));
    }

    #[test]
    fn validates_origins_and_emails() {
        let origins = normalize_origins("http://localhost:3000, https://hunter.example.com/").unwrap();
        assert_eq!(
            origins,
            vec!["http://localhost:3000", "https://hunter.example.com"]
        );
        assert!(normalize_origins("https://example.com/app").is_err());
        assert!(looks_like_email("Hackathon Hunter <digest@example.com>"));
        assert!(looks_like_email("a@b.co"));
        assert!(!looks_like_email("not-an-email"));
        assert!(validate_email("a@b.co, c@d.org", "EMAIL_TO", false).is_ok());
        assert!(validate_email("", "EMAIL_TO", false).is_err());
    }

    #[test]
    fn validates_database_urls() {
        assert!(validate_database_url("postgres://u:p@host:5432/db").is_ok());
        assert!(validate_database_url("mysql://u:p@host/db").is_err());
        assert!(validate_database_url("postgres://host").is_err());
    }
}
