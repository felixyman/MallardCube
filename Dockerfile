# syntax=docker/dockerfile:1
# Build stage
FROM rust:bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY data/seed_date_dim.sql ./data/seed_date_dim.sql
RUN cargo build --release --bin mallard

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/mallard /usr/local/bin/mallard
# The bundled sample project, so `mallard serve` works without a config.
COPY projects/project3 /app/projects/project3
WORKDIR /app
ENV BIND_ADDRESS=0.0.0.0:8080
EXPOSE 8080
ENTRYPOINT ["mallard"]
CMD ["serve"]
