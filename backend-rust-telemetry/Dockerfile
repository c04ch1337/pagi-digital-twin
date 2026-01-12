# --- STAGE 1: BUILD ---
FROM rust:1.78 AS builder
WORKDIR /src

# Copy telemetry service source
COPY backend-rust-telemetry/Cargo.toml ./backend-rust-telemetry/
COPY backend-rust-telemetry/src ./backend-rust-telemetry/src

WORKDIR /src/backend-rust-telemetry

# Build the application
RUN cargo build --release

# --- STAGE 2: RUNTIME ---
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/backend-rust-telemetry/target/release/backend-rust-telemetry ./backend-rust-telemetry

EXPOSE 8183

CMD ["./backend-rust-telemetry"]

