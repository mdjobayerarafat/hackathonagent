FROM rust:1.99-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY tests ./tests

RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 hunter \
    && useradd --system --uid 10001 --gid hunter --create-home hunter

WORKDIR /app
RUN mkdir -p /app/data && chown -R hunter:hunter /app

COPY --from=builder /app/target/release/hackathon-hunter /usr/local/bin/hackathon-hunter

ENV RUST_LOG=info,hackathon_hunter=info,tower_http=warn
ENV APP_HOST=0.0.0.0
ENV APP_PORT=8080
ENV SETTINGS_FILE=/app/data/settings.json

USER hunter
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD ["sh", "-c", "curl -fsS http://127.0.0.1:8080/api/health || exit 1"]

ENTRYPOINT ["hackathon-hunter"]
