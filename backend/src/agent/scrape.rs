use crate::agent::{content_hash, truncate_chars};
use crate::config::Config;
use ego_tree::NodeRef;
use reqwest::Client;
use scraper::{Html, Node, Selector};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Mutex;
use url::Url;

const MAX_HTML_BYTES: usize = 3 * 1024 * 1024;
const MAX_ROBOTS_BYTES: usize = 512 * 1024;
const MAX_ATTEMPTS: usize = 3;

const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "nav", "header", "footer", "svg", "iframe", "form", "button",
    "aside", "canvas", "template", "video", "audio", "select", "input",
];

const BLOCK_TAGS: &[&str] = &[
    "p", "div", "section", "article", "main", "li", "tr", "h1", "h2", "h3", "h4", "h5", "h6",
    "br", "ul", "ol", "table", "blockquote", "pre", "dd", "dt",
];

#[derive(Debug, Error)]
pub enum ScrapeError {
    #[error("robots.txt disallows this URL")]
    RobotsDisallowed,
    #[error("HTTP {0}")]
    HttpStatus(u16),
    #[error("unsupported content type: {0}")]
    UnsupportedContent(String),
    #[error("page exceeds the 3 MiB limit")]
    TooLarge,
    #[error("page has too little readable text")]
    TooLittleText,
    #[error("request failed: {0}")]
    Request(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
}

#[derive(Debug, Clone)]
pub struct ScrapedPage {
    pub url: String,
    pub title: String,
    pub text: String,
    pub content_hash: String,
    pub http_status: u16,
}

pub struct Scraper {
    http: Client,
    timeout: Duration,
    rate_limit: Duration,
    max_text_chars: usize,
    user_agent: String,
    host_locks: Mutex<HashMap<String, Arc<Mutex<Instant>>>>,
    robots_cache: Mutex<HashMap<String, Arc<RobotsRules>>>,
}

impl Scraper {
    pub fn new(http: Client, config: &Config) -> Self {
        Self {
            http,
            timeout: Duration::from_secs(config.scrape_timeout_secs),
            rate_limit: Duration::from_millis(config.domain_rate_limit_ms),
            max_text_chars: config.max_text_chars,
            user_agent: config.user_agent.clone(),
            host_locks: Mutex::new(HashMap::new()),
            robots_cache: Mutex::new(HashMap::new()),
        }
    }

    pub async fn fetch(&self, raw_url: &str) -> Result<ScrapedPage, ScrapeError> {
        let parsed = Url::parse(raw_url.trim())
            .map_err(|_| ScrapeError::InvalidUrl(raw_url.to_owned()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(ScrapeError::InvalidUrl(raw_url.to_owned()));
        }
        let domain = parsed
            .host_str()
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| ScrapeError::InvalidUrl(raw_url.to_owned()))?;

        let rules = self.robots_for(&parsed, &domain).await;
        if !rules.allows(&path_with_query(&parsed), &self.user_agent) {
            return Err(ScrapeError::RobotsDisallowed);
        }

        let mut last_error: Option<ScrapeError> = None;

        for attempt in 0..MAX_ATTEMPTS {
            self.throttle(&domain, rules.crawl_delay()).await;

            let response = match self
                .http
                .get(raw_url)
                .timeout(self.timeout)
                .send()
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(ScrapeError::Request(error.to_string()));
                    if attempt + 1 < MAX_ATTEMPTS {
                        sleep_backoff(attempt).await;
                        continue;
                    }
                    break;
                }
            };

            let status = response.status();
            if status.is_server_error() || status.as_u16() == 429 {
                last_error = Some(ScrapeError::HttpStatus(status.as_u16()));
                if attempt + 1 < MAX_ATTEMPTS {
                    sleep_backoff(attempt).await;
                    continue;
                }
                break;
            }
            if !status.is_success() {
                return Err(ScrapeError::HttpStatus(status.as_u16()));
            }

            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !content_type.is_empty()
                && !content_type.contains("text/html")
                && !content_type.contains("application/xhtml")
                && !content_type.contains("text/plain")
            {
                return Err(ScrapeError::UnsupportedContent(content_type));
            }

            if response
                .content_length()
                .is_some_and(|size| size as usize > MAX_HTML_BYTES)
            {
                return Err(ScrapeError::TooLarge);
            }

            let final_url = response.url().to_string();
            let body = response
                .text()
                .await
                .map_err(|error| ScrapeError::Request(error.to_string()))?;
            if body.len() > MAX_HTML_BYTES {
                return Err(ScrapeError::TooLarge);
            }

            return clean_html(&body, &final_url, self.max_text_chars);
        }

        Err(last_error.unwrap_or_else(|| ScrapeError::Request("unknown error".to_owned())))
    }

    async fn robots_for(&self, parsed: &Url, domain: &str) -> Arc<RobotsRules> {
        if let Some(cached) = self.robots_cache.lock().await.get(domain).cloned() {
            return cached;
        }

        let origin = parsed.origin().ascii_serialization();
        let robots_url = format!("{origin}/robots.txt");
        self.throttle(domain, None).await;

        let rules = match self
            .http
            .get(&robots_url)
            .timeout(Duration::from_secs(8))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let body = response.text().await.unwrap_or_default();
                if body.len() > MAX_ROBOTS_BYTES {
                    RobotsRules::allow_all()
                } else {
                    RobotsRules::parse(&body)
                }
            }
            Ok(response) => {
                tracing::debug!(
                    domain,
                    status = %response.status(),
                    "robots.txt unavailable, allowing crawl"
                );
                RobotsRules::allow_all()
            }
            Err(error) => {
                tracing::debug!(domain, error = %error, "robots.txt fetch failed, allowing crawl");
                RobotsRules::allow_all()
            }
        };

        let rules = Arc::new(rules);
        self.robots_cache
            .lock()
            .await
            .insert(domain.to_owned(), rules.clone());
        rules
    }

    /// Serialises requests per host and enforces both the configured delay and
    /// any robots.txt crawl-delay. Requests to different hosts stay parallel.
    async fn throttle(&self, domain: &str, crawl_delay: Option<Duration>) {
        let lock = {
            let mut hosts = self.host_locks.lock().await;
            hosts
                .entry(domain.to_owned())
                .or_insert_with(|| {
                    Arc::new(Mutex::new(
                        Instant::now() - Duration::from_secs(3600),
                    ))
                })
                .clone()
        };

        let delay = self.rate_limit.max(crawl_delay.unwrap_or_default());
        let mut last_request = lock.lock().await;
        let elapsed = last_request.elapsed();
        if elapsed < delay {
            tokio::time::sleep(delay - elapsed).await;
        }
        *last_request = Instant::now();
    }
}

async fn sleep_backoff(attempt: usize) {
    let base = 400u64.saturating_mul(1 << attempt.min(5));
    tokio::time::sleep(Duration::from_millis(base)).await;
}

fn path_with_query(url: &Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{}", url.path(), query),
        None => url.path().to_owned(),
    }
}

pub fn clean_html(
    html: &str,
    page_url: &str,
    max_text_chars: usize,
) -> Result<ScrapedPage, ScrapeError> {
    let document = Html::parse_document(html);

    let title_selector = Selector::parse("title").expect("static selector");
    let og_title_selector = Selector::parse(r#"meta[property="og:title"]"#).expect("static selector");
    let description_selector =
        Selector::parse(r#"meta[name="description"]"#).expect("static selector");

    let title = document
        .select(&title_selector)
        .next()
        .map(|element| element.text().collect::<String>())
        .or_else(|| {
            document
                .select(&og_title_selector)
                .next()
                .and_then(|element| element.value().attr("content").map(ToOwned::to_owned))
        })
        .map(|value| normalize_text(&value).replace('\n', " "))
        .unwrap_or_default();

    let description = document
        .select(&description_selector)
        .next()
        .and_then(|element| element.value().attr("content"))
        .map(ToOwned::to_owned)
        .unwrap_or_default();

    let body_selector = Selector::parse("body").expect("static selector");
    let mut raw_text = String::new();
    if let Some(body) = document.select(&body_selector).next() {
        walk_node(&body, &mut raw_text);
    } else {
        walk_node(&document.root_element(), &mut raw_text);
    }
    let body_text = normalize_text(&raw_text);

    if body_text.chars().count() < 200 {
        return Err(ScrapeError::TooLittleText);
    }

    let mut cleaned = String::new();
    if !title.is_empty() {
        cleaned.push_str(&title);
        cleaned.push('\n');
    }
    if !description.is_empty() {
        cleaned.push_str(&normalize_text(&description).replace('\n', " "));
        cleaned.push('\n');
    }
    cleaned.push_str(&body_text);
    cleaned.push_str(&format!("\nSource URL: {page_url}"));

    let hash = content_hash(&cleaned);
    Ok(ScrapedPage {
        url: page_url.to_owned(),
        title: truncate_chars(&title, 300),
        text: truncate_chars(&cleaned, max_text_chars),
        content_hash: hash,
        http_status: 200,
    })
}

fn walk_node(node: &NodeRef<'_, Node>, out: &mut String) {
    match node.value() {
        Node::Text(text) => out.push_str(&text.text),
        Node::Element(element) => {
            let name = element.name();
            if SKIP_TAGS.contains(&name) {
                return;
            }
            let is_block = BLOCK_TAGS.contains(&name);
            if is_block {
                out.push('\n');
            }
            for child in node.children() {
                walk_node(&child, out);
            }
            if is_block {
                out.push('\n');
            }
            return;
        }
        _ => {}
    }

    for child in node.children() {
        walk_node(&child, out);
    }
}

fn normalize_text(input: &str) -> String {
    input
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, Default)]
pub struct RobotsRules {
    groups: Vec<RobotsGroup>,
}

#[derive(Debug, Clone, Default)]
struct RobotsGroup {
    agents: Vec<String>,
    rules: Vec<RobotsRule>,
    crawl_delay_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct RobotsRule {
    allow: bool,
    pattern: String,
}

impl RobotsRules {
    pub fn allow_all() -> Self {
        Self::default()
    }

    pub fn parse(body: &str) -> Self {
        let mut groups: Vec<RobotsGroup> = Vec::new();
        let mut current: Option<RobotsGroup> = None;
        let mut last_was_agent = false;

        for raw_line in body.lines() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();

            match key.as_str() {
                "user-agent" => {
                    if !last_was_agent {
                        if let Some(group) = current.take() {
                            groups.push(group);
                        }
                    }
                    let group = current.get_or_insert_with(RobotsGroup::default);
                    if !value.is_empty() {
                        group.agents.push(value.to_ascii_lowercase());
                    }
                    last_was_agent = true;
                }
                "allow" | "disallow" => {
                    last_was_agent = false;
                    if value.is_empty() {
                        continue;
                    }
                    if let Some(group) = current.as_mut() {
                        group.rules.push(RobotsRule {
                            allow: key == "allow",
                            pattern: value.to_owned(),
                        });
                    }
                }
                "crawl-delay" => {
                    last_was_agent = false;
                    if let (Some(group), Ok(seconds)) = (current.as_mut(), value.parse::<f64>()) {
                        if seconds.is_finite() && seconds >= 0.0 {
                            group.crawl_delay_ms = Some((seconds * 1000.0) as u64);
                        }
                    }
                }
                _ => {
                    last_was_agent = false;
                }
            }
        }
        if let Some(group) = current {
            groups.push(group);
        }

        Self { groups }
    }

    fn matching_groups<'a>(&'a self, user_agent: &str) -> Vec<&'a RobotsGroup> {
        let ua = user_agent.to_ascii_lowercase();
        let specific: Vec<_> = self
            .groups
            .iter()
            .filter(|group| {
                group.agents.iter().any(|agent| {
                    agent != "*" && !agent.is_empty() && ua.contains(agent.as_str())
                })
            })
            .collect();
        if !specific.is_empty() {
            return specific;
        }
        self.groups
            .iter()
            .filter(|group| group.agents.iter().any(|agent| agent == "*"))
            .collect()
    }

    pub fn allows(&self, path: &str, user_agent: &str) -> bool {
        let groups = self.matching_groups(user_agent);
        if groups.is_empty() {
            return true;
        }

        let mut best_length = 0usize;
        let mut best_allow = true;
        for rule in groups.iter().flat_map(|group| group.rules.iter()) {
            if robot_pattern_matches(&rule.pattern, path) {
                let length = rule.pattern.len();
                if length > best_length || (length == best_length && rule.allow) {
                    best_length = length;
                    best_allow = rule.allow;
                }
            }
        }
        best_allow
    }

    pub fn crawl_delay(&self) -> Option<Duration> {
        let millis = self
            .groups
            .iter()
            .filter_map(|group| group.crawl_delay_ms)
            .max()?;
        Some(Duration::from_millis(millis.min(30_000)))
    }
}

/// robots.txt patterns are prefix matches unless they contain `*` or end in `$`.
fn robot_pattern_matches(pattern: &str, path: &str) -> bool {
    let anchored = pattern.ends_with('$');
    let pattern = pattern.strip_suffix('$').unwrap_or(pattern);
    if pattern.is_empty() {
        return true;
    }

    let pattern_chars: Vec<char> = pattern.chars().collect();
    let path_chars: Vec<char> = path.chars().collect();
    let mut pi = 0usize;
    let mut ti = 0usize;
    let mut star: Option<usize> = None;
    let mut star_match = 0usize;

    while ti < path_chars.len() {
        if pi == pattern_chars.len() && !anchored {
            return true;
        }
        if pi < pattern_chars.len()
            && (pattern_chars[pi] == '?' || pattern_chars[pi] == path_chars[ti])
        {
            pi += 1;
            ti += 1;
        } else if pi < pattern_chars.len() && pattern_chars[pi] == '*' {
            star = Some(pi);
            pi += 1;
            star_match = ti;
        } else if let Some(star_index) = star {
            pi = star_index + 1;
            star_match += 1;
            ti = star_match;
        } else {
            return false;
        }
    }

    while pi < pattern_chars.len() && pattern_chars[pi] == '*' {
        pi += 1;
    }
    pi == pattern_chars.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_robots_groups_and_specificity() {
        let rules = RobotsRules::parse(
            "User-agent: *\nDisallow: /private\n\nUser-agent: HackathonHunter\nDisallow: /admin\nAllow: /admin/public\nCrawl-delay: 2\n",
        );
        assert!(!rules.allows("/admin/secret", "HackathonHunter/1.0"));
        assert!(rules.allows("/admin/public/page", "HackathonHunter/1.0"));
        assert!(rules.allows("/private", "HackathonHunter/1.0"));
        assert!(!rules.allows("/private", "SomeOtherBot/1.0"));
        assert_eq!(rules.crawl_delay(), Some(Duration::from_secs(2)));
    }

    #[test]
    fn supports_wildcard_and_end_anchor() {
        let rules =
            RobotsRules::parse("User-agent: *\nDisallow: /*.pdf$\nDisallow: /tmp/*/secret\n");
        assert!(!rules.allows("/docs/file.pdf", "bot"));
        assert!(rules.allows("/docs/file.pdf?download=1", "bot"));
        assert!(!rules.allows("/tmp/a/secret", "bot"));
        assert!(rules.allows("/tmp/a/public", "bot"));
    }

    #[test]
    fn empty_robots_allows_everything() {
        let rules = RobotsRules::parse("");
        assert!(rules.allows("/anything", "HackathonHunter"));
    }

    #[test]
    fn cleans_navigation_scripts_and_keeps_content() {
        let html = r#"
            <html>
              <head>
                <title>AI Hackathon 2026</title>
                <meta name="description" content="Build AI in Shanghai">
                <style>.x{color:red}</style>
                <script>alert('noise')</script>
              </head>
              <body>
                <nav>Home About</nav>
                <main>
                  <h1>AI Hackathon 2026</h1>
                  <p>Welcome to the biggest hackathon in Shanghai. Registration is open now.</p>
                  <ul><li>Track A</li><li>Track B</li></ul>
                </main>
                <footer>Copyright noise</footer>
              </body>
            </html>
        "#;
        // The body is deliberately padded to pass the minimum content guard.
        let padded = format!("{html}<p>{}</p>", "more details ".repeat(30));
        let page = clean_html(&padded, "https://example.com/event", 50_000).unwrap();
        assert_eq!(page.title, "AI Hackathon 2026");
        assert!(page.text.contains("Welcome to the biggest hackathon"));
        assert!(page.text.contains("Track A"));
        assert!(!page.text.contains("alert('noise')"));
        assert!(!page.text.contains("Home About"));
        assert!(!page.text.contains("Copyright noise"));
        assert_eq!(page.content_hash.len(), 64);
    }

    #[test]
    fn rejects_pages_without_content() {
        let html = "<html><body><nav>only nav</nav></body></html>";
        assert!(matches!(
            clean_html(html, "https://example.com", 10_000),
            Err(ScrapeError::TooLittleText)
        ));
    }
}
