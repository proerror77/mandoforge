FROM rust:1.91-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p mandoforge-api --bins

FROM debian:trixie-slim
WORKDIR /app
RUN useradd --create-home --shell /usr/sbin/nologin mandoforge
COPY --from=builder /app/target/release/mandoforge-api /usr/local/bin/mandoforge-api
COPY --from=builder /app/target/release/mandoforge-worker /usr/local/bin/mandoforge-worker
COPY web ./web
USER mandoforge
EXPOSE 8787
CMD ["mandoforge-api"]
