# backend-rust-telemetry

Rust-based Telemetry (Observability) service for the PAGI Digital Twin platform.

## Overview

This service exposes a Server-Sent Events (SSE) stream that the frontend consumes (typically via the Gateway proxy) to render real-time telemetry charts.

## Endpoints

- `GET /v1/telemetry/stream` (SSE)

Each event uses `event: metrics` with a JSON payload.

## Configuration

Environment variables:

- `TELEMETRY_PORT` (default: `8183`)
- `TELEMETRY_INTERVAL_MS` (default: `2000`)

## Notes

Metric collection is cross-platform via the `sysinfo` crate.

