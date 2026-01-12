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

# Copy orchestrator source
COPY backend-rust-orchestrator/Cargo.toml backend-rust-orchestrator/build.rs ./backend-rust-orchestrator/
COPY backend-rust-orchestrator/src ./backend-rust-orchestrator/src

WORKDIR /src/backend-rust-orchestrator

# Build the application
RUN cargo build --release

# --- STAGE 2: RUNTIME ---
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from the builder stage
COPY --from=builder /src/backend-rust-orchestrator/target/release/backend-rust-orchestrator ./backend-rust-orchestrator

# HTTP
EXPOSE 8182

CMD ["./backend-rust-orchestrator"]
