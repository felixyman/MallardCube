# syntax=docker/dockerfile:1
# Build stage
# Keep the base tag in sync with rust-toolchain.toml (pinned toolchain).
FROM rust:1.98.0-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Dependency layer: build against a stub crate so the expensive dependency
# compile (including the bundled DuckDB C++) is cached until the manifests
# change; only the workspace crate rebuilds when src/ changes.
RUN mkdir -p src \
    && printf 'fn main() {}\n' > src/main.rs \
    && printf '' > src/lib.rs \
    && cargo build --release --bin mallard \
    && rm -rf src
COPY src ./src
COPY data/seed_date_dim.sql ./data/seed_date_dim.sql
# COPY preserves the context mtimes, which are older than the artifacts the
# stub layer just built; without the touch cargo considers the real sources
# fresh and links the stub. Touch every input the crate compiles.
RUN find src data -exec touch {} + \
    && cargo build --release --bin mallard

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
