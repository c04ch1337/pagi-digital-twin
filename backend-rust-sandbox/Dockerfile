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
# The Rust build.rs references ../backend-go-model-gateway/proto/model.proto.
COPY backend-rust-sandbox/Cargo.toml backend-rust-sandbox/Cargo.lock backend-rust-sandbox/build.rs ./backend-rust-sandbox/
COPY backend-rust-sandbox/src ./backend-rust-sandbox/src
COPY backend-go-model-gateway/proto/model.proto ./backend-go-model-gateway/proto/model.proto

WORKDIR /src/backend-rust-sandbox

# Build the application
RUN cargo build --release

# --- STAGE 2: RUNTIME ---
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from the builder stage
COPY --from=builder /src/backend-rust-sandbox/target/release/backend-rust-sandbox ./backend-rust-sandbox

# HTTP (optional) + gRPC
EXPOSE 8001
EXPOSE 50053

CMD ["./backend-rust-sandbox"]

