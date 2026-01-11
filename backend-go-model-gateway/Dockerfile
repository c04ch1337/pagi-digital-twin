# --- STAGE 1: BUILD ---
FROM golang:1.24 AS builder
WORKDIR /src

# NOTE: this Dockerfile expects the Docker build context to be the repo root.
COPY backend-go-model-gateway/ ./backend-go-model-gateway/

WORKDIR /src/backend-go-model-gateway
RUN go mod download

# Build the application
RUN CGO_ENABLED=0 GOOS=linux go build -o /out/model-gateway

# --- STAGE 2: RUNTIME ---
FROM gcr.io/distroless/base-debian12
WORKDIR /app
COPY --from=builder /out/model-gateway ./model-gateway

EXPOSE 50051
CMD ["/app/model-gateway"]

