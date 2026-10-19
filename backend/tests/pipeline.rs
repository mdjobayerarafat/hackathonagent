use chrono::{Duration, Utc};
use hackathon_hunter::agent::pipeline;
use hackathon_hunter::config::Config;
use hackathon_hunter::db;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config(mock_uri: &str, database_url: String) -> Config {
    Config {
        host: "127.0.0.1".to_owned(),
        port: 0,
        database_url,
        test_database_url: String::new(),
        db_max_connections: 5,
        app_api_key: "test-key".to_owned(),
        frontend_origins: vec!["http://localhost:3000".to_owned()],
        searxng_url: mock_uri.to_owned(),
        searxng_timeout_secs: 10,
        search_queries: vec!["hackathon China test".to_owned()],
        max_results_per_query: 10,
        max_urls_per_run: 10,
        ninerouter_base_url: format!("{mock_uri}/v1"),
        ninerouter_api_key: "test-router-key".to_owned(),
        ninerouter_model: "test-model".to_owned(),
        llm_timeout_secs: 20,
        confidence_threshold: 0.5,
        max_text_chars: 20_000,
        scrape_timeout_secs: 10,
        scrape_concurrency: 2,
        extract_concurrency: 1,
        domain_rate_limit_ms: 0,
        source_refresh_hours: 72,
        user_agent: "HackathonHunterTest/1.0".to_owned(),
        resend_api_key: String::new(),
        email_from: "test@example.com".to_owned(),
        email_to: String::new(),
        auto_send_email: false,
        dashboard_url: "http://localhost:3000".to_owned(),
        cron_schedule: "0 0 8 * * *".to_owned(),
        cron_timezone: "Asia/Shanghai".to_owned(),
    }
}

fn deadline_in_days(days: i64) -> String {
    (Utc::now().date_naive() + Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

fn llm_response(extraction: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": extraction.to_string()
            },
            "finish_reason": "stop"
        }]
    }))
}

async fn setup_mocks(server: &MockServer) {
    let searx = json!({
        "query": "hackathon China test",
        "results": [
            {
                "url": format!("{}/page/one", server.uri()),
                "title": "Alpha AI Hackathon 2026",
                "content": "A hackathon in Shanghai"
            },
            {
                "url": format!("{}/page/two", server.uri()),
                "title": "Beta Robotics Challenge 2026",
                "content": "A robotics competition in Shenzhen"
            },
            {
                "url": format!("{}/blocked", server.uri()),
                "title": "Blocked page",
                "content": "robots.txt should prevent this fetch"
            },
            {
                "url": "mailto:not-a-web-page@example.com",
                "title": "Invalid URL"
            }
        ]
    });

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(searx))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/robots.txt"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("User-agent: HackathonHunterTest\nDisallow: /blocked\n", "text/plain"),
        )
        .mount(server)
        .await;

    let alpha_html = format!(
        r#"
        <html><head><title>Alpha AI Hackathon 2026</title>
        <meta name="description" content="Build AI agents in Shanghai">
        <script>console.log('SHOULD_NOT_APPEAR')</script></head>
        <body>
          <nav>Home Events About</nav>
          <main>
            <h1>Alpha AI Hackathon 2026</h1>
            <p>ALPHA_UNIQUE_TOKEN Registrations are open for a 48 hour AI hackathon in Shanghai.
            Teams of up to four will build agents and compete for a 100,000 CNY prize pool.
            The event is open to students and professionals. Apply before {}.</p>
            <p>{}</p>
          </main>
          <footer>Footer noise</footer>
        </body></html>
        "#,
        deadline_in_days(10),
        "More event details and schedule information. ".repeat(8)
    );

    let beta_html = format!(
        r#"
        <html><head><title>Beta Robotics Challenge 2026</title></head>
        <body><main>
          <h1>Beta Robotics Challenge 2026</h1>
          <p>BETA_UNIQUE_TOKEN A robotics challenge in Shenzhen for university teams.
          Build autonomous robots and present them to judges. Registration closes {}.
          The winner receives 50,000 CNY and incubation support.</p>
          <p>{}</p>
        </main></body></html>
        "#,
        deadline_in_days(20),
        "Robot categories, rules and judging criteria. ".repeat(8)
    );

    Mock::given(method("GET"))
        .and(path("/page/one"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(alpha_html, "text/html; charset=utf-8"),
        )
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/page/two"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(beta_html, "text/html; charset=utf-8"),
        )
        .mount(server)
        .await;

    let alpha_extraction = json!({
        "is_hackathon": true,
        "title": "Alpha AI Hackathon 2026",
        "title_zh": "阿尔法人工智能黑客松 2026",
        "organizer": "Alpha Labs",
        "description": "A 48-hour AI agent hackathon in Shanghai for students and professionals.",
        "location": "Shanghai, China",
        "is_online": false,
        "start_date": deadline_in_days(30),
        "end_date": deadline_in_days(32),
        "registration_deadline": deadline_in_days(10),
        "registration_url": "https://register.example.com/alpha",
        "prize_summary": "100,000 CNY prize pool",
        "prize_amount_cny": 100000,
        "tracks_or_themes": ["AI Agents", "LLM"],
        "eligibility": "Students and professionals",
        "tags": ["ai", "agents", "shanghai"],
        "confidence": 0.94,
        "source_url": "https://ignored.example.com"
    });

    let beta_extraction = json!({
        "is_hackathon": true,
        "title": "Beta Robotics Challenge 2026",
        "title_zh": null,
        "organizer": "Beta University",
        "description": "A robotics challenge for university teams in Shenzhen.",
        "location": "Shenzhen, China",
        "is_online": false,
        "start_date": deadline_in_days(40),
        "end_date": deadline_in_days(42),
        "registration_deadline": deadline_in_days(20),
        "registration_url": "https://register.example.com/beta",
        "prize_summary": "50,000 CNY",
        "prize_amount_cny": 50000,
        "tracks_or_themes": ["Robotics"],
        "eligibility": "University students",
        "tags": ["robotics", "shenzhen", "students"],
        "confidence": 0.88,
        "source_url": "https://ignored.example.com"
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains("ALPHA_UNIQUE_TOKEN"))
        .respond_with(llm_response(alpha_extraction))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains("BETA_UNIQUE_TOKEN"))
        .respond_with(llm_response(beta_extraction))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected prompt"))
        .mount(server)
        .await;
}

async fn prepare_database(pool: &PgPool) {
    db::migrate(pool).await.expect("migrations");
    sqlx::query("TRUNCATE email_log, sources, hackathons, search_runs RESTART IDENTITY CASCADE")
        .execute(pool)
        .await
        .expect("truncate test database");
}

#[tokio::test]
async fn full_pipeline_run_with_mocked_http_and_deduplication() {
    let database_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        dotenvy::from_filename("../.env").ok();
        dotenvy::dotenv().ok();
        std::env::var("TEST_DATABASE_URL").expect(
            "TEST_DATABASE_URL must point at a dedicated PostgreSQL test database \
             (see .env.example). It is never read from DATABASE_URL.",
        )
    });

    let pool = db::connect(&database_url, 5).await.expect("test database");
    prepare_database(&pool).await;

    let server = MockServer::start().await;
    setup_mocks(&server).await;

    let config = Arc::new(test_config(&server.uri(), database_url));
    let run_id = pipeline::create_run(&pool, config.search_queries.len(), "manual")
        .await
        .expect("create run");

    let summary = pipeline::execute(run_id, pool.clone(), config.clone())
        .await
        .expect("pipeline run");

    assert_eq!(summary.found_count, 2, "two hackathons should be extracted");
    assert_eq!(summary.new_count, 2, "both should be new");
    assert_eq!(summary.sources_failed, 1, "robots-blocked URL should fail");
    assert_eq!(summary.sources_fetched, 2);

    let rows = sqlx::query_as::<_, (String, Option<String>, String, f32, String)>(
        "SELECT title, registration_url, status, confidence, source_url
         FROM hackathons ORDER BY title",
    )
    .fetch_all(&pool)
    .await
    .expect("query hackathons");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "Alpha AI Hackathon 2026");
    assert_eq!(
        rows[0].1.as_deref(),
        Some("https://register.example.com/alpha")
    );
    assert_eq!(rows[0].2, "new");
    assert!(rows[0].3 > 0.9);
    assert!(rows[0].4.contains("/page/one"));
    assert_eq!(rows[1].0, "Beta Robotics Challenge 2026");

    let run = pipeline::get_run(&pool, run_id)
        .await
        .expect("get run")
        .expect("run exists");
    assert_eq!(run.status, "completed");
    assert_eq!(run.found_count, 2);
    assert_eq!(run.new_count, 2);
    assert_eq!(run.progress, 100);
    assert!(
        run.errors.as_array().is_some_and(|errors| !errors.is_empty()),
        "robots failure should be recorded in run errors"
    );
    assert!(
        run.logs
            .as_array()
            .is_some_and(|logs| logs.iter().any(|entry| entry
                .get("message")
                .and_then(|message| message.as_str())
                .is_some_and(|message| message.contains("Run complete")))),
        "run should log completion"
    );

    let blocked_source: (String, Option<String>) = sqlx::query_as(
        "SELECT status, error FROM sources WHERE normalized_url LIKE '%/blocked'",
    )
    .fetch_one(&pool)
    .await
    .expect("blocked source row");
    assert_eq!(blocked_source.0, "failed");
    assert!(blocked_source
        .1
        .as_deref()
        .unwrap_or_default()
        .contains("robots"));

    // A second run skips recently processed sources, proving dedupe across runs.
    let second_id = pipeline::create_run(&pool, config.search_queries.len(), "manual")
        .await
        .expect("create second run");
    let second = pipeline::execute(second_id, pool.clone(), config)
        .await
        .expect("second pipeline run");
    assert_eq!(second.new_count, 0);
    assert_eq!(second.found_count, 0);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hackathons")
        .fetch_one(&pool)
        .await
        .expect("count hackathons");
    assert_eq!(count, 2, "second run must not duplicate events");

    let runs = pipeline::list_runs(&pool, 10).await.expect("list runs");
    assert_eq!(runs.len(), 2);
}
