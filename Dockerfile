FROM rust:slim-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY config/ config/

RUN cargo build --locked --release -p kias-main --bin kias

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home --home-dir /home/kias kias \
    && mkdir -p /app/config /app/data \
    && chown -R kias:kias /app /home/kias

WORKDIR /app
COPY --from=builder /app/target/release/kias /usr/local/bin/kias
COPY --from=builder /app/config/ /app/config/

ENV KIAS_CONFIG=/app/config/default.toml \
    KIAS_DB_PATH=/app/data/kias.db

VOLUME ["/app/data"]
EXPOSE 8080

USER kias

HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=6 \
    CMD curl --fail --silent http://127.0.0.1:8080/health || exit 1

ENTRYPOINT ["/usr/local/bin/kias"]
CMD ["server", "--host", "0.0.0.0", "--port", "8080"]
