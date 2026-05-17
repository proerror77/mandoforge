FROM rust:1.91-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p mandoforge-api --bins

FROM debian:trixie-slim
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends bash ca-certificates curl jq \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /usr/sbin/nologin mandoforge
COPY --from=builder /app/target/release/mandoforge-api /usr/local/bin/mandoforge-api
COPY --from=builder /app/target/release/mandoforge-worker /usr/local/bin/mandoforge-worker
COPY README.md README.zh-CN.md ./
COPY config ./config
COPY db ./db
COPY docs ./docs
COPY schemas ./schemas
COPY packs ./packs
COPY web ./web
COPY scripts ./scripts
COPY deploy ./deploy
USER mandoforge
EXPOSE 8787
CMD ["mandoforge-api"]
