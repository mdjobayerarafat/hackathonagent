pub mod extract;
pub mod pipeline;
pub mod scrape;
pub mod search;

use sha2::{Digest, Sha256};
use url::Url;

const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "utm_name",
    "fbclid",
    "gclid",
    "yclid",
    "mc_cid",
    "mc_eid",
    "ref",
    "referrer",
    "spm",
    "from",
];

/// Canonical URL used for de-duplication. The original URL is still the one
/// fetched and shown to the user.
pub fn normalize_url(raw: &str) -> Option<String> {
    let parsed = Url::parse(raw.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }

    let mut url = parsed;
    url.set_fragment(None);

    let host = url.host_str()?.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
    url.set_host(Some(&host)).ok()?;
    if (url.scheme() == "http" && url.port() == Some(80))
        || (url.scheme() == "https" && url.port() == Some(443))
    {
        url.set_port(None).ok()?;
    }

    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !TRACKING_PARAMS.contains(&key.to_ascii_lowercase().as_str()))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    pairs.sort();
    url.set_query(None);
    if !pairs.is_empty() {
        let mut query = url.query_pairs_mut();
        for (key, value) in pairs {
            query.append_pair(&key, &value);
        }
    }

    let path = url.path().to_owned();
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        url.set_path("/");
    } else {
        url.set_path(trimmed);
    }

    Some(url.to_string())
}

pub fn is_fetchable_url(raw: &str) -> bool {
    normalize_url(raw).is_some()
}

/// Strip punctuation, case and whitespace so "AI 创客松 2026!" and
/// "ai创客松 2026" collapse to the same event key.
pub fn normalize_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn dedupe_key(registration_url: Option<&str>, source_url: &str, title: &str) -> String {
    if let Some(url) = registration_url.and_then(normalize_url) {
        return format!("reg:{url}");
    }
    let source = normalize_url(source_url).unwrap_or_else(|| source_url.trim().to_owned());
    format!("src:{source}|title:{}", normalize_title(title))
}

pub fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

/// Truncate by Unicode scalar values, never in the middle of a character.
pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(max_chars.min(text.len()));
    for (index, ch) in text.chars().enumerate() {
        if index >= max_chars {
            break;
        }
        out.push(ch);
    }
    out
}

pub fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_tracking_fragments_and_trailing_slashes() {
        assert_eq!(
            normalize_url(
                "https://www.Example.com/event/42/?utm_source=x&b=2&a=1#register"
            )
            .unwrap(),
            "https://example.com/event/42?a=1&b=2"
        );
    }

    #[test]
    fn rejects_non_http_urls() {
        assert_eq!(normalize_url("mailto:hi@example.com"), None);
        assert_eq!(normalize_url("javascript:alert(1)"), None);
        assert_eq!(normalize_url("not a url"), None);
    }

    #[test]
    fn dedupe_prefers_registration_url() {
        let a = dedupe_key(
            Some("https://example.com/register/?utm_medium=email"),
            "https://news.example.com/a",
            "AI Hackathon 2026",
        );
        let b = dedupe_key(
            Some("https://www.example.com/register"),
            "https://other.example.com/b",
            "Different title",
        );
        assert_eq!(a, b);
        assert!(a.starts_with("reg:"));
    }

    #[test]
    fn dedupe_falls_back_to_source_and_title() {
        let a = dedupe_key(None, "https://Example.com/a/", "AI 黑客松 2026!");
        let b = dedupe_key(None, "https://example.com/a", "ai 黑客松 2026");
        assert_eq!(a, b);
        assert_eq!(normalize_title("  AI  创客松 2026!! "), "ai创客松2026");
    }

    #[test]
    fn truncates_on_character_boundaries() {
        assert_eq!(truncate_chars("你好世界", 2), "你好");
        assert_eq!(truncate_chars("abc", 10), "abc");
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(content_hash("hello"), content_hash("hello"));
        assert_ne!(content_hash("hello"), content_hash("hello "));
        assert_eq!(content_hash("hello").len(), 64);
    }
}
