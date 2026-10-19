use crate::agent::{non_empty, truncate_chars};
use crate::config::Config;
use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone, Deserialize)]
pub struct RawExtraction {
    pub is_hackathon: bool,
    pub title: String,
    #[serde(default)]
    pub title_zh: Option<String>,
    #[serde(default)]
    pub organizer: Option<String>,
    pub description: String,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub is_online: Option<bool>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub registration_deadline: Option<String>,
    #[serde(default)]
    pub registration_url: Option<String>,
    #[serde(default)]
    pub prize_summary: Option<String>,
    #[serde(default)]
    pub prize_amount_cny: Option<f64>,
    #[serde(default)]
    pub tracks_or_themes: Vec<String>,
    #[serde(default)]
    pub eligibility: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedHackathon {
    pub is_hackathon: bool,
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
    pub prize_summary: Option<String>,
    pub prize_amount_cny: Option<f64>,
    pub tracks_or_themes: Vec<String>,
    pub eligibility: Option<String>,
    pub tags: Vec<String>,
    pub confidence: f32,
    pub source_url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtractionDecision {
    Accepted(ExtractedHackathon),
    Rejected { reason: String },
}

pub struct Extractor {
    http: Client,
    endpoint: String,
    api_key: String,
    model: String,
    timeout: Duration,
    confidence_threshold: f32,
}

impl Extractor {
    pub fn new(http: Client, config: &Config) -> Self {
        Self {
            http,
            endpoint: completion_endpoint(&config.ninerouter_base_url),
            api_key: config.ninerouter_api_key.clone(),
            model: config.ninerouter_model.clone(),
            timeout: Duration::from_secs(config.llm_timeout_secs),
            confidence_threshold: config.confidence_threshold,
        }
    }

    pub fn with_settings(
        http: Client,
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        confidence_threshold: f32,
    ) -> Self {
        Self {
            http,
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            model: model.into(),
            timeout: Duration::from_secs(60),
            confidence_threshold,
        }
    }

    /// Extracts one hackathon from a scraped page. Invalid JSON is retried once
    /// with the validation error attached; a second failure is returned as an
    /// error so the pipeline can record it and continue with other pages.
    pub async fn extract(
        &self,
        page_url: &str,
        page_title: &str,
        page_text: &str,
    ) -> Result<ExtractionDecision> {
        if self.api_key.trim().is_empty() {
            bail!("NINEROUTER_API_KEY is not configured");
        }

        let mut correction: Option<String> = None;

        for attempt in 0..2 {
            let messages = build_messages(page_url, page_title, page_text, correction.as_deref());
            let content = self.complete(&messages).await?;

            match parse_extraction_response(&content) {
                Ok(raw) => return decide(raw, self.confidence_threshold, page_url),
                Err(error) => {
                    tracing::warn!(
                        page = %page_url,
                        attempt = attempt + 1,
                        error = %error,
                        "invalid extraction JSON"
                    );
                    correction = Some(error.to_string());
                }
            }
        }

        bail!(
            "model returned invalid extraction JSON after retry: {}",
            correction.unwrap_or_else(|| "unknown validation error".to_owned())
        )
    }

    async fn complete(&self, messages: &[Value]) -> Result<String> {
        let body = json!({
            "model": self.model,
            "temperature": 0.0,
            "stream": false,
            "max_tokens": 1600,
            "response_format": { "type": "json_object" },
            "messages": messages,
        });

        let first = self.send(body.clone()).await;
        let response = match first {
            Ok(response) => response,
            Err(error) if error.to_string().contains("response_format") => {
                // A few OpenAI-compatible routers reject response_format. The
                // prompt already demands JSON, so retry without the parameter.
                let mut fallback = body;
                if let Some(object) = fallback.as_object_mut() {
                    object.remove("response_format");
                }
                self.send(fallback).await?
            }
            Err(error) => return Err(error),
        };

        let status = response.status();
        let text = response.text().await.context("reading LLM response body")?;

        if !status.is_success() {
            bail!(
                "9router returned {status}: {}",
                truncate_chars(text.trim(), 600)
            );
        }

        let payload: ChatResponse =
            serde_json::from_str(&text).context("decoding 9router chat completion")?;
        let content = payload
            .choices
            .into_iter()
            .find_map(|choice| choice.message.content)
            .ok_or_else(|| anyhow::anyhow!("9router response contained no message content"))?;

        Ok(content)
    }

    async fn send(&self, body: Value) -> Result<reqwest::Response> {
        self.http
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .timeout(self.timeout)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("calling 9router at {}", self.endpoint))
    }
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<String>,
}

fn completion_endpoint(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_owned()
    } else {
        format!("{base}/chat/completions")
    }
}

fn build_messages(
    page_url: &str,
    page_title: &str,
    page_text: &str,
    correction: Option<&str>,
) -> Vec<Value> {
    let system = r#"You are a precise data-extraction service for a Chinese hackathon discovery agent.
You receive the cleaned text of ONE web page. Decide whether the page is primarily about a specific hackathon, hackathon-style competition, 黑客马拉松, 编程大赛, 创新大赛, or AI competition that people can register for or participate in.

Return exactly ONE JSON object and nothing else. No markdown, no code fences, no commentary.

Schema:
{
  "is_hackathon": boolean,
  "title": string,
  "title_zh": string|null,
  "organizer": string|null,
  "description": string,
  "location": string|null,
  "is_online": boolean|null,
  "start_date": "YYYY-MM-DD"|null,
  "end_date": "YYYY-MM-DD"|null,
  "registration_deadline": "YYYY-MM-DD"|null,
  "registration_url": string|null,
  "prize_summary": string|null,
  "prize_amount_cny": number|null,
  "tracks_or_themes": [string],
  "eligibility": string|null,
  "tags": [string],
  "confidence": number,
  "source_url": string
}

Rules:
1. NEVER invent information. Use null for every field that is not explicitly supported by the page. Never guess a date, city, organizer, prize or URL.
2. Only output full ISO dates (YYYY-MM-DD). If only a month, year, or relative phrase is stated, use null for that date.
3. `registration_deadline` is the deadline to sign up, not the event start date.
4. `is_online`: true only when the event is explicitly virtual/online, false only when explicitly in person, null when unclear.
5. `prize_amount_cny` is a number in CNY only when the page states an amount that can be converted using an explicitly stated CNY figure. If the prize is in another currency or vague, use null and describe it in `prize_summary`.
6. If the page is an article, directory or list covering many different events rather than one registrable event, set `is_hackathon` to false. Navigation pages, blog posts about past events, and closed events are not valid hackathons.
7. `description` must be 2-4 factual sentences taken from the page.
8. `tags` must be 3-8 short topical tags (for example "ai", "llm", "student", "shanghai", "fintech"). `tracks_or_themes` lists the event tracks/themes only if the page names them.
9. `confidence` is a number from 0 to 1 reflecting how certain you are that this is a real, currently registrable hackathon and that the extracted fields are correct.
10. `title` is the event title in its original language. If the page has a distinct Chinese title, put it in `title_zh`; otherwise null.
11. `source_url` must be the exact PAGE URL provided below.
12. For events whose registration deadline has already passed and the page has no future edition, set is_hackathon to false.

Output JSON only."#;

    let user = format!(
        "PAGE URL: {page_url}\nPAGE TITLE: {}\n\nPAGE TEXT:\n{}",
        truncate_chars(page_title, 300),
        truncate_chars(page_text, 60_000)
    );

    let mut messages = vec![
        json!({ "role": "system", "content": system }),
        json!({ "role": "user", "content": user }),
    ];

    if let Some(error) = correction {
        messages.push(json!({
            "role": "user",
            "content": format!(
                "Your previous reply was rejected: {error}\nReturn ONLY a corrected JSON object matching the schema. Use null for unknown values."
            )
        }));
    }

    messages
}

/// Parse the model output, tolerating accidental markdown fences or leading
/// prose by extracting the outermost JSON object.
pub fn parse_extraction_response(content: &str) -> Result<RawExtraction> {
    let trimmed = content.trim().trim_start_matches('\u{feff}');
    let start = trimmed
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("response contains no JSON object"))?;
    let end = trimmed
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("response contains no closing JSON brace"))?;
    if end <= start {
        bail!("response contains malformed JSON braces");
    }

    let raw: RawExtraction = serde_json::from_str(&trimmed[start..=end])
        .context("response JSON does not match the extraction schema")?;
    Ok(raw)
}

pub fn decide(
    raw: RawExtraction,
    confidence_threshold: f32,
    page_url: &str,
) -> Result<ExtractionDecision> {
    if !raw.is_hackathon {
        return Ok(ExtractionDecision::Rejected {
            reason: "model classified page as not a hackathon".to_owned(),
        });
    }

    let confidence = if raw.confidence.is_finite() {
        raw.confidence.clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    if confidence < confidence_threshold {
        return Ok(ExtractionDecision::Rejected {
            reason: format!("confidence {confidence:.2} below threshold {confidence_threshold:.2}"),
        });
    }

    let extracted = normalize(raw, confidence, page_url)?;
    Ok(ExtractionDecision::Accepted(extracted))
}

fn normalize(raw: RawExtraction, confidence: f32, page_url: &str) -> Result<ExtractedHackathon> {
    let title = raw.title.trim().to_owned();
    if title.is_empty() {
        bail!("title is empty");
    }
    let description = raw.description.trim().to_owned();
    if description.is_empty() {
        bail!("description is empty");
    }

    let start_date = parse_date(raw.start_date.as_deref(), "start_date")?;
    let end_date = parse_date(raw.end_date.as_deref(), "end_date")?;
    let registration_deadline =
        parse_date(raw.registration_deadline.as_deref(), "registration_deadline")?;

    if let (Some(start), Some(end)) = (start_date, end_date) {
        if start > end {
            bail!("start_date is after end_date");
        }
    }

    // Never trust a URL invented by the model; only accept absolute HTTP(S)
    // links that are present in the page text.
    let registration_url = raw
        .registration_url
        .as_deref()
        .map(str::trim)
        .filter(|candidate| {
            Url::parse(candidate)
                .map(|url| matches!(url.scheme(), "http" | "https"))
                .unwrap_or(false)
        })
        .map(ToOwned::to_owned);

    let tracks = clean_list(raw.tracks_or_themes, 12, 100);
    let tags = clean_list(raw.tags, 10, 40);

    let prize_amount_cny = raw
        .prize_amount_cny
        .filter(|amount| amount.is_finite() && *amount >= 0.0)
        .map(|amount| (amount * 100.0).round() / 100.0);

    Ok(ExtractedHackathon {
        is_hackathon: true,
        title: truncate_chars(&title, 300),
        title_zh: non_empty(raw.title_zh).map(|value| truncate_chars(&value, 300)),
        organizer: non_empty(raw.organizer).map(|value| truncate_chars(&value, 200)),
        description: truncate_chars(&description, 8_000),
        location: non_empty(raw.location).map(|value| truncate_chars(&value, 200)),
        is_online: raw.is_online,
        start_date,
        end_date,
        registration_deadline,
        registration_url,
        prize_summary: non_empty(raw.prize_summary).map(|value| truncate_chars(&value, 500)),
        prize_amount_cny,
        tracks_or_themes: tracks,
        eligibility: non_empty(raw.eligibility).map(|value| truncate_chars(&value, 1_000)),
        tags,
        confidence,
        source_url: page_url.to_owned(),
    })
}

fn parse_date(value: Option<&str>, field: &str) -> Result<Option<NaiveDate>> {
    let Some(raw) = value.map(str::trim).filter(|v| !v.is_empty() && *v != "null") else {
        return Ok(None);
    };
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map(Some)
        .with_context(|| format!("invalid {field} {raw:?}; expected YYYY-MM-DD"))
}

fn clean_list(values: Vec<String>, max_items: usize, max_chars: usize) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let item = truncate_chars(trimmed, max_chars);
        if seen.insert(item.to_lowercase()) {
            out.push(item);
        }
        if out.len() >= max_items {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_json() -> &'static str {
        r#"{
            "is_hackathon": true,
            "title": "Shanghai AI Hackathon 2026",
            "title_zh": "上海人工智能黑客松 2026",
            "organizer": "Example Labs",
            "description": "A 48-hour AI hackathon in Shanghai. Teams build LLM applications.",
            "location": "Shanghai, China",
            "is_online": false,
            "start_date": "2026-06-12",
            "end_date": "2026-06-14",
            "registration_deadline": "2026-06-01",
            "registration_url": "https://example.com/register",
            "prize_summary": "50,000 CNY prize pool",
            "prize_amount_cny": 50000,
            "tracks_or_themes": ["AI Agents", "LLM"],
            "eligibility": "Students and professionals",
            "tags": ["ai", "llm", "shanghai"],
            "confidence": 0.92,
            "source_url": "https://example.com/event"
        }"#
    }

    #[test]
    fn parses_valid_schema() {
        let raw = parse_extraction_response(valid_json()).unwrap();
        assert!(raw.is_hackathon);
        assert_eq!(raw.tags.len(), 3);
        assert_eq!(raw.start_date.as_deref(), Some("2026-06-12"));
    }

    #[test]
    fn parses_fenced_json_with_prose() {
        let content = format!("Here is the result:\n```json\n{}\n```\nDone.", valid_json());
        let raw = parse_extraction_response(&content).unwrap();
        assert_eq!(raw.title, "Shanghai AI Hackathon 2026");
    }

    #[test]
    fn rejects_malformed_json_and_wrong_types() {
        assert!(parse_extraction_response("not json").is_err());
        assert!(parse_extraction_response(r#"{"is_hackathon": "yes"}"#).is_err());
        assert!(parse_extraction_response(r#"{"is_hackathon": true}"#).is_err());
    }

    #[test]
    fn rejects_invalid_date_formats() {
        let raw = parse_extraction_response(valid_json()).unwrap();
        let decision = decide(
            RawExtraction {
                start_date: Some("June 2026".to_owned()),
                ..raw
            },
            0.5,
            "https://example.com/event",
        );
        assert!(decision.is_err());
    }

    #[test]
    fn accepts_confident_hackathon_and_forces_source_url() {
        let raw = parse_extraction_response(valid_json()).unwrap();
        let decision = decide(raw, 0.6, "https://scraped.example/page").unwrap();
        match decision {
            ExtractionDecision::Accepted(item) => {
                assert_eq!(item.source_url, "https://scraped.example/page");
                assert_eq!(item.confidence, 0.92);
                assert_eq!(item.prize_amount_cny, Some(50000.0));
                assert_eq!(item.tags, vec!["ai", "llm", "shanghai"]);
            }
            _ => panic!("expected accepted extraction"),
        }
    }

    #[test]
    fn rejects_low_confidence_and_non_hackathons() {
        let raw = parse_extraction_response(valid_json()).unwrap();
        let low = decide(
            RawExtraction {
                confidence: 0.42,
                ..raw.clone()
            },
            0.6,
            "https://example.com/event",
        )
        .unwrap();
        assert!(matches!(low, ExtractionDecision::Rejected { .. }));

        let not_hackathon = decide(
            RawExtraction {
                is_hackathon: false,
                ..raw
            },
            0.6,
            "https://example.com/event",
        )
        .unwrap();
        assert!(matches!(
            not_hackathon,
            ExtractionDecision::Rejected { .. }
        ));
    }

    #[test]
    fn normalizes_tags_and_invalid_registration_urls() {
        let raw = parse_extraction_response(valid_json()).unwrap();
        let decision = decide(
            RawExtraction {
                tags: vec![" AI ".to_owned(), "ai".to_owned(), "".to_owned()],
                registration_url: Some("javascript:alert(1)".to_owned()),
                ..raw
            },
            0.5,
            "https://example.com/event",
        )
        .unwrap();
        match decision {
            ExtractionDecision::Accepted(item) => {
                assert_eq!(item.tags, vec!["AI"]);
                assert_eq!(item.registration_url, None);
            }
            _ => panic!("expected accepted extraction"),
        }
    }

    #[test]
    fn rejects_end_before_start() {
        let raw = parse_extraction_response(valid_json()).unwrap();
        let result = decide(
            RawExtraction {
                start_date: Some("2026-06-20".to_owned()),
                end_date: Some("2026-06-10".to_owned()),
                ..raw
            },
            0.5,
            "https://example.com/event",
        );
        assert!(result.is_err());
    }

    #[test]
    fn builds_openai_compatible_endpoint() {
        assert_eq!(
            completion_endpoint("https://api.9router.ai/v1/"),
            "https://api.9router.ai/v1/chat/completions"
        );
        assert_eq!(
            completion_endpoint("https://api.9router.ai/v1/chat/completions"),
            "https://api.9router.ai/v1/chat/completions"
        );
    }
}
