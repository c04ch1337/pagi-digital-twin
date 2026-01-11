# --- STAGE 1: BUILD ---
FROM golang:1.24 AS builder
WORKDIR /src

# NOTE: this Dockerfile expects the Docker build context to be the repo root.
COPY backend-go-notification-service/ ./backend-go-notification-service/

WORKDIR /src/backend-go-notification-service
RUN go mod download

RUN CGO_ENABLED=0 GOOS=linux go build -o /out/notification-service

# --- STAGE 2: RUNTIME ---
FROM gcr.io/distroless/base-debian12
WORKDIR /app

COPY --from=builder /out/notification-service ./notification-service

CMD ["/app/notification-service"]

