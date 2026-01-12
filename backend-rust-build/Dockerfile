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
COPY backend-rust-build/proto/build.proto ./backend-rust-build/proto/

# Copy build service source
COPY backend-rust-build/Cargo.toml backend-rust-build/build.rs ./backend-rust-build/
COPY backend-rust-build/src ./backend-rust-build/src

WORKDIR /src/backend-rust-build

# Build the application
RUN cargo build --release

# --- STAGE 2: RUNTIME ---
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from the builder stage
COPY --from=builder /src/backend-rust-build/target/release/backend-rust-build ./backend-rust-build

# gRPC
EXPOSE 50055

CMD ["./backend-rust-build"]

