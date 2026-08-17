# --- Build Stage ---
FROM rust:1-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace / package manifests and source code
COPY core/Cargo.toml core/Cargo.lock* ./
COPY core/src ./src

# Build production release binary
RUN cargo build --release --bin hitkote-core
# --- Runtime Stage ---
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Copy compiled binary from build stage
COPY --from=builder /app/target/release/hitkote-core /app/hitkote-core
ENV RUST_LOG=info
ENV REDIS_URL=redis://redis:6379

CMD ["/app/hitkote-core"]