FROM rust:1.91-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p mandoforge-api

FROM debian:trixie-slim
WORKDIR /app
RUN useradd --create-home --shell /usr/sbin/nologin mandoforge
COPY --from=builder /app/target/release/mandoforge-api /usr/local/bin/mandoforge-api
COPY web ./web
USER mandoforge
EXPOSE 8787
CMD ["mandoforge-api"]

