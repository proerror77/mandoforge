FROM lukemathwalker/cargo-chef:latest-rust-1.91-slim AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
ARG CARGO_BUILD_JOBS
RUN if [ -n "$CARGO_BUILD_JOBS" ]; then \
      cargo chef cook --release -p mandoforge-api --recipe-path recipe.json --jobs "$CARGO_BUILD_JOBS"; \
    else \
      cargo chef cook --release -p mandoforge-api --recipe-path recipe.json; \
    fi
COPY . .
RUN if [ -n "$CARGO_BUILD_JOBS" ]; then \
      cargo build --release -p mandoforge-api --bins --jobs "$CARGO_BUILD_JOBS"; \
    else \
      cargo build --release -p mandoforge-api --bins; \
    fi

FROM debian:trixie-slim
ARG MANDOFORGE_IMAGE_TAG=local
ARG MANDOFORGE_GIT_SHA=unknown
ARG MANDOFORGE_BUILD_TIME=unknown
ENV MANDOFORGE_IMAGE_TAG=${MANDOFORGE_IMAGE_TAG}
ENV MANDOFORGE_GIT_SHA=${MANDOFORGE_GIT_SHA}
ENV MANDOFORGE_BUILD_TIME=${MANDOFORGE_BUILD_TIME}
LABEL org.opencontainers.image.title="mandoforge-api" \
      org.opencontainers.image.revision="${MANDOFORGE_GIT_SHA}" \
      org.opencontainers.image.version="${MANDOFORGE_IMAGE_TAG}" \
      org.opencontainers.image.created="${MANDOFORGE_BUILD_TIME}"
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
