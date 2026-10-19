use crate::config::Config;
use crate::agent::{normalize_url, truncate_chars};
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashSet;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCandidate {
    pub url: String,
    pub normalized_url: String,
    pub title: String,
    pub snippet: String,
}

#[derive(Debug, Deserialize)]
struct SearxResponse {
    #[serde(default)]
    results: Vec<SearxResult>,
}

#[derive(Debug, Deserialize)]
struct SearxResult {
    url: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    content: String,
}

pub struct SearxClient {
    http: Client,
    base_url: String,
    max_results_per_query: usize,
}

impl SearxClient {
    pub fn new(http: Client, config: &Config) -> Self {
        Self {
            http,
            base_url: config.searxng_url.clone(),
            max_results_per_query: config.max_results_per_query,
        }
    }

    pub fn with_base_url(http: Client, base_url: impl Into<String>, max_results: usize) -> Self {
        Self {
            http,
            base_url: base_url.into(),
            max_results_per_query: max_results,
        }
    }

    /// Query SearXNG with the JSON output format. A failing engine response is
    /// reported as an error so the pipeline can log it and continue.
    pub async fn search(&self, query: &str) -> Result<Vec<SearchCandidate>> {
        let endpoint = format!("{}/search", self.base_url);
        let response = self
            .http
            .get(&endpoint)
            .query(&[
                ("q", query),
                ("format", "json"),
                ("safesearch", "0"),
                ("categories", "general"),
            ])
            .header("Accept", "application/json")
            .timeout(Duration::from_secs(45))
            .send()
            .await
            .with_context(|| format!("requesting SearXNG for {query:?}"))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!(
                "SearXNG returned {status} for {query:?}: {}",
                truncate_chars(body.trim(), 300)
            );
        }

        let payload: SearxResponse = response
            .json()
            .await
            .with_context(|| format!("decoding SearXNG JSON for {query:?}"))?;

        tracing::debug!(
            query,
            engine_results = payload.results.len(),
            "searxng query completed"
        );

        Ok(payload
            .results
            .into_iter()
            .filter_map(|result| {
                let normalized = normalize_url(&result.url)?;
                Some(SearchCandidate {
                    url: result.url.trim().to_owned(),
                    normalized_url: normalized,
                    title: clean_snippet(&result.title, 220),
                    snippet: clean_snippet(&result.content, 500),
                })
            })
            .take(self.max_results_per_query)
            .collect())
    }
}

fn clean_snippet(raw: &str, max_chars: usize) -> String {
    truncate_chars(&raw.split_whitespace().collect::<Vec<_>>().join(" "), max_chars)
}

/// Collapse duplicate URLs while preserving discovery order and respecting the
/// global per-run URL budget.
pub fn dedupe_candidates(
    candidates: impl IntoIterator<Item = SearchCandidate>,
    max_urls: usize,
) -> Vec<SearchCandidate> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        if out.len() >= max_urls {
            break;
        }
        if seen.insert(candidate.normalized_url.clone()) {
            out.push(candidate);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(raw_url: &str, title: &str) -> SearchCandidate {
        SearchCandidate {
            url: raw_url.to_owned(),
            normalized_url: normalize_url(raw_url).unwrap(),
            title: title.to_owned(),
            snippet: String::new(),
        }
    }

    #[test]
    fn dedupes_across_queries_and_keeps_first_occurrence() {
        let first = candidate("https://www.example.com/event/?utm_source=a", "First");
        let duplicate = candidate("https://example.com/event", "Second");
        let other = candidate("https://example.com/other", "Other");

        let unique = dedupe_candidates(vec![first, duplicate, other], 10);
        assert_eq!(unique.len(), 2);
        assert_eq!(unique[0].title, "First");
        assert_eq!(unique[1].normalized_url, "https://example.com/other");
    }

    #[test]
    fn enforces_url_budget() {
        let candidates = (0..5)
            .map(|i| candidate(&format!("https://example.com/{i}"), "x"))
            .collect::<Vec<_>>();
        assert_eq!(dedupe_candidates(candidates, 3).len(), 3);
    }

    #[test]
    fn parses_searxng_payload() {
        let payload = r#"{
            "query": "黑客松",
            "results": [
                {"url": "https://example.com/hack", "title": "AI Hack", "content": "Join us", "engine": "google"},
                {"url": "mailto:bad@example.com", "title": "Bad"},
                {"url": "https://example.com/hack?utm_medium=x", "title": "Duplicate"}
            ]
        }"#;
        let parsed: SearxResponse = serde_json::from_str(payload).unwrap();
        let candidates = parsed
            .results
            .into_iter()
            .filter_map(|result| {
                Some(SearchCandidate {
                    normalized_url: normalize_url(&result.url)?,
                    url: result.url,
                    title: result.title,
                    snippet: result.content,
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].url, "https://example.com/hack");
        assert_eq!(candidates[1].url, "https://example.com/hack?utm_medium=x");
    }
}
