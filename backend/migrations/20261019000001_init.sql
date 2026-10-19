-- Hackathon Hunter schema.
-- All timestamps are UTC; the UI converts to the visitor's locale.

CREATE TABLE hackathons (
    id                          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    title                       text        NOT NULL,
    title_zh                    text,
    organizer                   text,
    description                 text        NOT NULL,
    location                    text,
    is_online                   boolean,
    start_date                  date,
    end_date                    date,
    registration_deadline       date,
    registration_url            text,
    registration_url_normalized text,
    source_url                  text        NOT NULL,
    source_url_normalized       text        NOT NULL,
    prize_summary               text,
    prize_amount_cny            double precision,
    tracks_or_themes            text[]      NOT NULL DEFAULT '{}',
    eligibility                 text,
    tags                        text[]      NOT NULL DEFAULT '{}',
    confidence                  real        NOT NULL DEFAULT 0,
    status                      text        NOT NULL DEFAULT 'new'
                                CHECK (status IN ('new', 'notified', 'saved', 'dismissed')),
    dedupe_key                  text        NOT NULL,
    content_hash                text        NOT NULL,
    first_seen_at               timestamptz NOT NULL DEFAULT now(),
    last_seen_at                timestamptz NOT NULL DEFAULT now(),
    content_changed_at          timestamptz,
    created_at                  timestamptz NOT NULL DEFAULT now(),
    updated_at                  timestamptz NOT NULL DEFAULT now()
);

-- Identity of an event: its registration URL when known, otherwise source + title.
CREATE UNIQUE INDEX hackathons_dedupe_key_uniq
    ON hackathons (dedupe_key);
CREATE UNIQUE INDEX hackathons_registration_url_uniq
    ON hackathons (registration_url_normalized)
    WHERE registration_url_normalized IS NOT NULL;
CREATE UNIQUE INDEX hackathons_source_title_uniq
    ON hackathons (source_url_normalized, lower(title));

CREATE INDEX hackathons_status_idx ON hackathons (status);
CREATE INDEX hackathons_created_at_idx ON hackathons (created_at DESC);
CREATE INDEX hackathons_deadline_idx
    ON hackathons (registration_deadline)
    WHERE registration_deadline IS NOT NULL;
CREATE INDEX hackathons_status_deadline_idx
    ON hackathons (status, registration_deadline)
    WHERE registration_deadline IS NOT NULL;
CREATE INDEX hackathons_tags_gin_idx ON hackathons USING GIN (tags);
CREATE INDEX hackathons_tracks_gin_idx
    ON hackathons USING GIN (tracks_or_themes);
CREATE INDEX hackathons_prize_idx
    ON hackathons (prize_amount_cny DESC)
    WHERE prize_amount_cny IS NOT NULL;

CREATE TABLE search_runs (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    trigger        text        NOT NULL DEFAULT 'manual'
                   CHECK (trigger IN ('manual', 'cron')),
    status         text        NOT NULL DEFAULT 'running'
                   CHECK (status IN ('running', 'completed', 'failed')),
    phase          text        NOT NULL DEFAULT 'starting',
    progress       integer     NOT NULL DEFAULT 0 CHECK (progress BETWEEN 0 AND 100),
    started_at     timestamptz NOT NULL DEFAULT now(),
    finished_at    timestamptz,
    queries_count  integer     NOT NULL DEFAULT 0,
    found_count    integer     NOT NULL DEFAULT 0,
    new_count      integer     NOT NULL DEFAULT 0,
    errors         jsonb       NOT NULL DEFAULT '[]'::jsonb,
    logs           jsonb       NOT NULL DEFAULT '[]'::jsonb
);

CREATE INDEX search_runs_started_at_idx ON search_runs (started_at DESC);
CREATE INDEX search_runs_status_idx ON search_runs (status);

CREATE TABLE sources (
    id             bigserial PRIMARY KEY,
    run_id         uuid REFERENCES search_runs (id) ON DELETE SET NULL,
    url            text        NOT NULL,
    normalized_url text        NOT NULL UNIQUE,
    domain         text        NOT NULL,
    status         text        NOT NULL DEFAULT 'pending'
                   CHECK (status IN ('pending', 'fetched', 'extracted', 'skipped', 'failed')),
    http_status    integer,
    title          text,
    content_hash   text,
    error          text,
    fetched_at     timestamptz,
    created_at     timestamptz NOT NULL DEFAULT now(),
    updated_at     timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX sources_run_id_idx ON sources (run_id);
CREATE INDEX sources_status_idx ON sources (status);
CREATE INDEX sources_fetched_at_idx ON sources (fetched_at DESC);

CREATE TABLE email_log (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    sent_at       timestamptz NOT NULL DEFAULT now(),
    hackathon_ids uuid[]      NOT NULL DEFAULT '{}',
    resend_id     text,
    status        text        NOT NULL CHECK (status IN ('sent', 'failed', 'skipped')),
    error         text,
    recipient     text,
    subject       text
);

CREATE INDEX email_log_sent_at_idx ON email_log (sent_at DESC);

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS trigger AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER hackathons_set_updated_at
    BEFORE UPDATE ON hackathons
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER sources_set_updated_at
    BEFORE UPDATE ON sources
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
