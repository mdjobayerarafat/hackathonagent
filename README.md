# Hackathon Hunter

A personal AI agent that finds hackathons in China, extracts structured details, stores them in PostgreSQL, shows them in a bilingual dashboard, and emails a modern HTML digest.

It searches in Chinese and English, respects `robots.txt`, rate-limits by domain, never invents missing fields, and skips events whose registration deadline has passed.

![Stack](https://img.shields.io/badge/Rust-Axum%20%2B%20SQLx-b7410e)
![Stack](https://img.shields.io/badge/Next.js-14%20App%20Router-000000)
![Stack](https://img.shields.io/badge/SearXNG-JSON%20API-3050ff)
![Stack](https://img.shields.io/badge/LLM-9router-6d28d9)

---

## Architecture

```mermaid
flowchart LR
    Cron["tokio-cron-scheduler\n08:00 Asia/Shanghai"] --> Agent
    UI["Next.js dashboard\nEN / 中文"] -->|"POST /api/agent/run"| API["Axum REST API"]
    API --> Agent["Agent pipeline"]
    Agent -->|"12 bilingual queries"| SX["SearXNG\nJSON API"]
    SX -->|"deduped URLs"| Scrape["reqwest + scraper\nrobots.txt · rate limits · retries"]
    Scrape -->|"clean truncated text"| LLM["9router\n/v1/chat/completions"]
    LLM -->|"strict JSON\n1 retry"| Validate["Serde validation\ndedupe · deadline check"]
    Validate -->|"upsert"| DB[("PostgreSQL")]
    DB --> Mail["Resend\nHTML + text digest"]
    DB --> UI
    Mail --> Inbox["Your inbox"]
```

**Run lifecycle:** `discovering → scraping → extracting → storing → completed`. One failed URL is recorded in `search_runs.errors` and `sources.error`; it never aborts the run.

---

## Project structure

```text
hackathon-hunter/
├── backend/
│   ├── migrations/
│   │   └── 20261019000001_init.sql
│   ├── src/
│   │   ├── agent/
│   │   │   ├── search.rs       # SearXNG JSON client + URL dedupe
│   │   │   ├── scrape.rs       # robots.txt, per-domain limiter, retries, text cleaning
│   │   │   ├── extract.rs      # 9router client, strict schema validation, tests
│   │   │   ├── pipeline.rs     # orchestration, persistence, run accounting
│   │   │   └── mod.rs          # URL normalization, hashing, tests
│   │   ├── config.rs           # all environment configuration
│   │   ├── db.rs               # pool, migrations, interrupted-run recovery
│   │   ├── email.rs            # Resend API, HTML/text templates, email_log
│   │   ├── error.rs            # typed API errors
│   │   ├── lib.rs
│   │   ├── main.rs             # tracing, cron scheduler, server, graceful shutdown
│   │   ├── models.rs
│   │   └── routes.rs           # Axum API, filters, API-key middleware, CORS
│   ├── tests/
│   │   └── pipeline.rs         # end-to-end run against mocked SearXNG/pages/9router
│   ├── Cargo.toml
│   └── Dockerfile
├── frontend/
│   ├── app/                    # App Router pages, layout, providers, global CSS
│   ├── components/
│   │   ├── ui/                 # shadcn/ui primitives
│   │   ├── dashboard.tsx
│   │   ├── hackathon-card.tsx
│   │   ├── hackathon-detail.tsx
│   │   ├── deadline-timeline.tsx
│   │   ├── runs-view.tsx
│   │   └── …
│   ├── lib/                    # API client, types, i18n, formatting
│   └── Dockerfile
├── searxng/
│   └── settings.yml            # JSON output enabled, limiter disabled
├── docker-compose.yml
├── .env.example
└── README.md
```

---

## Database schema

All migrations run automatically at backend startup via `sqlx::migrate!`.

| Table | Purpose | Key columns / indexes |
| --- | --- | --- |
| `hackathons` | One row per extracted event | `status` (`new`, `notified`, `saved`, `dismissed`), `registration_deadline`, `prize_amount_cny`, `tags text[]`, `tracks_or_themes text[]`, `first_seen_at`, `last_seen_at`, `content_hash`, `content_changed_at` |
| `search_runs` | Audit trail and live progress | `status`, `phase`, `progress`, `started_at`, `finished_at`, `queries_count`, `found_count`, `new_count`, `errors jsonb`, `logs jsonb` |
| `sources` | Every URL ever scraped | `normalized_url UNIQUE`, `status`, `http_status`, `fetched_at`, `error`, `content_hash` |
| `email_log` | Every digest attempt | `sent_at`, `hackathon_ids uuid[]`, `resend_id`, `status`, `error` |

Deduplication is enforced three ways:

1. `dedupe_key` — normalized registration URL, or `source_url + normalized title`.
2. A partial unique index on `registration_url_normalized`.
3. A unique expression index on `(source_url_normalized, lower(title))`.

Indexes cover status, deadline, created_at, prize, `tags` (GIN) and `tracks_or_themes` (GIN). A trigger maintains `updated_at`.

---

## Quick start with Docker Compose

```bash
cp .env.example .env
# Set APP_API_KEY, NINEROUTER_BASE_URL, NINEROUTER_API_KEY,
# NINEROUTER_MODEL, RESEND_API_KEY, EMAIL_FROM and EMAIL_TO.

docker compose up --build -d
docker compose logs -f backend
```

Services:

| Service | URL | Notes |
| --- | --- | --- |
| Frontend | http://localhost:3000 | Dashboard + runs history |
| Backend | http://localhost:8080 | REST API |
| SearXNG | http://localhost:8081 | JSON API enabled |
| PostgreSQL | localhost:5432 | `hackathon_hunter` database |

Compose overrides `DATABASE_URL` and `SEARXNG_URL` so the containers talk to each other; your `.env` values are used for every other setting. The frontend's public API URL stays `http://localhost:8080` because the browser makes those requests.

Trigger the first run:

```bash
curl -X POST http://localhost:8080/api/agent/run \
  -H "x-api-key: $APP_API_KEY"
```

---

## Local development

### Prerequisites

- Rust 1.82+ (`rustc --version`)
- Node.js 20+ and npm
- PostgreSQL 14+
- A SearXNG instance with JSON enabled (self-host with the included `searxng/settings.yml`, or point `SEARXNG_URL` at an existing instance)

### 1. Databases

```bash
createdb hackathon_hunter
createdb hackathon_hunter_test
```

`TEST_DATABASE_URL` must point at a **dedicated** database: the integration test truncates it. It is never inferred from `DATABASE_URL`.

### 2. Backend

```bash
cp .env.example .env
# edit .env

cd backend
cargo run
```

On boot the backend applies migrations, marks interrupted runs as failed, starts the cron scheduler, and listens on `APP_PORT` (default `8080`).

Verify:

```bash
curl http://localhost:8080/api/health
```

### 3. Frontend

```bash
cd frontend
cp .env.example .env.local
npm install
npm run dev        # http://localhost:3000
```

If port 3000 is occupied, use `npm run dev -- -p 3100` and keep that origin in `FRONTEND_ORIGINS`.

### 4. First run

Use **Run agent** in the header, or:

```bash
API_KEY=your-app-api-key
curl -X POST http://localhost:8080/api/agent/run -H "x-api-key: $API_KEY"
```

Watch progress in **Agent runs**. The dashboard refreshes automatically when the run finishes.

---

## Configuration

All settings are environment variables; `.env.example` documents every one.

| Variable | Default | Description |
| --- | --- | --- |
| `DATABASE_URL` | — | PostgreSQL connection string (required) |
| `TEST_DATABASE_URL` | — | Dedicated database for `cargo test` |
| `APP_HOST` / `APP_PORT` | `0.0.0.0` / `8080` | Axum bind address |
| `APP_API_KEY` | `change-me` | Shared key for `POST` / `PATCH` endpoints |
| `FRONTEND_ORIGINS` | `http://localhost:3000` | Comma-separated CORS allow-list |
| `SEARXNG_URL` | `http://localhost:8081` | SearXNG base URL |
| `NINEROUTER_BASE_URL` | `https://api.9router.ai/v1` | OpenAI-compatible base URL |
| `NINEROUTER_API_KEY` | empty | 9router key (required for runs) |
| `NINEROUTER_MODEL` | `gpt-4o-mini` | Model name, e.g. `ollama` |
| `CONFIDENCE_THRESHOLD` | `0.6` | Discard extractions below this score |
| `MAX_RESULTS_PER_QUERY` | `8` | Results kept per query |
| `MAX_URLS_PER_RUN` | `30` | Pages scraped per run |
| `MAX_TEXT_CHARS` | `16000` | Clean text sent to the model |
| `SCRAPE_CONCURRENCY` | `4` | Parallel page fetches |
| `EXTRACT_CONCURRENCY` | `2` | Parallel LLM requests |
| `DOMAIN_RATE_LIMIT_MS` | `1500` | Minimum delay per domain |
| `SOURCE_REFRESH_HOURS` | `72` | Re-check processed URLs after this |
| `RESEND_API_KEY` | empty | Resend API key |
| `EMAIL_FROM` | `Hackathon Hunter <digest@example.com>` | Verified Resend sender |
| `EMAIL_TO` | empty | Digest recipient(s), comma-separated |
| `AUTO_SEND_EMAIL` | `true` | Send a digest after runs with new events |
| `CRON_SCHEDULE` | `0 0 8 * * *` | Six-field cron (seconds included) |
| `CRON_TIMEZONE` | `Asia/Shanghai` | Scheduler timezone |
| `DASHBOARD_URL` | `http://localhost:3000` | Link in the email |

`SEARCH_QUERIES` can replace the built-in list with semicolon-separated queries.

---

## Adding search queries

Queries live in two places:

1. **Environment override** — no rebuild required:

   ```bash
   SEARCH_QUERIES="黑客松 2026 报名;hackathon China 2026 registration;site:devpost.com China hackathon;site:tianchi.aliyun.com 大赛"
   ```

   Separate queries with `;`. Each query is sent to SearXNG separately so engine limits do not hide results.

2. **Built-in defaults** — edit `DEFAULT_SEARCH_QUERIES` in [backend/src/config.rs](backend/src/config.rs).

Query tips:

- Pair every English phrase with a Chinese one (`hackathon China 2026` / `黑客松 2026 报名`).
- Use `site:` hints for Devpost, SegmentFault, ModelScope, Tianchi, Kaggle, university pages and company challenge pages.
- Prefer specific date phrases (`2026`, `报名`, `registration`) over generic words.
- Keep the number of queries moderate: each one costs a SearXNG request, and `MAX_URLS_PER_RUN` caps the scrape budget.
- URLs already scraped within `SOURCE_REFRESH_HOURS` are skipped automatically.

---

## REST API

Read endpoints are public (CORS-limited). Write endpoints require `x-api-key: $APP_API_KEY` or `Authorization: Bearer $APP_API_KEY`.

| Method | Endpoint | Description |
| --- | --- | --- |
| `GET` | `/api/health` | Database ping, version, timestamp |
| `GET` | `/api/hackathons` | Filtered, sorted, paginated list |
| `GET` | `/api/hackathons/{id}` | One event |
| `PATCH` | `/api/hackathons/{id}` | Body `{"status":"saved"}` or `{"status":"dismissed"}` |
| `POST` | `/api/agent/run` | Start a run, returns `202 {"run_id": "...", "status": "running"}` |
| `GET` | `/api/agent/runs?limit=20` | Run history |
| `GET` | `/api/agent/runs/{id}` | Progress, logs and errors |
| `POST` | `/api/email/digest` | Send a digest of un-notified new events |

`GET /api/hackathons` query parameters:

| Parameter | Example | Notes |
| --- | --- | --- |
| `status` | `new` | `new` (includes notified), `saved`, `dismissed`, `closing_soon`, `all` |
| `tag` | `ai` | `tags @> ARRAY[...]` |
| `online` | `true` | Boolean |
| `location` | `Shanghai` | Case-insensitive substring |
| `deadline_before` / `deadline_after` | `2026-12-31` | ISO dates |
| `min_prize` | `10000` | Minimum `prize_amount_cny` |
| `q` | `AI 创新` | Searches title, Chinese title, organizer, description, location |
| `page` / `page_size` | `1` / `24` | `page_size` capped at 100 |
| `sort` | `deadline_asc` | `deadline_asc`, `deadline_desc`, `newest`, `oldest`, `prize_desc`, `confidence_desc` |

Response shape:

```json
{
  "items": [],
  "total": 0,
  "page": 1,
  "page_size": 24,
  "total_pages": 0
}
```

---

## Extraction contract

The system prompt requires exactly one JSON object with this schema:

```json
{
  "is_hackathon": true,
  "title": "string",
  "title_zh": "string|null",
  "organizer": "string|null",
  "description": "string",
  "location": "string|null",
  "is_online": true,
  "start_date": "YYYY-MM-DD|null",
  "end_date": "YYYY-MM-DD|null",
  "registration_deadline": "YYYY-MM-DD|null",
  "registration_url": "string|null",
  "prize_summary": "string|null",
  "prize_amount_cny": 0,
  "tracks_or_themes": ["string"],
  "eligibility": "string|null",
  "tags": ["string"],
  "confidence": 0.0,
  "source_url": "string"
}
```

- Invalid JSON is retried once with the validation error fed back to the model.
- `is_hackathon: false` and low-confidence results are discarded.
- Invalid dates, empty titles, inverted date ranges and other schema problems fail validation rather than being invented.
- `source_url` is overwritten with the actual scraped page URL; the model cannot inject it.
- Expired deadlines (compared in `Asia/Shanghai`) are skipped before storage.

---

## Email digest

`send_digest` loads up to 20 `new` events that are not expired, renders both HTML and plain text, and calls `POST https://api.resend.com/emails`.

- Table-based layout with inline CSS for Gmail, Apple Mail and Outlook.
- Branded gradient header, summary line, deadline badges (red ≤3 days, amber ≤7, green otherwise), highlighted prize, bright **Register now** CTA.
- **View all in dashboard** button and footer.
- Every attempt is written to `email_log` with `sent`, `failed` or `skipped`.
- Successful recipients are moved from `new` to `notified`, so each event is emailed once.

`EMAIL_FROM` must use a domain verified in Resend. Set `AUTO_SEND_EMAIL=false` to disable digests while keeping the manual endpoint available.

### Running locally without a domain

If you have not verified a domain in Resend, Resend rejects any sender outside its own domain with `403 The example.com domain is not verified`. Use Resend's built-in test sender instead:

```json
"email_from": "Hackathon Hunter <onboarding@resend.dev>"
```

While `onboarding@resend.dev` is the sender, Resend only delivers to the address on your Resend account, so `EMAIL_TO` / `email_to` must be that exact address — every other recipient returns `403 You can only send testing emails to your own email address (...)`.

Once you own a domain: add it at <https://resend.com/domains>, create the DNS records Resend shows (TXT for SPF/DKIM, plus CNAME for DKIM/DMARC if offered), wait for the `Verified` badge, then switch `EMAIL_FROM` to e.g. `Hackathon Hunter <digest@yourdomain.com>` and set `EMAIL_TO` to any recipients you like. Verification takes about five minutes and needs no code changes.

---

## Tests

```bash
cd backend

# Unit tests: extraction parsing, dedupe, URL normalization, robots.txt, email HTML.
cargo test --lib

# Integration test: full pipeline against mocked SearXNG, web pages and 9router.
# Requires a dedicated database and never reads DATABASE_URL as a fallback.
TEST_DATABASE_URL=postgres://postgres:postgres@localhost:5432/hackathon_hunter_test \
  cargo test --test pipeline -- --nocapture

cd ../frontend
npm run typecheck
npm run lint
npm run build
```

The integration test asserts:

- two mocked events are scraped, extracted and stored;
- a robots-blocked URL fails without aborting the run;
- run logs and errors are persisted;
- a second run skips recently processed sources and creates no duplicates.

---

## Operations

- **Schedule:** `CRON_SCHEDULE` is parsed by `tokio-cron-scheduler` with a **seconds field**. Daily 08:00 in Shanghai is `0 0 8 * * *`.
- **Migrations:** embedded via `sqlx::migrate!`, applied on every boot.
- **Interrupted runs:** on startup, any `running` row is marked `failed` / `interrupted` so the UI never hangs.
- **Concurrency:** the API rejects a second simultaneous run with `409 Conflict`.
- **Rate limiting:** per-domain request serialization plus `crawl-delay` from robots.txt (capped at 30 s).
- **Re-checking sources:** URLs are skipped for `SOURCE_REFRESH_HOURS`; afterwards they are re-scraped and `content_hash` detects changes (changed events return to `new` unless saved/dismissed).
- **Logs:** structured `tracing` output; set `RUST_LOG=debug` for query-level detail.

---

## Security notes

This is a personal dashboard, not a multi-tenant service.

- The API key is shipped to the browser as `NEXT_PUBLIC_APP_API_KEY`. It stops casual writes but is not a secret. Do not expose the app publicly without an authenticating reverse proxy.
- Read endpoints are intentionally public behind CORS.
- `.env` and `frontend/.env.local` are git-ignored. Never commit real API keys.
- The scraper obeys `robots.txt`, identifies itself with `USER_AGENT`, and throttles per domain.

---

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| `cannot reach backend` toast | Confirm `NEXT_PUBLIC_API_URL` and that the backend is on port 8080 |
| `401 unauthorized` | `NEXT_PUBLIC_APP_API_KEY` must equal `APP_API_KEY`; restart the Next.js dev server after changing it |
| SearXNG returns HTML instead of JSON | Ensure `search.formats` includes `json` in `searxng/settings.yml` and restart the container |
| All runs fail at extraction | Check `NINEROUTER_API_KEY`, `NINEROUTER_BASE_URL` (include `/v1`) and `NINEROUTER_MODEL`; the endpoint must support `stream:false` |
| No digest email | `RESEND_API_KEY` and `EMAIL_TO` must be set, `EMAIL_FROM` must be verified, and there must be unexpired `new` events |
| CORS error | Add the exact browser origin (scheme + host + port) to `FRONTEND_ORIGINS` |
