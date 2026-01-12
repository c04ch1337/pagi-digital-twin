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
COPY backend-rust-tools/proto/tools.proto ./backend-rust-tools/proto/

# Copy tools service source
COPY backend-rust-tools/Cargo.toml backend-rust-tools/build.rs ./backend-rust-tools/
COPY backend-rust-tools/src ./backend-rust-tools/src

WORKDIR /src/backend-rust-tools

# Build the application
RUN cargo build --release

# --- STAGE 2: RUNTIME ---
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from the builder stage
COPY --from=builder /src/backend-rust-tools/target/release/backend-rust-tools ./backend-rust-tools

# gRPC
EXPOSE 50054

CMD ["./backend-rust-tools"]
