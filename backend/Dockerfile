FROM rust:1.86-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY backend/Cargo.toml backend/Cargo.lock ./backend/
COPY backend/src ./backend/src
COPY migrations ./migrations

WORKDIR /app/backend
RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/backend/target/release/mrr-fantasy-backend /usr/local/bin/mrr-fantasy-backend

ENV SERVER_HOST=0.0.0.0
ENV PORT=8080

EXPOSE 8080

CMD ["mrr-fantasy-backend"]
