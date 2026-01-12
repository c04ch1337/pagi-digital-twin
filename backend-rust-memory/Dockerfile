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
# Copy proto files
COPY backend-rust-memory/proto/memory.proto ./backend-rust-memory/proto/

# Copy memory service source
COPY backend-rust-memory/Cargo.toml backend-rust-memory/build.rs ./backend-rust-memory/
COPY backend-rust-memory/src ./backend-rust-memory/src

WORKDIR /src/backend-rust-memory

# Build the application
RUN cargo build --release

# --- STAGE 2: RUNTIME ---
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from the builder stage
COPY --from=builder /src/backend-rust-memory/target/release/backend-rust-memory ./backend-rust-memory

# gRPC
EXPOSE 50052

CMD ["./backend-rust-memory"]
