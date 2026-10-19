use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Hackathon {
    pub id: Uuid,
    pub title: String,
    pub title_zh: Option<String>,
    pub organizer: Option<String>,
    pub description: String,
    pub location: Option<String>,
    pub is_online: Option<bool>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub registration_deadline: Option<NaiveDate>,
    pub registration_url: Option<String>,
    pub registration_url_normalized: Option<String>,
    pub source_url: String,
    pub source_url_normalized: String,
    pub prize_summary: Option<String>,
    pub prize_amount_cny: Option<f64>,
    pub tracks_or_themes: Vec<String>,
    pub eligibility: Option<String>,
    pub tags: Vec<String>,
    pub confidence: f32,
    pub status: String,
    pub dedupe_key: String,
    pub content_hash: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub content_changed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SearchRun {
    pub id: Uuid,
    pub trigger: String,
    pub status: String,
    pub phase: String,
    pub progress: i32,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub queries_count: i32,
    pub found_count: i32,
    pub new_count: i32,
    pub errors: serde_json::Value,
    pub logs: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Source {
    pub id: i64,
    pub run_id: Option<Uuid>,
    pub url: String,
    pub normalized_url: String,
    pub domain: String,
    pub status: String,
    pub http_status: Option<i32>,
    pub title: Option<String>,
    pub content_hash: Option<String>,
    pub error: Option<String>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EmailLog {
    pub id: Uuid,
    pub sent_at: DateTime<Utc>,
    pub hackathon_ids: Vec<Uuid>,
    pub resend_id: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub recipient: Option<String>,
    pub subject: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HackathonPage {
    pub items: Vec<Hackathon>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunAccepted {
    pub run_id: Uuid,
    pub status: &'static str,
}
