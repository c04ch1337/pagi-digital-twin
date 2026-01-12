# --- STAGE 1: BUILD ---
FROM rust:1.78 AS builder
WORKDIR /src

# System deps for:
# - tonic-build/prost-build (protoc)
# - reqwest default TLS backend (openssl)
RUN apt-get update \
    && apt-get install -y --no-install-recommends protobuf-compiler pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# NOTE: this Dockerfile expects the Docker build context to be the repo root.
# Copy proto files from other services
COPY backend-rust-memory/proto/memory.proto ./backend-rust-memory/proto/
COPY backend-rust-tools/proto/tools.proto ./backend-rust-tools/proto/

# Copy gateway source
COPY backend-rust-gateway/Cargo.toml backend-rust-gateway/build.rs ./backend-rust-gateway/
COPY backend-rust-gateway/src ./backend-rust-gateway/src

WORKDIR /src/backend-rust-gateway

# Build the application
RUN cargo build --release

# --- STAGE 2: RUNTIME ---
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from the builder stage
COPY --from=builder /src/backend-rust-gateway/target/release/backend-rust-gateway ./backend-rust-gateway

# WebSocket + HTTP
EXPOSE 8181

CMD ["./backend-rust-gateway"]
